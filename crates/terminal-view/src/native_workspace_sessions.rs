//! Test-workspace shutdown collects weak pane owners, never session controls.

use gpui::{App, Global, WeakEntity};
use oneterm_terminal::TerminalError;
use std::sync::{Arc, Mutex, PoisonError};

use crate::{NativePaneCompletion, NativeTerminalPane};

#[derive(Default)]
pub struct NativeWorkspaceSessions {
    panes: Vec<(WeakEntity<NativeTerminalPane>, NativePaneCompletion)>,
    closing: bool,
    failure: Option<String>,
    connections: Vec<(oneterm_core::ConnectionCancellation, ConnectionCompletion)>,
    unplaced: Vec<NativePaneCompletion>,
}

#[derive(Clone, Default)]
pub struct ConnectionCompletion(Arc<Mutex<Option<Result<(), TerminalError>>>>);

impl ConnectionCompletion {
    fn finish(&self, result: Result<(), TerminalError>) {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = Some(result);
    }
    fn result(&self) -> Option<Result<(), TerminalError>> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

/// A connection attempt is not a native owner and cannot fabricate its report.
pub enum NativeWorkspaceCompletion {
    Pane(NativePaneCompletion),
    Connection(ConnectionCompletion),
}
impl NativeWorkspaceCompletion {
    pub fn result(&self) -> Option<Result<(), TerminalError>> {
        match self {
            Self::Pane(done) => done.result().map(|result| result.map(|_| ())),
            Self::Connection(done) => done.result(),
        }
    }
}

/// A pending connection remains a close obligation until placement or cleanup.
pub struct NativeConnectionGuard {
    completion: ConnectionCompletion,
    finished: bool,
}

impl NativeConnectionGuard {
    pub fn finish(mut self) {
        self.completion.finish(Ok(()));
        self.finished = true;
    }

    pub async fn close_unplaced(
        mut self,
        session: oneterm_terminal::CreatedSession,
        executor: gpui::BackgroundExecutor,
    ) {
        let result = match session {
            oneterm_terminal::CreatedSession::Legacy(session) => session.close(),
            oneterm_terminal::CreatedSession::Native(session) => {
                let completion = crate::native_pane::close_unplaced(session, executor.clone());
                loop {
                    if let Some(result) = completion.result() {
                        break result.map(|_| ());
                    }
                    executor.timer(std::time::Duration::from_millis(1)).await;
                }
            }
        };
        self.completion.finish(result);
        self.finished = true;
    }
}

impl Drop for NativeConnectionGuard {
    fn drop(&mut self) {
        if !self.finished {
            let error = oneterm_terminal::TerminalError::Transport(
                "Native SSH connection observation cancelled".into(),
            );
            log::error!("{error}");
            self.completion.finish(Err(error));
        }
    }
}

impl Global for NativeWorkspaceSessions {}

impl NativeWorkspaceSessions {
    pub fn begin_connection(
        cancellation: oneterm_core::ConnectionCancellation,
        cx: &mut App,
    ) -> Option<NativeConnectionGuard> {
        if !cx.has_global::<Self>() {
            return None;
        }
        let group = cx.global_mut::<Self>();
        if group.closing {
            cancellation.cancel();
        }
        let completion = ConnectionCompletion::default();
        group.connections.push((cancellation, completion.clone()));
        Some(NativeConnectionGuard {
            completion,
            finished: false,
        })
    }
    /// Explicit native-entry opt-in; ordinary workspaces do not install it.
    pub fn install(cx: &mut App) {
        assert!(
            !cx.has_global::<Self>(),
            "native workspace lifecycle installed twice"
        );
        cx.set_global(Self::default());
    }

    pub(crate) fn accepting(cx: &App) -> bool {
        cx.try_global::<Self>().is_none_or(|group| !group.closing)
    }

    pub(crate) fn register(
        pane: WeakEntity<NativeTerminalPane>,
        completion: NativePaneCompletion,
        cx: &mut App,
    ) {
        if !cx.has_global::<Self>() {
            return;
        }
        let group = cx.global_mut::<Self>();
        group.collect();
        group.panes.push((pane, completion));
    }

    pub(crate) fn register_unplaced(completion: NativePaneCompletion, cx: &mut App) {
        if !cx.has_global::<Self>() {
            return;
        }
        let group = cx.global_mut::<Self>();
        group.collect();
        group.unplaced.push(completion);
    }

    fn collect(&mut self) {
        self.unplaced
            .retain(|completion| match completion.result() {
                Some(Ok(_)) => false,
                Some(Err(error)) => {
                    self.failure.get_or_insert_with(|| error.to_string());
                    false
                }
                None => true,
            });
        self.connections
            .retain(|(_, completion)| match completion.result() {
                Some(Ok(_)) => false,
                Some(Err(error)) => {
                    self.failure.get_or_insert_with(|| error.to_string());
                    false
                }
                None => true,
            });
        self.panes
            .retain(|(_, completion)| match completion.result() {
                Some(Ok(_)) => false,
                Some(Err(error)) => {
                    self.failure.get_or_insert_with(|| error.to_string());
                    false
                }
                None => true,
            });
    }

    /// Stops new creation and releases all live surface/control handles.
    /// Call repeatedly on window close or forced application quit; no retry of
    /// admitted backend operations is introduced by repeating this collection.
    pub fn shutdown(cx: &mut App) {
        let panes = {
            let group = cx.global_mut::<Self>();
            group.closing = true;
            for (cancellation, _) in &group.connections {
                cancellation.cancel();
            }
            group
                .panes
                .iter()
                .map(|(pane, _)| pane.clone())
                .collect::<Vec<_>>()
        };
        for pane in panes {
            if let Some(pane) = pane.upgrade() {
                pane.update(cx, |pane, cx| pane.shutdown(cx));
            }
        }
    }

    /// Pending is not success; an earlier closed pane's error stays visible.
    /// Snapshot completion-only handles before GPUI's shutdown executor holds
    /// its App borrow. Waiting must not re-enter App through AsyncApp::update.
    pub fn completion_snapshot(cx: &mut App) -> (Option<String>, Vec<NativeWorkspaceCompletion>) {
        let group = cx.global_mut::<Self>();
        group.collect();
        (
            group.failure.clone(),
            group
                .panes
                .iter()
                .map(|(_, done)| NativeWorkspaceCompletion::Pane(done.clone()))
                .chain(
                    group
                        .unplaced
                        .iter()
                        .cloned()
                        .map(NativeWorkspaceCompletion::Pane),
                )
                .chain(
                    group
                        .connections
                        .iter()
                        .map(|(_, done)| NativeWorkspaceCompletion::Connection(done.clone())),
                )
                .collect(),
        )
    }

    /// Pending is not success; an earlier closed pane's error stays visible.
    pub fn result(cx: &mut App) -> Option<Result<(), String>> {
        let group = cx.global_mut::<Self>();
        group.collect();
        if let Some(error) = &group.failure {
            Some(Err(error.clone()))
        } else if group.closing
            && group.panes.is_empty()
            && group.connections.is_empty()
            && group.unplaced.is_empty()
        {
            Some(Ok(()))
        } else {
            None
        }
    }
}
