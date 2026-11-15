//! Application native lifecycle around the direct SSH channel owner.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use oneterm_terminal::ghostty_session::{self, NativeSession, SurfaceSession};
use oneterm_terminal::{SessionEvent, SessionKind, TerminalError, TerminalLifecycle};

use crate::ghostty_channel::{LifecycleDelivery, NativeChannel};

pub(crate) struct NativeSshSession {
    endpoint: NativeChannel,
    events: Mutex<Option<async_channel::Receiver<SessionEvent>>>,
}

impl NativeSshSession {
    pub(crate) fn new(
        endpoint: NativeChannel,
        events: async_channel::Receiver<SessionEvent>,
    ) -> Self {
        Self {
            endpoint,
            events: Mutex::new(Some(events)),
        }
    }
}

impl oneterm_terminal::ghostty_content::RenderSource for NativeSshSession {
    fn render_model(&self) -> oneterm_terminal::ghostty_model::SharedModel {
        self.endpoint.model()
    }
}

impl SurfaceSession for NativeSshSession {
    fn clear_command(&self) -> ghostty_session::PasteFuture<'_> {
        SurfaceSession::clear_command(&self.endpoint)
    }
    fn set_default_colors(
        &self,
        colors: oneterm_terminal::ghostty::colors::DefaultColors,
    ) -> ghostty_session::ColorFuture<'_> {
        SurfaceSession::set_default_colors(&self.endpoint, colors)
    }
    fn resize(&self, size: ghostty_session::Size) -> Result<(), ghostty_session::InputError> {
        SurfaceSession::resize(&self.endpoint, size)
    }
    fn key(
        &self,
        key: oneterm_terminal::ghostty::input::KeyEvent,
    ) -> Result<(), ghostty_session::InputError> {
        SurfaceSession::key(&self.endpoint, key)
    }
    fn key_receipt(
        &self,
        key: oneterm_terminal::ghostty::input::KeyEvent,
    ) -> ghostty_session::KeyReceiptFuture<'_> {
        SurfaceSession::key_receipt(&self.endpoint, key)
    }
    fn mouse(
        &self,
        mouse: oneterm_terminal::ghostty::input::MouseEvent,
    ) -> Result<(), ghostty_session::InputError> {
        SurfaceSession::mouse(&self.endpoint, mouse)
    }
    fn wheel(
        &self,
        wheel: oneterm_terminal::ghostty::wheel::WheelEvent,
    ) -> Result<(), ghostty_session::InputError> {
        SurfaceSession::wheel(&self.endpoint, wheel)
    }
    fn write_visible<'a>(&'a self, bytes: &'a [u8]) -> ghostty_session::PasteFuture<'a> {
        SurfaceSession::write_visible(&self.endpoint, bytes)
    }
    fn paste_visible<'a>(&'a self, text: &'a str) -> ghostty_session::PasteFuture<'a> {
        SurfaceSession::paste_visible(&self.endpoint, text)
    }
    fn selection(
        &self,
        update: oneterm_terminal::ghostty::selection::Update,
    ) -> ghostty_session::PasteFuture<'_> {
        SurfaceSession::selection(&self.endpoint, update)
    }
    fn search(
        &self,
        needle: String,
        options: oneterm_terminal::ghostty::search::Options,
    ) -> ghostty_session::SearchFuture<'_> {
        SurfaceSession::search(&self.endpoint, needle, options)
    }
    fn search_step(&self) -> ghostty_session::SearchFuture<'_> {
        SurfaceSession::search_step(&self.endpoint)
    }
    fn reveal_search(
        &self,
        navigation: oneterm_terminal::ghostty::search::Navigation,
    ) -> ghostty_session::SearchFuture<'_> {
        SurfaceSession::reveal_search(&self.endpoint, navigation)
    }
    fn scroll_viewport(
        &self,
        scroll: oneterm_terminal::ghostty::state::Scroll,
    ) -> ghostty_session::PasteFuture<'_> {
        SurfaceSession::scroll_viewport(&self.endpoint, scroll)
    }
    fn release_repaint_hint(&self) {
        SurfaceSession::release_repaint_hint(&self.endpoint);
    }
    fn set_marked_text(&self, text: String) -> Result<(), oneterm_terminal::ghostty_ime::Error> {
        SurfaceSession::set_marked_text(&self.endpoint, text)
    }
    fn marked_text(&self) -> Option<String> {
        self.endpoint.marked_text()
    }
    fn clear_marked_text(&self) {
        self.endpoint.clear_marked_text();
    }
    fn commit_text(&self, text: &str) -> Result<(), oneterm_terminal::ghostty_ime::Error> {
        self.endpoint.commit_text(text)
    }
}

impl TerminalLifecycle for NativeSshSession {
    fn take_events(&self) -> Option<async_channel::Receiver<SessionEvent>> {
        self.events
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
    }
    fn alive(&self) -> bool {
        self.endpoint.state().alive()
    }
    fn close(&self) -> Result<(), TerminalError> {
        self.endpoint.close()
    }
    fn kind(&self) -> SessionKind {
        SessionKind::Ssh
    }
    fn title(&self) -> Option<String> {
        self.endpoint.state().title()
    }
    fn cwd(&self) -> Option<PathBuf> {
        self.endpoint.state().cwd()
    }
}

impl NativeSession for NativeSshSession {
    fn capabilities(&self) -> oneterm_terminal::TerminalCapabilities {
        let state = self.endpoint.state();
        oneterm_terminal::TerminalCapabilities {
            network_stats: Some(state.net_stats()),
            sftp: self.endpoint.sftp(),
            cwd_source: Some(state),
            logging: Some(self.endpoint.logging()),
        }
    }
    fn request_shutdown(&self) -> Result<(), TerminalError> {
        self.endpoint.request_shutdown()
    }
    fn reply_clipboard(&self, text: &str) -> Result<(), TerminalError> {
        self.endpoint.reply_clipboard(text)
    }
    fn complete(self: Arc<Self>) -> ghostty_session::CompletionFuture {
        Box::pin(async move {
            let session = Arc::try_unwrap(self).map_err(|session| {
                if let Err(error) = session.close() {
                    log::error!("Native SSH retained completion close: {error}");
                }
                TerminalError::Transport(format!(
                    "Native SSH completion requires released controls; {} references remain",
                    Arc::strong_count(&session)
                ))
            })?;
            let report = session
                .endpoint
                .wait()
                .await
                .map_err(|error| TerminalError::Transport(error.to_string()))?;
            Ok(ghostty_session::Completion {
                exit_code: report.exit_code,
                lifecycle_delivered: report.lifecycle == LifecycleDelivery::Complete,
            })
        })
    }
}
