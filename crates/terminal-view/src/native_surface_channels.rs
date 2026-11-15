//! Ordered origin input: acknowledge the original FIFO before any channel fan-out.

use std::cell::Cell;

use gpui::Task;
use oneterm_state::BroadcastInput;
use oneterm_terminal::{TerminalError, backend::AsyncGhosttyPumpError};

use super::*;
use crate::terminal_view::BroadcastOrigin;

const CAPACITY: usize = 64;
const BYTE_BUDGET: usize = 4 * 1024 * 1024;

#[derive(Default)]
pub(super) struct OriginQueue {
    origin: Option<BroadcastOrigin>,
    sender: Option<async_channel::Sender<Queued>>,
    bytes: Rc<Cell<usize>>,
    task: Option<Task<()>>,
}

impl OriginQueue {
    pub(super) fn bound(&self) -> bool {
        self.sender.is_some()
    }
}

enum Input {
    Key {
        name: String,
        key: KeyEvent,
        interrupt: bool,
    },
    Text(String),
    Paste(String),
}

struct Queued {
    input: Input,
    origin: BroadcastOrigin,
    token: Option<(oneterm_core::InputChannel, u64)>,
    bytes: Rc<Cell<usize>>,
    size: usize,
}

impl Drop for Queued {
    fn drop(&mut self) {
        self.bytes.set(self.bytes.get() - self.size);
    }
}

impl NativeTerminalSurface {
    pub(crate) fn bind_channel_origin(&mut self, origin: BroadcastOrigin, cx: &mut Context<Self>) {
        self.channels.origin = Some(origin.clone());
        if origin.channels.is_none() || self.channels.sender.is_some() {
            return;
        }
        let (sender, receiver) = async_channel::bounded::<Queued>(CAPACITY);
        self.channels.sender = Some(sender);
        // Waiting for input retains neither surface nor a strong native session.
        let session = Arc::downgrade(&self.session);
        self.channels.task = Some(cx.spawn(async move |surface, cx| {
            let mut accepted: Vec<String> = Vec::new();
            while let Ok(queued) = receiver.recv().await {
                let Some(session) = session.upgrade() else {
                    break;
                };
                let paste = matches!(&queued.input, Input::Paste(_));
                let result = match &queued.input {
                    Input::Key {
                        name,
                        key,
                        interrupt,
                    } => {
                        if matches!(key.action, KeyAction::Release) {
                            if let Some(index) = accepted.iter().position(|held| held == name) {
                                accepted.remove(index);
                                session.key(key.clone()).map_err(|error| match error {
                                    InputError::Admission(error) => {
                                        AsyncGhosttyPumpError::Transport(error)
                                    }
                                    InputError::Invalid(error) => AsyncGhosttyPumpError::Engine(
                                        oneterm_terminal::ghostty::owner::Error::Native(error)
                                            .into(),
                                    ),
                                })
                            } else {
                                Ok(())
                            }
                        } else {
                            match session.key_receipt(key.clone()).await {
                                Ok(bytes) => {
                                    if !accepted.contains(name) {
                                        accepted.push(name.clone());
                                    }
                                    if !bytes.is_empty() {
                                        Self::fan_out_receipt(
                                            &queued,
                                            if *interrupt {
                                                BroadcastInput::Interrupt
                                            } else {
                                                BroadcastInput::Bytes(&bytes)
                                            },
                                            cx,
                                        );
                                    }
                                    Ok(())
                                }
                                Err(error) => Err(error),
                            }
                        }
                    }
                    Input::Text(text) => {
                        let result = session.write_visible(text.as_bytes()).await;
                        if result.is_ok() {
                            Self::fan_out_receipt(&queued, BroadcastInput::Text(text), cx);
                        }
                        result
                    }
                    Input::Paste(text) => {
                        let result = session.paste_visible(text).await;
                        if result.is_ok() {
                            Self::fan_out_receipt(&queued, BroadcastInput::Paste(text), cx);
                        }
                        result
                    }
                };
                if let Err(released) = surface.update(cx, |surface, cx| {
                    if paste {
                        surface.edit.busy = false;
                    }
                    if let Err(error) = &result {
                        (surface.on_error)(NativeSurfaceError::Edit(error.clone()), cx);
                    }
                    cx.notify();
                }) {
                    if let Err(error) = result {
                        log::warn!("Native channel origin failed after surface release: {error}");
                    }
                    log::debug!("Native channel origin recipient released: {released}");
                }
            }
        }));
    }

