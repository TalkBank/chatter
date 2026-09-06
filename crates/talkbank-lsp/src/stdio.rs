//! Bind tower-lsp's protocol lifecycle to transport and process termination.
//!
//! tower-lsp 0.20 closes its service on `exit`, but its transport reader remains
//! parked until another frame or EOF. Observe completed protocol transitions at
//! the service boundary so an editor need not close stdin to stop the process.

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::sync::watch;
use tower_lsp::jsonrpc::{Request, Response};
use tower_lsp::{ExitedError, LspService, Server};
use tower_service::Service;

use crate::backend::Backend;

#[derive(Clone, Copy)]
enum SessionState {
    Running,
    ShutdownCompleted,
    ExitedAfterShutdown,
    ExitedWithoutShutdown,
}

impl SessionState {
    fn complete_shutdown(&mut self) {
        if matches!(self, Self::Running) {
            *self = Self::ShutdownCompleted;
        }
    }

    fn exit(&mut self) {
        *self = match self {
            Self::ShutdownCompleted | Self::ExitedAfterShutdown => Self::ExitedAfterShutdown,
            Self::Running | Self::ExitedWithoutShutdown => Self::ExitedWithoutShutdown,
        };
    }

    fn has_exited(&self) -> bool {
        matches!(
            self,
            Self::ExitedAfterShutdown | Self::ExitedWithoutShutdown
        )
    }
}

struct LifecycleService {
    inner: LspService<Backend>,
    state: watch::Sender<SessionState>,
}

impl Service<Request> for LifecycleService {
    type Response = Option<Response>;
    type Error = ExitedError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: Request) -> Self::Future {
        let shutdown = request.method() == "shutdown" && request.id().is_some();
        let exit = request.method() == "exit" && request.id().is_none();
        let future = self.inner.call(request);
        let state = self.state.clone();
        Box::pin(async move {
            let response = future.await?;
            if shutdown
                && response
                    .as_ref()
                    .is_some_and(|reply| reply.error().is_none())
            {
                // A rejected shutdown cannot authorize a successful process exit.
                state.send_modify(SessionState::complete_shutdown);
            } else if exit && response.is_none() {
                state.send_modify(SessionState::exit);
            }
            Ok(response)
        })
    }
}

pub(super) async fn serve() -> std::io::Result<()> {
    let (inner, socket) = LspService::new(Backend::new);
    let (state, mut lifecycle) = watch::channel(SessionState::Running);
    let service = LifecycleService { inner, state };
    let server = Server::new(tokio::io::stdin(), tokio::io::stdout(), socket).serve(service);
    tokio::select! {
        _ = server => {},
        _ = lifecycle.wait_for(SessionState::has_exited) => {},
    }
    // Read the final state even when EOF and exit become ready together.
    match *lifecycle.borrow() {
        SessionState::ExitedWithoutShutdown => Err(std::io::Error::other(
            "LSP exit received without a successful shutdown request",
        )),
        SessionState::Running
        | SessionState::ShutdownCompleted
        | SessionState::ExitedAfterShutdown => Ok(()),
    }
}
