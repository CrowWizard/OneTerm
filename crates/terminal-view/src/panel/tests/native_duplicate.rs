//! Actual panel duplication must preserve native identity and completion ownership.

use oneterm_core::ShellKind;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use super::*;
use crate::native_surface::tests::native_duplicate_fixture;
use crate::panel::DuplicateDestination;
use crate::panel::pane::TerminalPane;

struct Call {
    config: LocalShellConfig,
    size: PtySize,
    scrollback: usize,
    security: TerminalSecurityPolicy,
    logging: oneterm_core::TerminalLogConfig,
}

#[derive(Default)]
struct Factory {
    calls: Mutex<Vec<Call>>,
    completions: Mutex<Vec<Arc<AtomicUsize>>>,
    fail: AtomicBool,
}

impl SessionFactory for Factory {
    fn create_local(
        &self,
        config: LocalShellConfig,
        size: PtySize,
        scrollback: usize,
        security: TerminalSecurityPolicy,
        logging: oneterm_core::TerminalLogConfig,
    ) -> Result<oneterm_terminal::CreatedSession> {
        self.spawn_native_local(config, size, scrollback, security, logging)
            .map(oneterm_terminal::CreatedSession::Native)
    }

    fn spawn_local(
        &self,
        _: LocalShellConfig,
        _: PtySize,
        _: usize,
        _: TerminalSecurityPolicy,
        _: oneterm_core::TerminalLogConfig,
    ) -> Result<Box<dyn TerminalSession>> {
        panic!("native duplication must never call the legacy factory")
    }

    fn spawn_native_local(
        &self,
        config: LocalShellConfig,
        size: PtySize,
        scrollback: usize,
        security: TerminalSecurityPolicy,
        logging: oneterm_core::TerminalLogConfig,
    ) -> Result<Arc<dyn oneterm_terminal::ghostty_session::NativeSession>> {
        self.calls.lock().unwrap().push(Call {
            config,
            size,
            scrollback,
            security,
            logging,
        });
        if self.fail.load(Ordering::Relaxed) {
            return Err(AppError::msg("native duplicate fixture creation rejected"));
        }
        let (session, events, completed) = native_duplicate_fixture(b"duplicate");
        drop(events);
        self.completions.lock().unwrap().push(completed);
        Ok(session)
    }

    fn connect_ssh(
        &self,
        _: SshConfig,
        _: PtySize,
        _: usize,
        _: TerminalSecurityPolicy,
        _: oneterm_core::TerminalLogConfig,
        _: oneterm_core::SshKeepaliveConfig,
    ) -> Result<Box<dyn TerminalSession>> {
        panic!("native duplication must not enter the legacy SSH factory")
    }
}

