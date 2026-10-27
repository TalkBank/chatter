//! Ownership of one desktop run across startup, streaming and cancellation.

use std::sync::Arc;

use arc_swap::ArcSwapOption;
use talkbank_transform::Canceller;

use crate::errors::ValidationStartError;
use crate::events::FrontendEvent;
use crate::protocol::ValidationRunId;

/// Constructed only by the session that owns the event's producer.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ValidationEventEnvelope {
    run_id: ValidationRunId,
    event: FrontendEvent,
}

enum Session {
    Starting(ValidationRunId),
    Running {
        id: ValidationRunId,
        canceller: Canceller,
    },
}

impl Session {
    fn id(&self) -> &ValidationRunId {
        match self {
            Self::Starting(id) | Self::Running { id, .. } => id,
        }
    }

    fn cancel(&self) {
        match self {
            Self::Starting(_) => {}
            Self::Running { canceller, .. } => canceller.cancel(),
        }
    }
}

/// The single pending or running request selected by the desktop.
#[derive(Default)]
pub(crate) struct ValidationSessions {
    current: ArcSwapOption<Session>,
}

impl ValidationSessions {
    /// Replace prior ownership before startup begins, stopping its active runner.
    pub(crate) fn begin(self: &Arc<Self>, id: ValidationRunId) -> PendingValidation {
        let session = Arc::new(Session::Starting(id));
        if let Some(previous) = self.current.swap(Some(Arc::clone(&session))) {
            previous.cancel();
        }
        PendingValidation(Lease {
            owner: Arc::clone(self),
            session,
        })
    }

    /// Revoke only the named request; a stale caller cannot cancel its successor.
    pub(crate) fn cancel(&self, id: &ValidationRunId) {
        while let Some(session) = self.current.load_full() {
            if session.id() != id {
                return;
            }
            if self.replace(&session, None) {
                session.cancel();
                return;
            }
        }
    }

    fn replace(&self, expected: &Arc<Session>, next: Option<Arc<Session>>) -> bool {
        let previous = self
            .current
            .compare_and_swap(&Some(Arc::clone(expected)), next);
        previous
            .as_ref()
            .is_some_and(|value| Arc::ptr_eq(value, expected))
    }
}

struct Lease {
    owner: Arc<ValidationSessions>,
    session: Arc<Session>,
}

impl Lease {
    fn event(&self, event: FrontendEvent) -> ValidationEventEnvelope {
        ValidationEventEnvelope {
            run_id: self.session.id().clone(),
            event,
        }
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        if self.owner.replace(&self.session, None) {
            self.session.cancel();
        }
    }
}

/// Startup ownership that must survive cancellation/replacement to admit a runner.
pub(crate) struct PendingValidation(Lease);

impl PendingValidation {
    /// Bind startup's cache warning to the requesting run.
    pub(crate) fn cache_unavailable(&self, reason: String) -> ValidationEventEnvelope {
        self.0.event(FrontendEvent::CacheUnavailable { reason })
    }

    /// Start the shared runner and bind its stream and canceller to this lease.
    /// A revoked lease stops the runner instead of replacing the current session.
    pub(crate) fn start(
        self,
        target: std::path::PathBuf,
        run: &talkbank_transform::ValidationRun,
    ) -> Result<RunningValidation, ValidationStartError> {
        let (events, canceller) =
            crate::validation::validate_target_streaming_with_config(target, run)?;
        let running = Arc::new(Session::Running {
            id: self.0.session.id().clone(),
            canceller,
        });
        if !self
            .0
            .owner
            .replace(&self.0.session, Some(Arc::clone(&running)))
        {
            running.cancel();
            return Err(ValidationStartError::Superseded);
        }
        Ok(RunningValidation {
            lease: Lease {
                owner: Arc::clone(&self.0.owner),
                session: running,
            },
            events,
        })
    }
}

/// One admitted runner, its own event stream and identity-bound cleanup.
pub(crate) struct RunningValidation {
    lease: Lease,
    events: crossbeam_channel::Receiver<FrontendEvent>,
}

impl RunningValidation {
    /// Read and identify an event from this runner without accepting another stream.
    pub(crate) fn next_event(
        &self,
    ) -> Result<ValidationEventEnvelope, crossbeam_channel::RecvError> {
        self.events.recv().map(|event| self.lease.event(event))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_transform::{ValidationConfig, ValidationRun};

    fn id(value: &str) -> ValidationRunId {
        serde_json::from_value(serde_json::json!(value)).unwrap()
    }

    fn start(pending: PendingValidation) -> Result<RunningValidation, ValidationStartError> {
        let reference = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../corpus/reference/core/basic-conversation.cha");
        pending.start(
            reference,
            &ValidationRun::uncached(ValidationConfig::default()),
        )
    }

    #[test]
    fn cancelled_or_replaced_startup_cannot_take_the_current_slot() {
        tauri::async_runtime::block_on(async {
            let sessions = Arc::new(ValidationSessions::default());
            let cancelled = sessions.begin(id("cancelled"));
            sessions.cancel(&id("cancelled"));
            assert!(matches!(
                start(cancelled),
                Err(ValidationStartError::Superseded)
            ));
            assert!(sessions.current.load_full().is_none());

            let old = sessions.begin(id("old"));
            let current = sessions.begin(id("current"));
            assert!(matches!(start(old), Err(ValidationStartError::Superseded)));
            assert_eq!(sessions.current.load_full().unwrap().id(), &id("current"));
            drop(current);
            assert!(sessions.current.load_full().is_none());
        });
    }

    #[test]
    fn stale_cancel_and_completion_preserve_the_new_run_and_events_keep_identity() {
        tauri::async_runtime::block_on(async {
            let sessions = Arc::new(ValidationSessions::default());
            let pending = sessions.begin(id("old"));
            let cache_event = pending.cache_unavailable("offline".into());
            assert_eq!(serde_json::to_value(cache_event).unwrap()["runId"], "old");
            let active = start(pending).unwrap();
            let current = sessions.begin(id("current"));
            let current = start(current).unwrap();
            sessions.cancel(&id("old"));
            assert_eq!(sessions.current.load_full().unwrap().id(), &id("current"));
            let late = serde_json::to_value(active.next_event().unwrap()).unwrap();
            assert_eq!(
                late,
                serde_json::json!({"runId": "old", "event": {"type": "discovering"}})
            );
            drop(active);
            assert_eq!(sessions.current.load_full().unwrap().id(), &id("current"));
            let current_event = serde_json::to_value(current.next_event().unwrap()).unwrap();
            assert_eq!(current_event["runId"], "current");
            sessions.cancel(&id("current"));
            drop(current);
            assert!(sessions.current.load_full().is_none());
        });
    }

    #[test]
    fn requests_require_nonempty_run_identity() {
        for invalid in [
            serde_json::Value::Null,
            serde_json::json!(""),
            serde_json::json!(0),
        ] {
            assert!(serde_json::from_value::<ValidationRunId>(invalid).is_err());
        }
        assert!(
            serde_json::from_value::<crate::protocol::commands::CancelValidationRequest>(
                serde_json::json!({})
            )
            .is_err()
        );
        let request: crate::protocol::commands::CancelValidationRequest =
            serde_json::from_value(serde_json::json!({"runId": "requested"})).unwrap();
        assert_eq!(request.run_id, id("requested"));
    }
}
