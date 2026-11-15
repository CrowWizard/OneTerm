//! Actual context-menu navigation uses existing command callbacks, never a host.

use super::*;
use gpui::{Entity, Global, MouseButton, MouseDownEvent, MouseMoveEvent, point, px};

#[derive(Clone, Default)]
struct SshMenuProbe {
    saved: oneterm_state::commands::SavedSshSessionSections,
    calls: Vec<u64>,
    completion: Option<oneterm_state::commands::SshDuplicateCompletion>,
}
impl Global for SshMenuProbe {}

fn install(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    cx.update(gpui_component::init);
    cx.update(crate::init);
    cx.update(oneterm_settings::TerminalSettings::init);
    cx.update(oneterm_state::AppState::init);
    cx.update(|cx| {
        cx.set_global(SshMenuProbe {
            saved: vec![
                (
                    String::new(),
                    vec![(71, "Ungrouped host".into(), "#ffffff".into())],
                ),
                (
                    "Production".into(),
                    vec![(902, "Saved host".into(), "#ffffff".into())],
                ),
            ],
            ..Default::default()
        });
        let mut commands = duplicate_test_commands();
        commands.saved_ssh_sessions = |cx| cx.global::<SshMenuProbe>().saved.clone();
        commands.open_quick_connect_dialog = |_, cx| cx.global_mut::<SshMenuProbe>().calls.push(0);
        commands.open_saved_ssh_session =
            |id, _, cx| cx.global_mut::<SshMenuProbe>().calls.push(id);
        commands.open_quick_connect_to = |completion, _, cx| {
            let probe = cx.global_mut::<SshMenuProbe>();
            probe.calls.push(0);
            probe.completion = Some(completion);
        };
        commands.open_saved_ssh_session_to = |id, completion, _, cx| {
            let probe = cx.global_mut::<SshMenuProbe>();
            probe.calls.push(id);
            probe.completion = Some(completion);
        };
        AppServices::install(
            cx,
            Arc::new(DuplicateSessionFactory {
                spawned_local_configs: Arc::default(),
            }),
            commands,
            crate::status_metrics(),
        )
        .unwrap();
    });
}

fn draw(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, cx| {
        _ = window.draw(cx);
    });
}

fn rows(cx: &mut VisualTestContext) -> Vec<(String, gpui::Point<gpui::Pixels>)> {
    cx.update(|window, _| {
        let mut rows: Vec<_> = gpui_base::test_support::snapshots(window)
            .into_iter()
            .filter(|item| item.role() == Some(gpui::Role::MenuItem))
            .filter_map(|item| Some((item.label()?.to_string(), item.bounds().center())))
            .collect();
        rows.sort_by(|a, b| {
            a.1.x
                .partial_cmp(&b.1.x)
                .unwrap()
                .then(a.1.y.partial_cmp(&b.1.y).unwrap())
        });
        rows
    })
}

fn at(cx: &mut VisualTestContext, label: &str) -> gpui::Point<gpui::Pixels> {
    let rows = rows(cx);
    rows.iter()
        .find(|(name, _)| name == label)
        .unwrap_or_else(|| panic!("missing {label} in {rows:?}"))
        .1
}

fn open_label(cx: &mut VisualTestContext, position: gpui::Point<gpui::Pixels>, label: &str) {
    draw(cx);
    cx.simulate_event(MouseDownEvent {
        button: MouseButton::Right,
        position,
        modifiers: Default::default(),
        click_count: 1,
        first_mouse: false,
    });
    draw(cx);
    let rows = rows(cx);
    assert_eq!(rows[1].0, label, "drawn rows: {rows:?}");
    let position = at(cx, label);
    cx.simulate_event(MouseMoveEvent {
        position,
        pressed_button: None,
        modifiers: Default::default(),
    });
    draw(cx);
    cx.simulate_keystrokes("right");
    draw(cx);
    assert!(cx.update(|_, cx| cx.global::<SshMenuProbe>().calls.is_empty()));
}

fn open(cx: &mut VisualTestContext, position: gpui::Point<gpui::Pixels>) {
    open_label(cx, position, "New SSH");
}

fn click(cx: &mut VisualTestContext, label: &str) {
    let position = at(cx, label);
    cx.simulate_click(position, Default::default());
    draw(cx);
}

