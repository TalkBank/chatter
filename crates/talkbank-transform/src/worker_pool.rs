//! The one bounded fan-out every corpus-scale command runs its files through.
//!
//! `chatter validate` (through the validation runner) and `chatter to-json`
//! each had a hand-written pool: their own spawn loop, their own queue size,
//! their own idea of what a worker panic meant, and different thread stacks
//! from the program thread they replaced. This module is the single pool.
//!
//! # Shape
//!
//! ```text
//! items --(calling thread feeds)--> bounded queue --> worker 1 --> R
//!                                                 --> worker 2 --> R
//!                                                 --> ...
//! join every worker --> PoolRun { results: one R per returned worker, outcome }
//! ```
//!
//! - **Width.** The requested count, else the machine's parallelism, never
//!   zero. Serial work is a pool of width one; there is no
//!   separate serial path to drift from the parallel one.
//! - **Stack.** Every worker runs on a [`CHAT_THREAD_STACK_BYTES`] stack, the size
//!   the CLI gives its program thread, because CHAT parsing and validation
//!   recurse. A default `thread::spawn` stack is 2 MiB.
//! - **Results.** Each worker returns its own value (counts, say) and the
//!   caller combines them after the join. Nothing is read from shared
//!   counters whose exactness depends on a comment about when they are read.
//! - **Stopping early.** The caller decides by the iterator it passes (for
//!   example `take_while(|_| !cancelled())`); a worker decides by returning.
//!   When every worker has returned, feeding stops at the next item.
//! - **Panics.** A worker that unwinds is joined and counted, never
//!   re-raised: [`PoolOutcome::SomeUnwound`] says how many, and its items
//!   produced no result, which the caller can measure against what it fed.
//!
//! Scoped threads, so the worker closure may borrow from the caller.

use crossbeam_channel::{Receiver, bounded};
use std::num::NonZeroUsize;
use std::thread;

/// Stack size of every thread that parses or validates CHAT: each pool
/// worker, and the CLI's program thread, which reads this constant rather
/// than keeping its own copy. 16 MiB, because deeply nested CHAT recurses in
/// the parser and validator.
pub const CHAT_THREAD_STACK_BYTES: usize = 16 * 1024 * 1024;

/// How many workers to run: the requested count, or the machine's
/// parallelism when none was requested, but never more than `most`, the
/// items' own upper bound when they state one (a worker with nothing to
/// take would only reserve its stack). Never zero, so the run always makes
/// progress: the request is non-zero by type, a platform that cannot
/// report its parallelism runs one worker and says so in the log, and no
/// items still get one worker, which finds the queue closed. `available`
/// is only called when no count was requested.
pub(crate) fn worker_count(
    requested: Option<NonZeroUsize>,
    available: impl FnOnce() -> std::io::Result<NonZeroUsize>,
    most: Option<usize>,
) -> NonZeroUsize {
    let wanted = requested.unwrap_or_else(|| {
        available().unwrap_or_else(|err| {
            tracing::warn!(error = %err, "cannot determine available parallelism; using 1 worker");
            NonZeroUsize::MIN
        })
    });
    match most.map(NonZeroUsize::new) {
        // The items do not say how many they are.
        None => wanted,
        Some(None) => NonZeroUsize::MIN,
        Some(Some(most)) => wanted.min(most),
    }
}

/// The items one worker takes, in turn, until the feeder has no more.
///
/// An iterator over the shared bounded queue. Ends when the caller's items
/// are exhausted (or its iterator stopped early) and the queue is drained.
pub struct WorkQueue<T>(Receiver<T>);

impl<T> Iterator for WorkQueue<T> {
    type Item = T;

    fn next(&mut self) -> Option<T> {
        self.0.recv().ok()
    }
}

/// What became of the pool's workers.
#[derive(Debug)]
pub enum PoolOutcome {
    /// Every worker returned normally.
    AllReturned,
    /// Some workers unwound. Whatever items they had taken produced no
    /// result; the others' results are in [`PoolRun::results`].
    SomeUnwound {
        /// How many workers unwound; never zero, by type.
        unwound_workers: NonZeroUsize,
    },
    /// A worker thread could not be started. No item was fed: the workers
    /// that did start saw an empty queue.
    CouldNotStart {
        /// Why the operating system refused the thread.
        error: std::io::Error,
        /// How many of the workers that did start unwound anyway (normally
        /// none): kept, so a panic is never hidden behind the start failure.
        unwound_workers: usize,
    },
}

