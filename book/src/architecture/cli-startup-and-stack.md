# CLI Startup and the Program Stack

**Status:** Current
**Last modified:** {{git-dates:page}}

The CLI runs argument construction, parsing and command execution on an
explicitly sized thread. This keeps that work off the operating system's
main-thread stack, whose available budget depends on the target and executable.
The historical failure recorded in the startup regression was command-tree
construction overflowing on Windows before argument parsing began.

## Startup ownership

`crates/chatter/src/main.rs` owns the transition:

1. `main()` requests a thread named `chatter-program` with
   `PROGRAM_STACK_BYTES`.
2. That thread runs `program_main()`: collect arguments, construct the clap
   command tree, parse arguments and call `cli::run`.
3. The main thread joins it. If the program thread unwinds, `resume_unwind`
   propagates its panic payload. If thread creation fails, startup prints an
   error and returns the internal-failure status 3.

Every ordinary handler returns `CommandResult<T>` through the dispatcher.
`ExitStatus` owns the numeric process codes. The boundary returns that typed
status, and both top-level help and JSON command discovery describe the same
closed set. Help can no longer omit classified statuses by maintaining a
separate hand-written table; machine discovery exposes the process contract
without implying that every command can produce every status.
`CommandStop` distinguishes already-rendered domain failures, usage diagnostics
and human or machine output failures. The main thread consumes that result and
returns `ExitCode`; handlers and signal callbacks do not terminate the process.
Help and version are successful writes, and OS-native arguments avoid a Unicode
conversion panic before clap can report usage.

Text macros and JSON record writers propagate a failed write immediately. No
later command mutation follows it. A failed validation renderer requests worker
cancellation and releases its receiver; an unfinished audit writer is joined
and its unpublished regular-file stage is discarded. The second Ctrl-C sends a
forced-stop event to the validation loop, which returns status 130. The TUI owns
terminal restoration through setup, rendering and input failures as well as
normal and forced completion.

An abort, an external kill or an uncaught panic is not a recoverable command
result. Files already published before a later failure remain published; this
boundary does not turn a multi-file command into a transaction.

`PROGRAM_STACK_BYTES` aliases
`talkbank_transform::worker_pool::CHAT_THREAD_STACK_BYTES`, currently 16 MiB.
There are not two independent budgets to keep synchronized. This is an
explicit requested stack size, not a proof that arbitrary nesting fits or a
promise about physical-memory use or startup latency. Build profile, generated
code and platform affect stack needs; evaluate a budget change deliberately.

## Regression evidence and its limits

The test lives at
`crates/chatter/tests/integration/stack_limit_tests.rs` and is included by
the CLI's integration test target. Its
`help_runs_within_windows_sized_main_stack` test runs the actual test-built
binary as `chatter --help`, through a shell that first sets `ulimit -s 1024`.
The shell failure also fails the test because execution is chained with `&&`.

This Unix-only control checks startup and clap construction under that shell
limit. It does not run every command, measure recursive parser depth, reproduce
all Windows stack behavior, or qualify a released executable. The configured
Windows job in `.github/workflows/cross-platform.yml` runs `just test-workspace`
on Windows; an actual successful job is separate evidence, not implied by the
configuration or a local Unix result.

## Worker threads and contributor guidance

Keep command-tree construction and application work in `program_main()`, not
before the spawn. For bounded file-processing fan-out, use
`talkbank_transform::worker_pool::fan_out`; its scoped workers request the
same `CHAT_THREAD_STACK_BYTES` budget.

Worker failure semantics differ from startup. `fan_out` joins its workers and
returns `PoolRun`, whose `PoolOutcome` distinguishes normal returns, unwinding
workers and inability to start a worker. Callers must consume that outcome;
the presence of some worker results does not prove all work succeeded. If
worker creation fails, no input is fed to the workers already started.

The pool's `workers_run_on_the_large_stack` regression exercises recursive
work. Neither that finite control nor the startup test establishes an unlimited
recursion guarantee. New thread-owning components must account for their own
stack and failure contract rather than assuming the CLI's program-thread
setting applies to them.