    fn fan_out_receipt(queued: &Queued, input: BroadcastInput<'_>, cx: &mut gpui::AsyncApp) {
        let Some(token) = queued.token else {
            return;
        };
        cx.update(|cx| {
            let Some(registry) = &queued.origin.channels else {
                return;
            };
            // Leave, close or move while waiting must not redirect old input.
            if registry.read(cx).membership_token(queued.origin.id) == Some(token) {
                queued.origin.fan_out(input, cx);
            }
        });
    }

    fn enqueue_channel(&mut self, input: Input, size: usize, cx: &mut Context<Self>) -> bool {
        let Some(sender) = &self.channels.sender else {
            return false;
        };
        if sender.is_full() || sender.is_closed() || size > BYTE_BUDGET - self.channels.bytes.get()
        {
            (self.on_error)(
                NativeSurfaceError::Control(InputError::Admission(TerminalError::QueueFull)),
                cx,
            );
            return false;
        }
        let origin = self
            .channels
            .origin
            .as_ref()
            .expect("bound origin queue")
            .clone();
        let token = origin
            .channels
            .as_ref()
            .and_then(|registry| registry.read(cx).membership_token(origin.id));
        self.channels.bytes.set(self.channels.bytes.get() + size);
        if let Err(error) = sender.try_send(Queued {
            input,
            origin,
            token,
            bytes: self.channels.bytes.clone(),
            size,
        }) {
            (self.on_error)(
                NativeSurfaceError::Control(InputError::Admission(TerminalError::Transport(
                    error.to_string(),
                ))),
                cx,
            );
            return false;
        }
        true
    }

    pub(super) fn queue_channel_key(
        &mut self,
        name: String,
        key: KeyEvent,
        interrupt: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        if let Err(error) = key.validate() {
            (self.on_error)(NativeSurfaceError::Control(InputError::Invalid(error)), cx);
            return false;
        }
        let size = name.len() + key.text.len();
        self.enqueue_channel(
            Input::Key {
                name,
                key,
                interrupt,
            },
            size,
            cx,
        )
    }

    pub(super) fn queue_channel_text(&mut self, text: &str, cx: &mut Context<Self>) {
        if text.len() > oneterm_terminal::ghostty_ime::MAX_TEXT_BYTES {
            (self.on_error)(
                NativeSurfaceError::Composition(oneterm_terminal::ghostty_ime::Error::TextLimit),
                cx,
            );
            return;
        }
        self.session.clear_marked_text();
        self.enqueue_channel(Input::Text(text.to_owned()), text.len(), cx);
    }