/// One way a pool's workers failed, whatever the pool was running.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PoolFault {
    /// Worker threads unwound (panicked); the items they had taken produced
    /// no result.
    Unwound {
        /// How many workers unwound.
        workers: NonZeroUsize,
    },
    /// The operating system refused a worker thread, so no item was fed.
    ThreadRefused {
        /// The operating system's error, as text (`io::Error` is not
        /// `Clone`, and a fault travels in events that are).
        reason: String,
    },
}

impl std::fmt::Display for PoolFault {
    /// The fault as a clause safe to show a user verbatim.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unwound { workers } => {
                write!(f, "{workers} worker(s) failed with an internal error")
            }
            Self::ThreadRefused { reason } => {
                write!(f, "a worker thread could not be started ({reason})")
            }
        }
    }
}

impl PoolOutcome {
    /// The worker failures this outcome is, each said by its `Display`:
    /// none when every worker returned, otherwise a refused thread and any
    /// unwound workers, one never hiding the other. The one reading of a
    /// pool's outcome every caller shares.
    pub fn faults(&self) -> impl Iterator<Item = PoolFault> {
        let (refused, unwound) = match self {
            Self::AllReturned => (None, None),
            Self::SomeUnwound { unwound_workers } => (None, Some(*unwound_workers)),
            Self::CouldNotStart {
                error,
                unwound_workers,
            } => (Some(error.to_string()), NonZeroUsize::new(*unwound_workers)),
        };
        [
            refused.map(|reason| PoolFault::ThreadRefused { reason }),
            unwound.map(|workers| PoolFault::Unwound { workers }),
        ]
        .into_iter()
        .flatten()
    }
}

/// The pool's result: one value per worker that returned, and what became
/// of the rest.
#[derive(Debug)]
pub struct PoolRun<R> {
    /// One result per worker that returned normally.
    pub results: Vec<R>,
    /// Whether every worker returned.
    pub outcome: PoolOutcome,
}