fn exercise(panel: Entity<TerminalPanel>, cx: &mut VisualTestContext) {
    let origin = point(px(40.), px(40.));
    open(cx, origin);
    click(cx, "Quick Connect...");
    assert_eq!(
        cx.update(|_, cx| std::mem::take(&mut cx.global_mut::<SshMenuProbe>().calls)),
        [0]
    );
    open(cx, origin);
    let names: Vec<_> = rows(cx).into_iter().map(|(name, _)| name).collect();
    let ungrouped = names
        .iter()
        .position(|name| name == "Ungrouped host")
        .unwrap();
    let grouped = names.iter().position(|name| name == "Saved host").unwrap();
    let heading = names.iter().position(|name| name == "Production").unwrap();
    assert!(ungrouped < heading && heading < grouped);
    click(cx, "Saved host");
    assert_eq!(
        cx.update(|_, cx| std::mem::take(&mut cx.global_mut::<SshMenuProbe>().calls)),
        [902]
    );
    cx.update(|_, cx| {
        cx.global_mut::<SshMenuProbe>().saved = vec![(
            String::new(),
            vec![(1234, "Newly added host".into(), "#ffffff".into())],
        )]
    });
    open(cx, origin);
    assert!(!rows(cx).iter().any(|(name, _)| name == "Saved host"));
    click(cx, "Newly added host");
    assert_eq!(
        cx.update(|_, cx| std::mem::take(&mut cx.global_mut::<SshMenuProbe>().calls)),
        [1234]
    );
    cx.update(|_, cx| {
        cx.global_mut::<SshMenuProbe>().saved = vec![(
            String::new(),
            (1..=60)
                .map(|id| (id, format!("Host {id}"), "#ffffff".into()))
                .collect(),
        )];
    });
    open(cx, origin);
    let position = at(cx, "Quick Connect...");
    cx.simulate_event(MouseMoveEvent {
        position,
        pressed_button: None,
        modifiers: Default::default(),
    });
    draw(cx);
    for _ in 0..60 {
        cx.simulate_keystrokes("down");
        draw(cx);
    }
    let last = at(cx, "Host 60");
    assert!(cx.update(|window, _| last.y > px(0.) && last.y < window.viewport_size().height));
    cx.simulate_keystrokes("enter");
    draw(cx);
    assert_eq!(
        cx.update(|_, cx| std::mem::take(&mut cx.global_mut::<SshMenuProbe>().calls)),
        [60]
    );
    cx.update(|_, cx| cx.global_mut::<SshMenuProbe>().saved.clear());
    panel.update_in(cx, |panel, window, cx| {
        let original = panel.tree.active();
        panel.split_active_at(original, SplitDir::Right, window, cx);
    });
    let empty_at = cx.update(|window, _| {
        let size = window.viewport_size();
        point(size.width * 0.75, size.height * 0.5)
    });
    assert!(cx.update(|_, cx| cx.global::<SshMenuProbe>().completion.is_none()));
    open_label(cx, empty_at, "New SSH Here");
    click(cx, "Quick Connect...");
    assert_eq!(
        cx.update(|_, cx| std::mem::take(&mut cx.global_mut::<SshMenuProbe>().calls)),
        [0]
    );
    assert!(cx.update(|_, cx| cx.global_mut::<SshMenuProbe>().completion.take().is_some()));
    cx.update(|_, cx| {
        cx.global_mut::<SshMenuProbe>().saved = vec![(
            String::new(),
            vec![(902, "Saved here".into(), "#ffffff".into())],
        )]
    });
    open_label(cx, empty_at, "New SSH Here");
    click(cx, "Saved here");
    assert_eq!(
        cx.update(|_, cx| std::mem::take(&mut cx.global_mut::<SshMenuProbe>().calls)),
        [902]
    );
    assert!(cx.update(|_, cx| cx.global_mut::<SshMenuProbe>().completion.take().is_some()));
    cx.update(|_, cx| cx.global_mut::<SshMenuProbe>().saved.clear());
    open_label(cx, empty_at, "New SSH Here");
    click(cx, "No saved sessions");
    assert!(cx.update(|_, cx| cx.global::<SshMenuProbe>().calls.is_empty()));
    cx.simulate_keystrokes("escape");
    draw(cx);
    cx.simulate_keystrokes("escape");
    draw(cx);
    panel.update_in(cx, |panel, window, cx| panel.shutdown(window, cx));
}

#[gpui::test]
fn legacy_and_empty_context_menus_dispatch_new_ssh(cx: &mut TestAppContext) {
    install(cx);
    let (session, _) = FakeTerminalSession::boxed(24, 80, "menu");
    let handoff = Rc::new(RefCell::new(None));
    let output = handoff.clone();
    let (_, cx) = cx.add_window_view(move |window, cx| {
        let panel = TerminalPanel::open(
            PanelSpec::Session {
                session,
                title: "Menu".into(),
                duplicate_config: None,
            },
            window,
            cx,
        );
        *output.borrow_mut() = Some(panel.clone());
        Root::new(panel, window, cx)
    });
    exercise(handoff.borrow().clone().unwrap(), cx);
}