    pub(super) fn queue_channel_paste(&mut self, text: String, cx: &mut Context<Self>) -> bool {
        let size = text.len();
        self.enqueue_channel(Input::Paste(text), size, cx)
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use gpui::{AppContext as _, EntityInputHandler as _, TestAppContext, VisualTestContext};
    use oneterm_core::InputChannel;
    use oneterm_state::InputChannelRegistry;
    use oneterm_terminal::ghostty_bridge::{AsyncEngine, Command};

    use super::*;
    use crate::native_surface::tests::{Endpoint, call, down};

    struct Fixture {
        surface: Entity<NativeTerminalSurface>,
        endpoint: Arc<Endpoint>,
        registry: Entity<InputChannelRegistry>,
        peer: Entity<()>,
        received: Rc<RefCell<Vec<(u8, Vec<u8>)>>>,
        errors: Rc<RefCell<Vec<NativeSurfaceError>>>,
    }

    fn fixture(cx: &mut TestAppContext) -> (Fixture, &mut VisualTestContext) {
        cx.executor().allow_parking();
        cx.update(gpui_component::init);
        let registry = cx.new(|_| InputChannelRegistry::default());
        let peer = cx.new(|_| ());
        let engine = AsyncEngine::spawn(16, 3).unwrap();
        call(&engine, Command::Feed(b"\x1b[?1h".to_vec()));
        call(&engine, Command::Capture);
        let mut endpoint = Endpoint::new(engine.model());
        endpoint.selection_engine = Some(Arc::new(engine));
        let endpoint = Arc::new(endpoint);
        let source = endpoint.clone();
        let errors = Rc::new(RefCell::new(Vec::new()));
        let failures = errors.clone();
        let (surface, window) = cx.add_window_view(move |window, cx| {
            NativeTerminalSurface::new(
                source,
                gpui::font("monospace"),
                px(14.),
                move |error, _| failures.borrow_mut().push(error),
                window,
                cx,
            )
        });
        let received = Rc::new(RefCell::new(Vec::new()));
        let calls = received.clone();
        registry.update(window, |registry, cx| {
            registry.join_receiver(
                surface.entity_id(),
                InputChannel::A,
                Rc::new(|_, _| panic!("origin skipped")),
                cx,
            );
            registry.join_receiver(
                peer.entity_id(),
                InputChannel::A,
                Rc::new(move |input, _| {
                    calls.borrow_mut().push(match input {
                        BroadcastInput::Bytes(bytes) => (0, bytes.to_vec()),
                        BroadcastInput::Text(text) => (1, text.as_bytes().to_vec()),
                        BroadcastInput::Paste(text) => (2, text.as_bytes().to_vec()),
                        BroadcastInput::Interrupt => (3, vec![3]),
                    });
                }),
                cx,
            );
        });
        surface.update(window, |surface, cx| {
            surface.bind_channel_origin(
                BroadcastOrigin {
                    id: cx.entity_id(),
                    channels: Some(registry.clone()),
                },
                cx,
            )
        });
        (
            Fixture {
                surface,
                endpoint,
                registry,
                peer,
                received,
                errors,
            },
            window,
        )
    }

    fn pump(cx: &mut VisualTestContext, mut ready: impl FnMut(&mut VisualTestContext) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            cx.run_until_parked();
            if ready(cx) {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "native channel origin proof deadline"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    #[gpui::test]
    fn native_channel_origin_orders_ime_key_paste_and_keeps_release_and_blur_local(
        cx: &mut TestAppContext,
    ) {
        let (f, cx) = fixture(cx);
        let (release, gate) = async_channel::bounded(1);
        *f.endpoint.channel_probe.gate.lock().unwrap() = Some(gate);
        f.surface.update_in(cx, |surface, window, cx| {
            surface.replace_text_in_range(None, "text", window, cx);
            surface.key_down(&down("up", false), window, cx);
            cx.write_to_clipboard(gpui::ClipboardItem::new_string("paste\n".into()));
            surface.paste_clipboard(cx);
            surface.key_up(
                &KeyUpEvent {
                    keystroke: Keystroke::parse("up").unwrap(),
                },
                window,
                cx,
            );
        });
        cx.run_until_parked();
        assert!(
            f.received.borrow().is_empty(),
            "no fanout before origin acknowledgment"
        );
        assert!(f.surface.read_with(cx, |surface, _| surface.edit.busy));
        release.send_blocking(()).unwrap();
        pump(cx, |cx| {
            f.received.borrow().len() == 3
                && !f.surface.read_with(cx, |s, _| s.edit.busy)
                && f.endpoint
                    .keys
                    .lock()
                    .unwrap()
                    .iter()
                    .any(|key| matches!(key.action, KeyAction::Release))
        });
        assert_eq!(
            *f.received.borrow(),
            [
                (1, b"text".to_vec()),
                (0, b"\x1bOA".to_vec()),
                (2, b"paste\n".to_vec())
            ]
        );
        assert_eq!(
            *f.endpoint.channel_probe.inputs.lock().unwrap(),
            [
                (false, b"text".to_vec()),
                (false, b"\x1bOA".to_vec()),
                (true, b"paste\n".to_vec())
            ]
        );
        f.surface.update_in(cx, |surface, window, cx| {
            surface.key_down(&down("ctrl-c", false), window, cx);
            surface.release_held_keys(cx);
        });
        pump(cx, |_| {
            f.received.borrow().len() == 4 && f.endpoint.keys.lock().unwrap().len() == 4
        });
        assert_eq!(f.received.borrow()[3], (3, vec![3]));
        assert!(f.errors.borrow().is_empty());
    }

    #[gpui::test]
    fn native_channel_origin_failed_and_empty_receipts_never_fan_out(cx: &mut TestAppContext) {
        let (f, cx) = fixture(cx);
        f.endpoint
            .channel_probe
            .reject
            .store(true, std::sync::atomic::Ordering::Relaxed);
        f.surface
            .update_in(cx, |s, w, cx| s.key_down(&down("up", false), w, cx));
        pump(cx, |_| !f.errors.borrow().is_empty());
        assert!(f.received.borrow().is_empty());
        f.endpoint
            .channel_probe
            .reject
            .store(false, std::sync::atomic::Ordering::Relaxed);
        f.surface.update(cx, |s, cx| {
            assert!(s.queue_channel_key(
                "empty".into(),
                KeyEvent::new(KeyCode::UNIDENTIFIED),
                false,
                cx
            ));
            let mut release = KeyEvent::new(KeyCode::UNIDENTIFIED);
            release.action = KeyAction::Release;
            assert!(s.queue_channel_key("empty".into(), release, false, cx));
        });
        pump(cx, |_| f.endpoint.keys.lock().unwrap().len() == 3);
        assert!(f.received.borrow().is_empty());
        assert_eq!(
            f.endpoint
                .channel_probe
                .inputs
                .lock()
                .unwrap()
                .last()
                .unwrap()
                .1,
            b""
        );
    }

    #[gpui::test]
    fn native_channel_origin_pending_input_never_redirects_after_leave_rejoin_or_move(
        cx: &mut TestAppContext,
    ) {
        let (f, cx) = fixture(cx);
        for destination in [InputChannel::A, InputChannel::B] {
            let (release, gate) = async_channel::bounded(1);
            *f.endpoint.channel_probe.gate.lock().unwrap() = Some(gate);
            f.surface
                .update_in(cx, |s, w, cx| s.key_down(&down("up", false), w, cx));
            cx.run_until_parked();
            let id = f.surface.entity_id();
            f.registry.update(cx, |registry, cx| {
                registry.leave(id, cx);
                registry.join_receiver(id, destination, Rc::new(|_, _| {}), cx);
                registry.join_receiver(
                    f.peer.entity_id(),
                    destination,
                    Rc::new(|_, _| panic!("old input redirected")),
                    cx,
                );
            });
            release.send_blocking(()).unwrap();
            pump(cx, |cx| {
                f.surface.read_with(cx, |s, _| s.channels.bytes.get() == 0)
            });
            assert!(f.received.borrow().is_empty());
        }
    }

    #[gpui::test]
    fn native_channel_origin_queue_bounds_and_surface_release_cancel_pending_work(
        cx: &mut TestAppContext,
    ) {
        let (f, cx) = fixture(cx);
        let (_release, gate) = async_channel::bounded(1);
        *f.endpoint.channel_probe.gate.lock().unwrap() = Some(gate);
        f.surface
            .update(cx, |s, cx| s.queue_channel_text("first", cx));
        cx.run_until_parked();
        f.surface.update(cx, |s, cx| {
            for _ in 0..CAPACITY {
                s.queue_channel_text("queued", cx);
            }
            s.queue_channel_text("rejected", cx);
        });
        assert_eq!(f.errors.borrow().len(), 1);
        assert_eq!(f.endpoint.channel_probe.inputs.lock().unwrap().len(), 1);
        let weak = f.surface.downgrade();
        let session = Arc::downgrade(&f.endpoint);
        cx.update(|window, _| window.remove_window());
        cx.cx.update(|_| drop(f.surface));
        cx.run_until_parked();
        assert!(weak.upgrade().is_none());
        assert!(f.received.borrow().is_empty());
        drop(f.endpoint);
        pump(cx, |cx| {
            cx.executor().advance_clock(Duration::from_millis(20));
            session.upgrade().is_none()
        });
    }

    #[gpui::test]
    fn native_channel_origin_byte_budget_and_rejected_release_remain_bounded(
        cx: &mut TestAppContext,
    ) {
        let (f, cx) = fixture(cx);
        let (release, gate) = async_channel::bounded(1);
        *f.endpoint.channel_probe.gate.lock().unwrap() = Some(gate);
        f.surface
            .update_in(cx, |s, w, cx| s.key_down(&down("up", false), w, cx));
        cx.run_until_parked();
        f.surface.update(cx, |s, cx| {
            for _ in 0..CAPACITY {
                s.queue_channel_text("queued", cx);
            }
            s.release_held_keys(cx);
            assert_eq!(
                s.held.len(),
                1,
                "rejected release remains available for blur cleanup"
            );
        });
        assert_eq!(f.errors.borrow().len(), 1);
        release.send_blocking(()).unwrap();
        pump(cx, |cx| {
            f.surface.read_with(cx, |s, _| s.channels.bytes.get() == 0)
        });
        f.surface.update(cx, |s, cx| s.release_held_keys(cx));
        pump(cx, |_| f.endpoint.keys.lock().unwrap().len() == 2);
        assert!(matches!(
            f.endpoint.keys.lock().unwrap()[1].action,
            KeyAction::Release
        ));
        let (release, gate) = async_channel::bounded(1);
        *f.endpoint.channel_probe.gate.lock().unwrap() = Some(gate);
        let payload = "x".repeat(1024 * 1024);
        f.surface.update(cx, |s, cx| {
            for _ in 0..4 {
                s.queue_channel_text(&payload, cx);
            }
            assert_eq!(s.channels.bytes.get(), BYTE_BUDGET);
            s.queue_channel_text("over-budget", cx);
        });
        assert_eq!(f.errors.borrow().len(), 2);
        release.send_blocking(()).unwrap();
        pump(cx, |cx| {
            f.surface.read_with(cx, |s, _| s.channels.bytes.get() == 0)
        });
    }
}
