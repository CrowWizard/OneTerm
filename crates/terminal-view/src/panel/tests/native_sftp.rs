//! Native capability publication uses the real panel/Space active-state seam.

use super::*;
use oneterm_core::{FileEntry, RemotePath, SftpBackend, SftpFuture, SftpSessionId, TransferHandle};
use oneterm_terminal::{SessionKind, SharedSessionState, TerminalCapabilities};

struct Backend(SftpSessionId);
fn unused<T: Send + 'static>() -> SftpFuture<'static, T> {
    Box::pin(async { Err(AppError::msg("publication fixture does not perform I/O")) })
}
impl SftpBackend for Backend {
    fn session_id(&self) -> SftpSessionId {
        self.0
    }
    fn read_dir(&self, _: RemotePath) -> SftpFuture<'_, Vec<FileEntry>> {
        unused()
    }
    fn stat(&self, _: RemotePath) -> SftpFuture<'_, FileEntry> {
        unused()
    }
    fn realpath(&self, _: RemotePath) -> SftpFuture<'_, RemotePath> {
        unused()
    }
    fn rename(&self, _: RemotePath, _: RemotePath) -> SftpFuture<'_, ()> {
        unused()
    }
    fn remove(&self, _: RemotePath) -> SftpFuture<'_, ()> {
        unused()
    }
    fn remove_dir_all(&self, _: RemotePath) -> SftpFuture<'_, ()> {
        unused()
    }
    fn mkdir(&self, _: RemotePath) -> SftpFuture<'_, ()> {
        unused()
    }
    fn upload(&self, _: u64, _: std::path::PathBuf, _: RemotePath) -> TransferHandle {
        TransferHandle::failed(AppError::Cancelled)
    }
    fn download(&self, _: u64, _: RemotePath, _: std::path::PathBuf) -> TransferHandle {
        TransferHandle::failed(AppError::Cancelled)
    }
    fn cancel_transfer(&self, _: u64) {}
    fn close(&self) {}
    fn alive(&self) -> bool {
        true
    }
}

#[gpui::test]
fn native_sftp_capabilities_follow_active_space_and_workspace(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    cx.update(gpui_component::init);
    cx.update(crate::init);
    cx.update(oneterm_settings::TerminalSettings::init);
    cx.update(oneterm_state::AppState::init);
    let backend: Arc<dyn SftpBackend> = Arc::new(Backend(SftpSessionId::next()));
    let state = SharedSessionState::new_alive();
    let capabilities = TerminalCapabilities {
        sftp: Some(backend.clone()),
        cwd_source: Some(state.clone()),
        ..Default::default()
    };
    let (session, events, _) = crate::native_surface::tests::native_capability_fixture(
        b"remote",
        SessionKind::Ssh,
        capabilities,
    );
    let handed = Rc::new(RefCell::new(None));
    let handoff = handed.clone();
    let (_root, cx) = cx.add_window_view(move |window, cx| {
        let dock = cx.new(|cx| DockArea::new("native-sftp-publication", None, window, cx));
        let workspace = oneterm_state::AppState::global(cx)
            .update(cx, |state, _| state.register_workspace(&dock.downgrade()));
        let panel = TerminalPanel::open(
            PanelSpec::NativeSession {
                session,
                title: "SSH".into(),
                duplicate_config: None,
            },
            window,
            cx,
        );
        *handoff.borrow_mut() = Some((panel.clone(), workspace, dock));
        Root::new(panel, window, cx)
    });
    let (panel, workspace, _dock) = handed.borrow().clone().unwrap();
    panel.update_in(cx, |panel, window, cx| {
        let remote = panel.tree.active();
        panel.publish_active_session(cx);
        let active = oneterm_state::AppState::global(cx)
            .read(cx)
            .active_workspace(Some(workspace));
        assert!(!active.active_is_local);
        assert!(Arc::ptr_eq(active.active_sftp.as_ref().unwrap(), &backend));
        assert!(Arc::ptr_eq(
            active.active_cwd_source.as_ref().unwrap(),
            &state
        ));
        panel.split_active_at(remote, SplitDir::Right, window, cx);
        panel.publish_active_session(cx);
        let empty = panel.tree.active();
        let active = oneterm_state::AppState::global(cx)
            .read(cx)
            .active_workspace(Some(workspace));
        assert!(active.active_sftp.is_none());
        assert!(active.active_cwd_source.is_none());
        let (local, local_events, _) =
            crate::native_surface::tests::native_duplicate_fixture(b"local");
        let pane = TerminalPanel::new_native_view(&panel.deps, local, None, window, cx).unwrap();
        panel.tree.fill_empty(empty, pane.clone()).unwrap();
        panel.attach_split_ctx(&pane, empty, cx);
        panel.publish_active_session(cx);
        let active = oneterm_state::AppState::global(cx)
            .read(cx)
            .active_workspace(Some(workspace));
        assert!(active.active_is_local && active.active_sftp.is_none());
        panel.close_space(empty, window, cx);
        panel.publish_active_session(cx);
        let active = oneterm_state::AppState::global(cx)
            .read(cx)
            .active_workspace(Some(workspace));
        assert_eq!(
            active.active_sftp.unwrap().session_id(),
            backend.session_id()
        );
        let other = cx.new(|_| ()).entity_id();
        assert!(
            oneterm_state::AppState::global(cx)
                .read(cx)
                .active_workspace(Some(other))
                .active_sftp
                .is_none()
        );
        panel.shutdown(window, cx);
        drop(local_events);
    });
    drop(events);
    cx.run_until_parked();
}