/// Feed `items` to a pool of workers and collect what each returns.
///
/// `requested` is the job count (`--jobs`): the pool runs that many workers,
/// or the machine's parallelism when it is `None`, never zero and never more
/// than the items' upper bound (see [`worker_count`]). Each worker
/// runs `worker` on its own [`WorkQueue`] view of the shared queue, on a
/// [`CHAT_THREAD_STACK_BYTES`] stack. The calling thread feeds the items and then
/// joins every worker, so this returns only when all are done.
pub fn fan_out<T, R, I, W>(items: I, requested: Option<NonZeroUsize>, worker: W) -> PoolRun<R>
where
    I: IntoIterator<Item = T>,
    T: Send,
    R: Send,
    W: Fn(WorkQueue<T>) -> R + Sync,
{
    let items = items.into_iter();
    let width = worker_count(
        requested,
        thread::available_parallelism,
        items.size_hint().1,
    );
    // Twice the width: enough that no worker waits for the feeder, small
    // enough that a stopped pool leaves little fed but untaken.
    let (tx, rx) = bounded::<T>(width.get() * 2);
    let worker = &worker;

    thread::scope(|scope| {
        let mut handles = Vec::with_capacity(width.get());
        let mut spawn_error = None;
        for index in 0..width.get() {
            let queue = WorkQueue(rx.clone());
            let spawned = thread::Builder::new()
                .name(format!("talkbank-worker-{index}"))
                .stack_size(CHAT_THREAD_STACK_BYTES)
                .spawn_scoped(scope, move || worker(queue));
            match spawned {
                Ok(handle) => handles.push(handle),
                Err(error) => {
                    spawn_error = Some(error);
                    break;
                }
            }
        }
        // Only the workers hold receivers now, so a send fails exactly when
        // every worker has returned or unwound: stop feeding then.
        drop(rx);
        if spawn_error.is_none() {
            for item in items {
                if tx.send(item).is_err() {
                    break;
                }
            }
        }
        // Closing the queue ends each worker's `WorkQueue`.
        drop(tx);

        let mut results = Vec::with_capacity(handles.len());
        let mut unwound = 0usize;
        for handle in handles {
            match handle.join() {
                Ok(result) => results.push(result),
                Err(payload) => {
                    tracing::error!(panic = ?payload, "pool worker panicked");
                    unwound += 1;
                }
            }
        }
        let outcome = match (spawn_error, NonZeroUsize::new(unwound)) {
            (Some(error), _) => PoolOutcome::CouldNotStart {
                error,
                unwound_workers: unwound,
            },
            (None, None) => PoolOutcome::AllReturned,
            (None, Some(unwound_workers)) => PoolOutcome::SomeUnwound { unwound_workers },
        };
        PoolRun { results, outcome }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The worker count is never zero: a platform that cannot report its
    /// parallelism runs one worker, and otherwise the requested count or the
    /// reported parallelism is used as given. A requested zero is not a case
    /// here: the request is a `NonZeroUsize`.
    #[test]
    fn worker_count_is_the_request_or_the_parallelism_and_never_zero() {
        let eight = || Ok(NonZeroUsize::new(8).unwrap());
        let unknown = || Err(std::io::Error::other("no parallelism information"));
        assert_eq!(worker_count(NonZeroUsize::new(3), eight, None).get(), 3);
        assert_eq!(worker_count(None, eight, None).get(), 8);
        assert_eq!(worker_count(None, unknown, None).get(), 1);
    }

    /// Every item reaches exactly one worker, at width one and above, and
    /// each returned worker contributes one result.
    #[test]
    fn every_item_is_taken_exactly_once_at_any_width() {
        for width in [1, 3] {
            let run = fan_out(1..=100u64, NonZeroUsize::new(width), |queue| {
                queue.collect::<Vec<_>>()
            });
            assert!(matches!(run.outcome, PoolOutcome::AllReturned));
            assert_eq!(run.results.len(), width);
            let mut taken: Vec<u64> = run.results.into_iter().flatten().collect();
            taken.sort_unstable();
            assert_eq!(taken, (1..=100).collect::<Vec<_>>());
        }
    }

    /// A pool never starts more workers than it has items, so one file
    /// (each save under `chatter watch`) is one worker, not one per core;
    /// at least one runs even with nothing to take.
    #[test]
    fn a_pool_runs_no_more_workers_than_items() {
        let wide = NonZeroUsize::new(8);
        assert_eq!(
            fan_out([1u32], wide, |queue| queue.count()).results.len(),
            1
        );
        assert_eq!(
            fan_out([1u32, 2, 3], wide, |queue| queue.count())
                .results
                .len(),
            3
        );
        assert_eq!(
            fan_out(1..=20u32, wide, |queue| queue.count())
                .results
                .len(),
            8
        );
        assert_eq!(
            fan_out(std::iter::empty::<u32>(), wide, |queue| queue.count())
                .results
                .len(),
            1
        );
    }

    /// A worker that unwinds is counted, not re-raised; the others' results
    /// survive, and the items the unwound worker took are missing from them.
    #[test]
    fn an_unwinding_worker_is_counted_and_the_rest_return() {
        let run = fan_out(1..=50u32, NonZeroUsize::new(2), |queue| {
            let mut taken = Vec::new();
            for item in queue {
                if item == 7 {
                    panic!("worker fault at item 7");
                }
                taken.push(item);
            }
            taken
        });
        match run.outcome {
            PoolOutcome::SomeUnwound { unwound_workers } => {
                assert_eq!(unwound_workers.get(), 1)
            }
            other => panic!("expected one unwound worker, got {other:?}"),
        }
        let taken: Vec<u32> = run.results.into_iter().flatten().collect();
        assert!(!taken.contains(&7));
        assert!(taken.len() < 50, "the unwound worker's items are missing");
    }

    /// Workers run on the large stack: a recursion needing about 6 MiB, which
    /// overflows a default 2 MiB thread, completes.
    #[test]
    fn workers_run_on_the_large_stack() {
        fn deep(depth: u32) -> u64 {
            // 64 KiB of stack per frame, kept live by `black_box`.
            let frame = std::hint::black_box([depth as u8; 64 * 1024]);
            if depth == 0 {
                u64::from(frame[0])
            } else {
                deep(depth - 1) + u64::from(frame[frame.len() - 1])
            }
        }
        let run = fan_out([96u32], NonZeroUsize::new(1), |queue| {
            queue.map(deep).sum::<u64>()
        });
        assert!(matches!(run.outcome, PoolOutcome::AllReturned));
        assert_eq!(run.results.len(), 1);
    }
}
