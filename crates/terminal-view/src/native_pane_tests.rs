use super::*;

#[gpui::test]
fn native_pane_clipboard_live_gate_origin_hidden_refresh_and_shutdown(
    cx: &mut gpui::TestAppContext,
) {
    cx.executor().allow_parking();
    cx.update(gpui_component::init);
    let settings = cx.new(|_| oneterm_settings::TerminalSettings::default());
    let (session, events, replies, reject) = crate::native_surface::tests::native_effect_fixture();
    let (other, other_events, other_replies, _) =
        crate::native_surface::tests::native_effect_fixture();
    let (pane, window) = cx.add_window_view(move |window, cx| {
        NativeTerminalPane::new(session, gpui::font("monospace"), gpui::px(14.), window, cx)
            .unwrap()
    });
    pane.update(window, |pane, cx| {
        cx.write_to_clipboard(gpui::ClipboardItem::new_string("secret".into()));
        pane.state
            .lock()
            .unwrap()
            .event(SessionEvent::ClipboardRead);
        pane.apply_clipboard(cx);
        assert!(replies.lock().unwrap().is_empty(), "unbound read is denied");
        pane.bind_settings(settings.clone(), cx);
        pane.state
            .lock()
            .unwrap()
            .event(SessionEvent::ClipboardRead);
        pane.apply_clipboard(cx);
        assert!(replies.lock().unwrap().is_empty(), "default read is denied");
    });
    settings.update(window, |settings, _| settings.allow_clipboard_read = true);
    events.send_blocking(SessionEvent::ClipboardRead).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    // No draw/render call: the independent consumer and foreground refresh own effects.
    while replies.lock().unwrap().is_empty() {
        window.executor().advance_clock(Duration::from_millis(20));
        window.run_until_parked();
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(*replies.lock().unwrap(), ["secret"]);
    assert!(other_replies.lock().unwrap().is_empty());
    settings.update(window, |settings, _| settings.allow_clipboard_read = false);
    pane.update(window, |pane, cx| {
        pane.state
            .lock()
            .unwrap()
            .event(SessionEvent::ClipboardRead);
        pane.apply_clipboard(cx);
        assert_eq!(replies.lock().unwrap().len(), 1, "revocation is live");
    });
    settings.update(window, |settings, _| settings.allow_clipboard_read = true);
    reject.store(true, std::sync::atomic::Ordering::Relaxed);
    pane.update(window, |pane, cx| {
        pane.state
            .lock()
            .unwrap()
            .event(SessionEvent::ClipboardRead);
        pane.apply_clipboard(cx);
        assert!(
            pane.state
                .lock()
                .unwrap()
                .error
                .as_ref()
                .unwrap()
                .contains("reply failed")
        );
        pane.state
            .lock()
            .unwrap()
            .event(SessionEvent::Clipboard(Some("must not escape".into())));
        pane.shutdown(cx);
        pane.apply_clipboard(cx);
        assert_eq!(cx.read_from_clipboard().unwrap().text().unwrap(), "secret");
        assert!(pane.state.lock().unwrap().clipboard.is_empty());
    });
    drop(events);
    drop(other_events);
    let observation = pane.read_with(window, |pane, _| pane.completion());
    other.close().unwrap();
    drop(other.take_events());
    assert!(crate::native_surface::tests::complete(other.complete()).is_ok());
    let deadline = Instant::now() + Duration::from_secs(10);
    while observation.result().is_none() {
        window.executor().advance_clock(Duration::from_millis(20));
        window.run_until_parked();
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(observation.result().unwrap().is_ok());
}

#[gpui::test]
fn native_pane_binds_settings_to_its_actual_surface(cx: &mut gpui::TestAppContext) {
    cx.executor().allow_parking();
    cx.update(gpui_component::init);
    let settings = cx.new(|_| oneterm_settings::TerminalSettings::default());
    let (session, events) = crate::native_surface::tests::native_pane_fixture();
    drop(events);
    let (pane, window) = cx.add_window_view(move |window, cx| {
        NativeTerminalPane::new(session, gpui::font("monospace"), gpui::px(14.), window, cx)
            .unwrap()
    });
    pane.update(window, |pane, cx| {
        pane.bind_settings(settings.clone(), cx);
        assert!(pane.surface.is_some());
        // Rebinding replaces the observer without creating another surface.
        let identity = pane.surface.as_ref().unwrap().entity_id();
        pane.bind_settings(settings.clone(), cx);
        assert_eq!(pane.surface.as_ref().unwrap().entity_id(), identity);
    });
    let observation = pane.read_with(window, |pane, _| pane.completion());
    pane.update(window, |pane, cx| pane.shutdown(cx));
    let deadline = Instant::now() + Duration::from_secs(10);
    while observation.result().is_none() {
        window.executor().advance_clock(Duration::from_millis(20));
        window.run_until_parked();
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(observation.result().unwrap().is_ok());
}

#[test]
fn native_pane_completion_does_not_invent_success() {
    let observation = NativePaneCompletion(Arc::new(Mutex::new(CloseState::Running)));
    assert_eq!(observation.result(), None);
    *observation.0.lock().unwrap() = CloseState::Closing;
    assert_eq!(observation.result(), None);
    let report = Completion {
        exit_code: Some(0),
        lifecycle_delivered: true,
    };
    CompletionGuard(Some(observation.clone())).finish(Ok(report));
    assert_eq!(observation.result(), Some(Ok(report)));
}

#[test]
fn native_pane_unpolled_observer_cancellation_is_failure() {
    let observation = NativePaneCompletion(Arc::new(Mutex::new(CloseState::Closing)));
    let guard = CompletionGuard(Some(observation.clone()));
    drop(async move {
        std::future::pending::<()>().await;
        guard.finish(Ok(Completion {
            exit_code: None,
            lifecycle_delivered: true,
        }));
    });
    assert!(
        observation
            .result()
            .unwrap()
            .unwrap_err()
            .to_string()
            .contains("cancelled")
    );
}

#[test]
fn native_pane_effect_queues_are_bounded_and_explicit() {
    let mut state = EventState::default();
    for index in 0..10_000 {
        state.event(SessionEvent::ClipboardRead);
        state.event(SessionEvent::Notification(format!("notice {index}")));
    }
    assert_eq!(state.clipboard.len(), MAX_QUEUED_NOTIFICATIONS);
    assert_eq!(state.dropped_clipboard, 9_995);
    assert_eq!(state.notifications.len(), MAX_QUEUED_NOTIFICATIONS);
    assert_eq!(state.dropped_notifications, 9_995);
    assert_eq!(
        state.notifications.front().map(String::as_str),
        Some("notice 9995")
    );
    assert_eq!(
        state.notifications.back().map(String::as_str),
        Some("notice 9999")
    );
    assert!(state.error.unwrap().contains("dropped"));
}

#[test]
fn native_pane_progress_bell_and_effects_stop_on_exit() {
    let mut state = EventState::default();
    state.event(SessionEvent::Progress(
        oneterm_terminal::TerminalProgress::Set(45),
    ));
    state.event(SessionEvent::Bell);
    assert_eq!(
        state.progress,
        Some(oneterm_terminal::TerminalProgress::Set(45))
    );
    assert!(state.bell);
    state.event(SessionEvent::Progress(
        oneterm_terminal::TerminalProgress::Remove,
    ));
    assert!(state.progress.is_none());
    state.event(SessionEvent::ClipboardRead);
    state.event(SessionEvent::Exited(Some(0)));
    for event in [
        SessionEvent::ClipboardRead,
        SessionEvent::Notification("late".into()),
        SessionEvent::Bell,
        SessionEvent::Progress(oneterm_terminal::TerminalProgress::Indeterminate),
    ] {
        state.event(event);
    }
    assert!(state.clipboard.is_empty());
    assert!(state.notifications.is_empty());
    assert!(state.progress.is_none());
    assert!(!state.bell);
}

#[test]
fn native_pane_exit_clears_live_status_but_keeps_finished_result() {
    let mut state = EventState::default();
    state.event(SessionEvent::ProgramStatus(Some(ProgramState::Working)));
    state.event(SessionEvent::Exited(Some(7)));
    assert_eq!(state.exited, Some(Some(7)));
    assert_eq!(state.status, None);
    state.event(SessionEvent::ProgramStatus(Some(ProgramState::Error)));
    state.event(SessionEvent::Closed);
    assert_eq!(state.status, Some(ProgramState::Error));
    assert!(state.closed);
}

#[test]
fn native_pane_title_and_cwd_replace_previous_state() {
    let mut state = EventState::default();
    state.event(SessionEvent::Title("old".into()));
    state.event(SessionEvent::Title("new".into()));
    state.event(SessionEvent::Cwd(PathBuf::from("current")));
    assert_eq!(state.title.as_deref(), Some("new"));
    assert_eq!(state.cwd, Some(PathBuf::from("current")));
    assert!(state.dirty);
}