#[cfg(feature = "ghostty-native")]
#[gpui::test]
fn native_and_empty_context_menus_dispatch_new_ssh(cx: &mut TestAppContext) {
    install(cx);
    let (session, events, _) = crate::native_surface::tests::native_duplicate_fixture(b"menu");
    let handoff = Rc::new(RefCell::new(None));
    let output = handoff.clone();
    let (_, cx) = cx.add_window_view(move |window, cx| {
        let panel = TerminalPanel::open(
            PanelSpec::NativeSession {
                session,
                title: "Menu".into(),
                duplicate_config: None,
            },
            window,
            cx,
        );
        *output.borrow_mut() = Some(panel.clone());
        Root::new(panel, window, cx)
    });
    exercise(handoff.borrow().clone().unwrap(), cx);
    drop(events);
}

#[cfg(feature = "ghostty-native")]
#[gpui::test]
fn native_ssh_here_completion_keeps_clicked_space_and_cleans_unplaced(cx: &mut TestAppContext) {
    use crate::native_surface::tests::native_duplicate_fixture;
    use oneterm_terminal::CreatedSession;
    use std::sync::atomic::Ordering;
    install(cx);
    cx.update(crate::NativeWorkspaceSessions::install);
    let (session, events, done) = native_duplicate_fixture(b"original");
    drop(events);
    let handoff = Rc::new(RefCell::new(None));
    let output = handoff.clone();
    let (_, cx) = cx.add_window_view(move |window, cx| {
        let panel = TerminalPanel::open(
            PanelSpec::NativeSession {
                session,
                title: "Here".into(),
                duplicate_config: None,
            },
            window,
            cx,
        );
        *output.borrow_mut() = Some(panel.clone());
        Root::new(panel, window, cx)
    });
    let panel = handoff.borrow().clone().unwrap();
    let original = panel.read_with(cx, |panel, _| panel.tree.active());
    let original_id = panel.read_with(cx, |panel, _| panel.tree.active_terminal().unwrap().id());
    let empty_at = cx.update(|window, _| {
        point(
            window.viewport_size().width * 0.75,
            window.viewport_size().height * 0.5,
        )
    });
    let config = SessionDuplicateConfig::Local(LocalShellConfig::default());
    let mut completions = vec![done];
    for label in ["Quick Connect...", "Saved host"] {
        let target = panel.update_in(cx, |panel, window, cx| {
            panel.split_active_at(original, SplitDir::Right, window, cx);
            panel.tree.active()
        });
        open_label(cx, empty_at, "New SSH Here");
        click(cx, label);
        let callback = cx.update(|_, cx| {
            let probe = cx.global_mut::<SshMenuProbe>();
            assert_eq!(
                std::mem::take(&mut probe.calls),
                [if label == "Quick Connect..." { 0 } else { 902 }]
            );
            probe.completion.take().unwrap()
        });
        panel.update_in(cx, |panel, window, cx| {
            panel.set_active_space(original, window, cx)
        });
        let (created, events, completed) = native_duplicate_fixture(b"connected");
        drop(events);
        completions.push(completed);
        cx.update(|window, cx| {
            callback(
                CreatedSession::Native(created),
                "Connected here".into(),
                config.clone(),
                window,
                cx,
            )
        });
        let installed_id = panel.read_with(cx, |panel, _| {
            assert_eq!(panel.tree.active(), target);
            assert_eq!(
                panel.tree.leaf_terminal(original).unwrap().id(),
                original_id
            );
            assert_eq!(panel.leaf_count(), 2);
            panel.tree.leaf_terminal(target).unwrap().id()
        });
        let (occupied, events, completed) = native_duplicate_fixture(b"occupied target");
        drop(events);
        completions.push(completed);
        cx.update(|window, cx| {
            callback(
                CreatedSession::Native(occupied),
                "Must not replace".into(),
                config.clone(),
                window,
                cx,
            )
        });
        assert_eq!(
            panel.read_with(cx, |panel, _| panel
                .tree
                .leaf_terminal(target)
                .unwrap()
                .id()),
            installed_id
        );
        panel.update_in(cx, |panel, window, cx| {
            panel.close_space(target, window, cx)
        });
        let (missing, events, completed) = native_duplicate_fixture(b"closed target");
        drop(events);
        completions.push(completed);
        cx.update(|window, cx| {
            callback(
                CreatedSession::Native(missing),
                "Must not open tab".into(),
                config.clone(),
                window,
                cx,
            )
        });
        assert_eq!(panel.read_with(cx, |panel, _| panel.leaf_count()), 1);
    }
    panel.update_in(cx, |panel, window, cx| panel.shutdown(window, cx));
    cx.update(|_, cx| crate::NativeWorkspaceSessions::shutdown(cx));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while completions
        .iter()
        .any(|done| done.load(Ordering::Relaxed) != 1)
        || cx
            .update(|_, cx| crate::NativeWorkspaceSessions::result(cx))
            .is_none()
    {
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(20));
        cx.run_until_parked();
        assert!(
            std::time::Instant::now() < deadline,
            "SSH Here cleanup must be observed"
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert_eq!(
        cx.update(|_, cx| crate::NativeWorkspaceSessions::result(cx)),
        Some(Ok(()))
    );
}