fn pump_until(cx: &mut VisualTestContext, mut ready: impl FnMut(&mut VisualTestContext) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        cx.executor().advance_clock(Duration::from_millis(20));
        cx.run_until_parked();
        if ready(cx) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "native duplication proof deadline"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[gpui::test]
fn native_actual_context_menu_exposes_and_executes_all_split_directions(cx: &mut TestAppContext) {
    use gpui::{MouseButton, MouseDownEvent, point, px};
    cx.executor().allow_parking();
    cx.update(gpui_component::init);
    cx.update(crate::init);
    cx.update(oneterm_settings::TerminalSettings::init);
    cx.update(oneterm_state::AppState::init);
    let (session, events, original_completed) = native_duplicate_fixture(b"native menu");
    let handed = Rc::new(RefCell::new(None));
    let handoff = handed.clone();
    let (_root, cx) = cx.add_window_view(move |window, cx| {
        let panel = TerminalPanel::open(
            PanelSpec::NativeSession {
                session,
                title: "Native menu".into(),
                duplicate_config: None,
            },
            window,
            cx,
        );
        *handoff.borrow_mut() = Some(panel.clone());
        Root::new(panel, window, cx)
    });
    let panel = handed.borrow().clone().unwrap();
    let original = panel.read_with(cx, |panel, _| panel.tree.active());
    for label in ["Split Right", "Split Left", "Split Up", "Split Down"] {
        cx.run_until_parked();
        cx.update(|window, cx| {
            _ = window.draw(cx);
        });
        cx.simulate_event(MouseDownEvent {
            button: MouseButton::Right,
            position: point(px(40.), px(40.)),
            modifiers: Default::default(),
            click_count: 1,
            first_mouse: false,
        });
        cx.run_until_parked();
        cx.update(|window, cx| {
            _ = window.draw(cx);
        });
        let at = cx.update(|window, _| {
            let items = gpui_base::test_support::snapshots(window);
            assert!(
                !items.iter().any(|item| {
                    item.role() == Some(gpui::Role::MenuItem) && item.label() == Some("Close Space")
                }),
                "single-Space context menu must omit Close Space"
            );
            items
                .into_iter()
                .find(|item| {
                    item.role() == Some(gpui::Role::MenuItem) && item.label() == Some(label)
                })
                .unwrap_or_else(|| panic!("missing native menu row {label}"))
                .bounds()
                .center()
        });
        cx.simulate_click(at, Default::default());
        cx.run_until_parked();
        panel.read_with(cx, |panel, _| {
            assert_eq!(panel.leaf_count(), 2, "{label} must route into the panel");
            assert!(panel.tree.active_terminal().is_none());
            let empty = panel.tree.active();
            assert_ne!(empty, original);
        });
        cx.update(|window, cx| {
            _ = window.draw(cx);
        });
        let empty_at = cx.update(|window, _| {
            let size = window.viewport_size();
            match label {
                "Split Right" => point(size.width * 0.75, size.height * 0.5),
                "Split Left" => point(size.width * 0.25, size.height * 0.5),
                "Split Up" => point(size.width * 0.5, size.height * 0.25),
                "Split Down" => point(size.width * 0.5, size.height * 0.75),
                _ => unreachable!(),
            }
        });
        click_context_item(cx, empty_at, "Close Space");
        panel.read_with(cx, |panel, _| {
            assert_eq!(panel.leaf_count(), 1, "Close Space collapses the split");
            assert_eq!(panel.tree.active(), original);
            assert!(matches!(
                panel.tree.active_terminal(),
                Some(TerminalPane::Native(_))
            ));
        });
    }
    cx.update(|_, cx| {
        cx.bind_keys([
            gpui::KeyBinding::new("ctrl-shift-right", oneterm_actions::SplitRight, None),
            gpui::KeyBinding::new("ctrl-shift-left", oneterm_actions::SplitLeft, None),
            gpui::KeyBinding::new("ctrl-shift-up", oneterm_actions::SplitUp, None),
            gpui::KeyBinding::new("ctrl-shift-down", oneterm_actions::SplitDown, None),
        ]);
    });
    for keys in [
        "ctrl-shift-right",
        "ctrl-shift-left",
        "ctrl-shift-up",
        "ctrl-shift-down",
    ] {
        panel.update_in(cx, |panel, window, cx| panel.focus_active_space(window, cx));
        cx.update(|window, cx| {
            _ = window.draw(cx);
        });
        cx.simulate_keystrokes(keys);
        cx.run_until_parked();
        panel.update_in(cx, |panel, window, cx| {
            assert_eq!(panel.leaf_count(), 2, "native shortcut {keys}");
            panel.close_space(panel.tree.active(), window, cx);
        });
    }
    let (other, other_events, other_completed) = native_duplicate_fixture(b"move me");
    let source = cx.update(|window, cx| {
        TerminalPanel::open(
            PanelSpec::NativeSession {
                session: other,
                title: "Source".into(),
                duplicate_config: None,
            },
            window,
            cx,
        )
    });
    let identity = source.read_with(cx, |panel, _| panel.tree.active_terminal().unwrap().id());
    let target = panel.update_in(cx, |panel, window, cx| {
        panel.split_active_at(original, SplitDir::Right, window, cx);
        panel.tree.active()
    });
    cx.update(|window, cx| {
        _ = window.draw(cx);
    });
    let target_at = cx.update(|window, _| {
        let size = window.viewport_size();
        point(size.width * 0.75, size.height * 0.5)
    });
    panel.update_in(cx, |panel, window, cx| {
        panel.move_tab_into(target, source.clone(), window, cx);
        assert_eq!(panel.tree.active_terminal().unwrap().id(), identity);
        assert!(source.read(cx).has_no_terminals());
        panel.set_active_space(original, window, cx);
    });
    click_context_item(cx, target_at, "Close Space");
    drop(other_events);
    pump_until(cx, |_| other_completed.load(Ordering::Relaxed) == 1);
    panel.read_with(cx, |panel, _| {
        assert_eq!(panel.leaf_count(), 1);
        assert_eq!(
            panel.tree.active(),
            original,
            "close targets the right-clicked pane"
        );
        assert_ne!(panel.tree.active_terminal().unwrap().id(), identity);
        assert!(panel.tree.is_single());
    });
    assert_eq!(
        original_completed.load(Ordering::Relaxed),
        0,
        "sibling stays alive"
    );
    panel.update_in(cx, |panel, window, cx| panel.shutdown(window, cx));
    drop(events);
}

fn click_context_item(cx: &mut VisualTestContext, at: gpui::Point<gpui::Pixels>, label: &str) {
    cx.run_until_parked();
    cx.update(|window, cx| {
        _ = window.draw(cx);
    });
    cx.simulate_event(gpui::MouseDownEvent {
        button: gpui::MouseButton::Right,
        position: at,
        modifiers: Default::default(),
        click_count: 1,
        first_mouse: false,
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        _ = window.draw(cx);
    });
    let at = cx.update(|window, _| {
        gpui_base::test_support::snapshots(window)
            .into_iter()
            .find(|item| item.role() == Some(gpui::Role::MenuItem) && item.label() == Some(label))
            .unwrap_or_else(|| panic!("missing actual context menu item {label}"))
            .bounds()
            .center()
    });
    cx.simulate_click(at, Default::default());
    cx.run_until_parked();
}

#[gpui::test]
fn native_startup_default_shell_explicit_shell_empty_space_and_workspace_shutdown(
    cx: &mut TestAppContext,
) {
    cx.executor().allow_parking();
    cx.update(gpui_component::init);
    cx.update(crate::init);
    cx.update(oneterm_settings::TerminalSettings::init);
    cx.update(oneterm_state::AppState::init);
    cx.update(crate::NativeWorkspaceSessions::install);
    let factory = Arc::new(Factory::default());
    let services = factory.clone();
    cx.update(|cx| {
        AppServices::install(
            cx,
            services,
            duplicate_test_commands(),
            crate::status_metrics(),
        )
        .unwrap();
        oneterm_settings::TerminalSettings::global(cx).update(cx, |settings, _| {
            settings.scrollback_history = 123;
            settings.shell.kind = ShellKind::Pwsh;
            settings.shell.program = Some(r"D:\Program Files\PowerShell\7\pwsh.exe".into());
        });
    });
    let panels = Rc::new(RefCell::new(Vec::new()));
    let handed = panels.clone();
    let (_root, cx) = cx.add_window_view(move |window, cx| {
        let default = TerminalPanel::open(PanelSpec::DefaultShell { workspace: None }, window, cx);
        let explicit = TerminalPanel::open(PanelSpec::Shell(ShellKind::Cmd), window, cx);
        let pwsh = TerminalPanel::open(PanelSpec::Shell(ShellKind::Pwsh), window, cx);
        *handed.borrow_mut() = vec![default.clone(), explicit, pwsh];
        Root::new(default, window, cx)
    });
    let panels = panels.borrow().clone();
    for panel in &panels {
        panel.read_with(cx, |panel, cx| {
            let pane = panel.tree.active_terminal().unwrap();
            assert!(matches!(pane, TerminalPane::Native(_)));
            assert!(panel.active_view().is_none());
            assert_eq!(pane.kind(cx), Some(oneterm_terminal::SessionKind::Local));
        });
    }
    panels[0].update_in(cx, |panel, window, cx| {
        panel.split_active_at(panel.tree.active(), SplitDir::Right, window, cx);
        assert!(panel.tree.active_terminal().is_none());
        panel.new_terminal_here(panel.tree.active(), window, cx);
        assert!(matches!(
            panel.tree.active_terminal(),
            Some(TerminalPane::Native(_))
        ));
    });
    assert_eq!(factory.calls.lock().unwrap().len(), 4);
    assert!(
        factory
            .calls
            .lock()
            .unwrap()
            .iter()
            .all(|call| call.scrollback == 123)
    );
    assert_eq!(factory.calls.lock().unwrap()[1].config.kind, ShellKind::Cmd);
    {
        let calls = factory.calls.lock().unwrap();
        assert!(
            calls[1].config.program.is_none(),
            "Cmd must not inherit pwsh.exe"
        );
        for index in [0, 2, 3] {
            assert_eq!(calls[index].config.kind, ShellKind::Pwsh);
            assert_eq!(
                calls[index].config.program.as_deref(),
                Some(std::path::Path::new(
                    r"D:\Program Files\PowerShell\7\pwsh.exe"
                ))
            );
        }
    }
    cx.update(|_, cx| {
        assert!(crate::NativeWorkspaceSessions::result(cx).is_none());
        crate::NativeWorkspaceSessions::shutdown(cx);
        assert!(!crate::NativeWorkspaceSessions::accepting(cx));
        crate::NativeWorkspaceSessions::shutdown(cx);
    });
    pump_until(cx, |cx| {
        cx.update(|_, cx| crate::NativeWorkspaceSessions::result(cx))
            .is_some()
    });
    assert_eq!(
        cx.update(|_, cx| crate::NativeWorkspaceSessions::result(cx)),
        Some(Ok(()))
    );
    assert!(
        factory
            .completions
            .lock()
            .unwrap()
            .iter()
            .all(|done| done.load(Ordering::Relaxed) == 1)
    );
}

#[gpui::test]
fn native_channel_actual_panel_menu_and_tab_wide_membership_use_pane_identity(
    cx: &mut TestAppContext,
) {
    use crate::panel::channel_menu::ChannelMenuItem;
    cx.executor().allow_parking();
    cx.update(gpui_component::init);
    cx.update(crate::init);
    cx.update(oneterm_settings::TerminalSettings::init);
    cx.update(oneterm_state::AppState::init);
    cx.update(InputChannelRegistry::init);
    let (session, events, done) = native_duplicate_fixture(b"first");
    let (other, other_events, other_done) = native_duplicate_fixture(b"second");
    let handed = Rc::new(RefCell::new(None));
    let handoff = handed.clone();
    let (_root, cx) = cx.add_window_view(move |window, cx| {
        let panel = cx.new(|cx| {
            TerminalPanel::from_spec(
                PanelSpec::NativeSession {
                    session,
                    title: "Native channels".into(),
                    duplicate_config: None,
                },
                window,
                cx,
            )
        });
        *handoff.borrow_mut() = Some(panel.clone());
        Root::new(panel, window, cx)
    });
    let panel = handed.borrow().clone().unwrap();
    panel.update_in(cx, |panel, window, cx| {
        let first = panel.tree.active();
        let first_id = panel.tree.active_terminal().unwrap().id();
        panel.apply_channel_item(ChannelMenuItem::Join(InputChannel::A, false), cx);
        assert!(
            panel
                .channel_entries(cx)
                .unwrap()
                .contains(&ChannelMenuItem::Join(InputChannel::A, true))
        );
        panel.split_active_at(first, SplitDir::Right, window, cx);
        let empty = panel.tree.active();
        let second = TerminalPanel::new_native_view(&panel.deps, other, None, window, cx).unwrap();
        panel.tree.fill_empty(empty, second.clone()).unwrap();
        panel.attach_split_ctx(&second, empty, cx);
        panel.apply_channel_item(ChannelMenuItem::Join(InputChannel::C, false), cx);
        assert_eq!(panel.tab_channels(cx), [InputChannel::A, InputChannel::C]);
        panel.apply_channel_item(ChannelMenuItem::JoinTab(InputChannel::C), cx);
        assert_eq!(panel.tab_channels(cx), [InputChannel::C]);
        let registry = panel.deps.input_channels.as_ref().unwrap();
        assert_eq!(
            registry.read(cx).channel_of(first_id),
            Some(InputChannel::C)
        );
        assert_eq!(
            registry.read(cx).channel_of(second.id()),
            Some(InputChannel::C)
        );
        panel.close_active_channel(cx);
        assert!(panel.tab_channels(cx).is_empty());
        panel.apply_channel_item(ChannelMenuItem::Join(InputChannel::B, false), cx);
        panel.apply_channel_item(ChannelMenuItem::Leave, cx);
        assert!(panel.tab_channels(cx).is_empty());
        panel.join_tab_to_channel(InputChannel::D, cx);
        panel.apply_channel_item(ChannelMenuItem::LeaveTab, cx);
        assert!(panel.tab_channels(cx).is_empty());
    });
    other_events
        .send_blocking(oneterm_terminal::SessionEvent::Exited(Some(0)))
        .unwrap();
    pump_until(cx, |cx| {
        panel.read_with(cx, |panel, cx| panel.channel_entries(cx).is_none())
    });
    panel.update_in(cx, |panel, window, cx| {
        panel.join_tab_to_channel(InputChannel::E, cx);
        let registry = panel.deps.input_channels.as_ref().unwrap();
        assert_eq!(
            registry.read(cx).members(InputChannel::E).len(),
            1,
            "exited native Space cannot join"
        );
        panel.shutdown(window, cx);
    });
    drop((events, other_events));
    pump_until(cx, |_| {
        done.load(Ordering::Relaxed) == 1 && other_done.load(Ordering::Relaxed) == 1
    });
}

#[gpui::test]
fn native_duplicate_preserves_engine_config_destinations_and_failed_placement_completion(
    cx: &mut TestAppContext,
) {
    cx.executor().allow_parking();
    cx.update(gpui_component::init);
    cx.update(crate::init);
    cx.update(oneterm_settings::TerminalSettings::init);
    cx.update(oneterm_state::AppState::init);
    let factory = Arc::new(Factory::default());
    let services = factory.clone();
    cx.update(|cx| {
        AppServices::install(
            cx,
            services,
            duplicate_test_commands(),
            crate::status_metrics(),
        )
        .unwrap();
        oneterm_settings::TerminalSettings::global(cx).update(cx, |settings, _| {
            settings.scrollback_history = 321;
            settings.allow_clipboard_read = true;
        });
    });
    let (session, events, source_completion) = native_duplicate_fixture(b"source");
    let mut config = LocalShellConfig::default();
    config.program = Some("recorded-shell".into());
    config.args = vec!["--login".into()];
    config
        .env
        .insert("NATIVE_DUPLICATE_TEST".into(), "preserved".into());
    config.utf8 = false;
    config.cwd = Some("old-configured-cwd".into());
    let handed = Rc::new(RefCell::new(None));
    let handoff = handed.clone();
    let (_root, cx) = cx.add_window_view(move |window, cx| {
        let panel = cx.new(|cx| {
            TerminalPanel::from_spec(
                PanelSpec::NativeSession {
                    session,
                    title: "Source".into(),
                    duplicate_config: Some(SessionDuplicateConfig::Local(config)),
                },
                window,
                cx,
            )
        });
        *handoff.borrow_mut() = Some(panel.clone());
        Root::new(panel, window, cx)
    });
    let panel = handed.borrow().clone().unwrap();
    events
        .send_blocking(oneterm_terminal::SessionEvent::Cwd("live-cwd".into()))
        .unwrap();
    drop(events);
    pump_until(cx, |cx| {
        panel
            .read_with(cx, |panel, cx| panel.local_cwd(cx))
            .is_some()
    });
    let source = panel.read_with(cx, |panel, _| panel.tree.active());
    let source_id = panel.read_with(cx, |panel, _| panel.tree.active_terminal().unwrap().id());
    panel.update_in(cx, |panel, window, cx| {
        panel.duplicate_session_to(
            source,
            DuplicateDestination::Split(SplitDir::Right),
            window,
            cx,
        );
        let duplicate = panel.tree.active_terminal().unwrap();
        assert!(matches!(duplicate, TerminalPane::Native(_)));
        assert_ne!(duplicate.id(), source_id);
        assert_eq!(panel.tree.leaf_terminal(source).unwrap().id(), source_id);
        assert_eq!(panel.leaf_count(), 2);
        // Occupied Space and absent tab container both require native cleanup.
        panel.duplicate_session_to(
            source,
            DuplicateDestination::ExistingSpace(source),
            window,
            cx,
        );
        panel.duplicate_session_to(source, DuplicateDestination::NewTab, window, cx);
        assert_eq!(panel.leaf_count(), 2);
        panel.split_active_at(panel.tree.active(), SplitDir::Down, window, cx);
        let empty = panel.tree.active();
        panel.duplicate_session_to(
            source,
            DuplicateDestination::ExistingSpace(empty),
            window,
            cx,
        );
        assert!(matches!(
            panel.tree.leaf_terminal(empty),
            Some(TerminalPane::Native(_))
        ));
        assert_eq!(panel.tree.leaf_terminal(source).unwrap().id(), source_id);
    });
    pump_until(cx, |_| {
        let probes = factory.completions.lock().unwrap();
        probes.len() == 4
            && probes[1].load(Ordering::Relaxed) == 1
            && probes[2].load(Ordering::Relaxed) == 1
    });
    {
        let calls = factory.calls.lock().unwrap();
        assert_eq!(calls.len(), 4);
        for call in calls.iter() {
            assert_eq!(
                call.config.program.as_deref(),
                Some(std::path::Path::new("recorded-shell"))
            );
            assert_eq!(call.config.args, ["--login"]);
            assert_eq!(
                call.config
                    .env
                    .get("NATIVE_DUPLICATE_TEST")
                    .map(String::as_str),
                Some("preserved")
            );
            assert!(!call.config.utf8);
            assert_eq!(
                call.config.cwd.as_deref(),
                Some(std::path::Path::new("live-cwd"))
            );
            assert_eq!(call.size, PtySize::INITIAL);
            assert_eq!(call.scrollback, 321);
            assert!(call.security.allow_remote_clipboard_read);
            assert!(!call.security.allow_remote_clipboard_write);
            assert!(!call.logging.enabled);
        }
    }
    factory.fail.store(true, Ordering::Relaxed);
    let expected_logging = cx.update(|_, cx| {
        oneterm_settings::TerminalSettings::global(cx).update(cx, |settings, _| {
            settings.scrollback_history = 654;
            settings.allow_clipboard_read = false;
            settings.logging.local = true;
            settings.logging.directory = "native-duplicate-logs".into();
            settings.logging.write_mode = oneterm_core::LogWriteMode::Overwrite;
            settings.logging.local_config()
        })
    });
    panel.update_in(cx, |panel, window, cx| {
        let before = panel.leaf_count();
        panel.duplicate_session_to(
            source,
            DuplicateDestination::Split(SplitDir::Left),
            window,
            cx,
        );
        assert_eq!(panel.leaf_count(), before);
        assert_eq!(panel.tree.leaf_terminal(source).unwrap().id(), source_id);
        panel.shutdown(window, cx);
    });
    {
        let calls = factory.calls.lock().unwrap();
        assert_eq!(calls.len(), 5);
        assert_eq!(calls[4].scrollback, 654);
        assert!(!calls[4].security.allow_remote_clipboard_read);
        assert_eq!(calls[4].logging, expected_logging);
    }
    pump_until(cx, |_| {
        source_completion.load(Ordering::Relaxed) == 1
            && factory
                .completions
                .lock()
                .unwrap()
                .iter()
                .all(|probe| probe.load(Ordering::Relaxed) == 1)
    });
}

#[gpui::test]
fn native_duplicate_new_tab_uses_existing_dock_and_native_owner(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    cx.update(gpui_component::init);
    cx.update(crate::init);
    cx.update(oneterm_settings::TerminalSettings::init);
    cx.update(oneterm_state::AppState::init);
    let factory = Arc::new(Factory::default());
    let services = factory.clone();
    cx.update(|cx| {
        AppServices::install(
            cx,
            services,
            duplicate_test_commands(),
            crate::status_metrics(),
        )
        .unwrap()
    });
    let (session, events, source_done) = native_duplicate_fixture(b"source");
    drop(events);
    let handed = Rc::new(RefCell::new(None));
    let handoff = handed.clone();
    let (_root, cx) = cx.add_window_view(move |window, cx| {
        let dock = cx.new(|cx| DockArea::new("native-duplicate-tabs", None, window, cx));
        let panel = TerminalPanel::open(
            PanelSpec::NativeSession {
                session,
                title: "Source".into(),
                duplicate_config: Some(SessionDuplicateConfig::Local(LocalShellConfig::default())),
            },
            window,
            cx,
        );
        dock.update(cx, |dock, cx| {
            dock.set_center(
                DockLayout::tabs().panel_view(panel_handle(panel.clone()), cx),
                window,
                cx,
            )
        });
        oneterm_state::AppState::global(cx)
            .update(cx, |state, _| state.dock_area = Some(dock.downgrade()));
        let tabs = panel
            .read(cx)
            .tab_panel
            .as_ref()
            .unwrap()
            .upgrade()
            .unwrap();
        *handoff.borrow_mut() = Some((panel, tabs));
        Root::new(dock, window, cx)
    });
    let (source, tabs) = handed.borrow().clone().unwrap();
    source.update_in(cx, |panel, window, cx| {
        panel.duplicate_session(panel.tree.active(), window, cx)
    });
    assert_eq!(tabs.read_with(cx, |tabs, _| tabs.panels().len()), 2);
    let duplicate = tabs.read_with(cx, |tabs, _| {
        tabs.panels()[1].view().downcast::<TerminalPanel>().unwrap()
    });
    assert_ne!(source.entity_id(), duplicate.entity_id());
    duplicate.read_with(cx, |panel, _| {
        assert!(matches!(
            panel.tree.active_terminal(),
            Some(TerminalPane::Native(_))
        ))
    });
    source.update_in(cx, |panel, window, cx| panel.shutdown(window, cx));
    duplicate.update_in(cx, |panel, window, cx| panel.shutdown(window, cx));
    pump_until(cx, |_| {
        source_done.load(Ordering::Relaxed) == 1
            && factory.completions.lock().unwrap()[0].load(Ordering::Relaxed) == 1
    });
}

#[gpui::test]
fn native_duplicate_factory_default_rejects_without_legacy_fallback(cx: &mut TestAppContext) {
    let factory = DuplicateSessionFactory {
        spawned_local_configs: Arc::default(),
    };
    assert!(
        factory
            .spawn_native_local(
                LocalShellConfig::default(),
                PtySize::INITIAL,
                10,
                TerminalSecurityPolicy::default(),
                oneterm_core::TerminalLogConfig::default()
            )
            .is_err()
    );
    assert!(factory.spawned_local_configs.lock().unwrap().is_empty());
    cx.run_until_parked();
}

#[derive(Default)]
struct SshHandoff(Option<oneterm_state::commands::SshDuplicateCompletion>);
impl gpui::Global for SshHandoff {}

#[gpui::test]
fn native_ssh_duplicate_tag_survives_dialog_and_missing_destination(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    cx.update(gpui_component::init);
    cx.update(crate::init);
    cx.update(oneterm_settings::TerminalSettings::init);
    cx.update(oneterm_state::AppState::init);
    cx.update(|cx| {
        cx.set_global(SshHandoff::default());
        let mut commands = duplicate_test_commands();
        commands.open_duplicate_ssh_dialog = |config, cwd, completion, _, cx| {
            assert_eq!(config.host, "loopback.invalid");
            assert_eq!(cwd, None);
            cx.global_mut::<SshHandoff>().0 = Some(completion);
        };
        AppServices::install(
            cx,
            Arc::new(Factory::default()),
            commands,
            crate::status_metrics(),
        )
        .unwrap();
    });
    let cfg = SshConfig {
        host: "loopback.invalid".into(),
        port: 22,
        username: "user".into(),
        auth: oneterm_core::SshAuthMethod::None,
        cancellation: Default::default(),
        host_key_policy: Default::default(),
        shell_integration: false,
        jump_hops: vec![],
        port_forwards: vec![],
        agent_forwarding: false,
    };
    let config = SessionDuplicateConfig::Ssh(cfg.duplicate_config());
    let source_config = config.clone();
    let (source, events, source_done) = native_duplicate_fixture(b"ssh source");
    drop(events);
    // A separate entity can disappear while the containing window remains.
    let (_root, cx) = cx.add_window_view(|window, cx| {
        let dock = cx.new(|cx| DockArea::new("ssh-handoff", None, window, cx));
        Root::new(dock, window, cx)
    });
    let panel = cx.update(|window, cx| {
        cx.new(|cx| {
            TerminalPanel::from_spec(
                PanelSpec::NativeSession {
                    session: source,
                    title: "SSH".into(),
                    duplicate_config: Some(source_config),
                },
                window,
                cx,
            )
        })
    });
    panel.update_in(cx, |panel, window, cx| {
        panel.duplicate_session_to(
            panel.tree.active(),
            DuplicateDestination::Split(SplitDir::Right),
            window,
            cx,
        )
    });
    let callback = cx.update(|_, cx| cx.global_mut::<SshHandoff>().0.take().unwrap());
    let (created, events, created_done) = native_duplicate_fixture(b"ssh duplicate");
    drop(events);
    let duplicate_config = config.clone();
    cx.update(|window, cx| {
        callback(
            oneterm_terminal::CreatedSession::Native(created),
            "SSH duplicate".into(),
            duplicate_config,
            window,
            cx,
        )
    });
    assert_eq!(panel.read_with(cx, |p, _| p.leaf_count()), 2);
    panel.read_with(cx, |p, _| {
        assert!(matches!(
            p.tree.active_terminal(),
            Some(TerminalPane::Native(_))
        ))
    });
    panel.update_in(cx, |panel, window, cx| {
        panel.duplicate_session_to(
            panel.tree.active(),
            DuplicateDestination::Split(SplitDir::Down),
            window,
            cx,
        )
    });
    let late = cx.update(|_, cx| cx.global_mut::<SshHandoff>().0.take().unwrap());
    panel.update_in(cx, |panel, window, cx| panel.shutdown(window, cx));
    drop(panel);
    cx.run_until_parked();
    let (unplaced, events, unplaced_done) = native_duplicate_fixture(b"late ssh");
    drop(events);
    cx.update(|window, cx| {
        late(
            oneterm_terminal::CreatedSession::Native(unplaced),
            "late".into(),
            config,
            window,
            cx,
        )
    });
    pump_until(cx, |_| {
        source_done.load(Ordering::Relaxed) == 1
            && created_done.load(Ordering::Relaxed) == 1
            && unplaced_done.load(Ordering::Relaxed) == 1
    });
}

#[gpui::test]
fn native_pending_ssh_is_cancelled_and_awaited_at_workspace_close(cx: &mut TestAppContext) {
    use crate::NativeWorkspaceSessions;
    cx.update(NativeWorkspaceSessions::install);
    let cancellation = oneterm_core::ConnectionCancellation::default();
    let guard = cx
        .update(|cx| NativeWorkspaceSessions::begin_connection(cancellation.clone(), cx).unwrap());
    cx.update(NativeWorkspaceSessions::shutdown);
    assert!(cancellation.is_cancelled());
    assert!(cx.update(NativeWorkspaceSessions::result).is_none());
    let (_, pending) = cx.update(NativeWorkspaceSessions::completion_snapshot);
    assert_eq!(pending.len(), 1);
    assert!(pending[0].result().is_none());
    guard.finish();
    assert_eq!(cx.update(NativeWorkspaceSessions::result), Some(Ok(())));
}

#[gpui::test]
fn native_cancelled_ssh_observer_cannot_report_success(cx: &mut TestAppContext) {
    use crate::NativeWorkspaceSessions;
    cx.update(NativeWorkspaceSessions::install);
    let guard =
        cx.update(|cx| NativeWorkspaceSessions::begin_connection(Default::default(), cx).unwrap());
    drop(guard);
    cx.update(NativeWorkspaceSessions::shutdown);
    assert!(
        matches!(cx.update(NativeWorkspaceSessions::result), Some(Err(error)) if error.contains("observation cancelled"))
    );
}

#[gpui::test]
fn native_unplaced_cleanup_stays_in_close_snapshot_until_events_drain(cx: &mut TestAppContext) {
    use crate::NativeWorkspaceSessions;
    cx.executor().allow_parking();
    cx.update(NativeWorkspaceSessions::install);
    let (session, events, completed) = native_duplicate_fixture(b"late connection");
    cx.update(|cx| {
        crate::close_unplaced_session_in_workspace(
            oneterm_terminal::CreatedSession::Native(session),
            cx,
        )
    });
    cx.update(NativeWorkspaceSessions::shutdown);
    assert!(cx.update(NativeWorkspaceSessions::result).is_none());
    let (_, pending) = cx.update(NativeWorkspaceSessions::completion_snapshot);
    assert_eq!(pending.len(), 1);
    assert!(pending[0].result().is_none());
    drop(events);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while cx.update(NativeWorkspaceSessions::result).is_none() {
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(1));
        cx.run_until_parked();
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert_eq!(completed.load(Ordering::Relaxed), 1);
    assert_eq!(cx.update(NativeWorkspaceSessions::result), Some(Ok(())));
}

#[gpui::test]
fn native_unplaced_missing_event_receiver_cannot_disappear_from_close_proof(
    cx: &mut TestAppContext,
) {
    use crate::NativeWorkspaceSessions;
    cx.executor().allow_parking();
    cx.update(NativeWorkspaceSessions::install);
    let (session, sender, _) = native_duplicate_fixture(b"invalid placement");
    let events = session.take_events().unwrap();
    drop(sender);
    drop(events);
    cx.update(|cx| {
        crate::close_unplaced_session_in_workspace(
            oneterm_terminal::CreatedSession::Native(session),
            cx,
        )
    });
    cx.update(NativeWorkspaceSessions::shutdown);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while cx.update(NativeWorkspaceSessions::result).is_none() {
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(1));
        cx.run_until_parked();
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(matches!(cx.update(NativeWorkspaceSessions::result),
        Some(Err(error)) if error.contains("event receiver already taken")));
}
