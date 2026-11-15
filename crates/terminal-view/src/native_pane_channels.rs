//! Weak membership and bounded, ordered peer input. Never recursively fans out.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{EntityId, Task};
use oneterm_core::InputChannel;
use oneterm_state::{BroadcastInput, InputChannelRegistry};

use super::*;

const QUEUE_CAPACITY: usize = 64;
const BYTE_BUDGET: usize = 4 * 1024 * 1024;

#[derive(Default)]
pub(super) struct Channels {
    registry: Option<Entity<InputChannelRegistry>>,
    id: Option<EntityId>,
    sender: Option<async_channel::Sender<Queued>>,
    bytes: Rc<Cell<usize>>,
    task: Option<Task<()>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_surface::tests::{ChannelProbe, native_channel_fixture};
    use gpui::{TestAppContext, VisualTestContext};
    use oneterm_terminal::test_support::FakeTerminalSession;
    use std::sync::atomic::Ordering;

    fn pump(cx: &mut VisualTestContext, mut ready: impl FnMut(&mut VisualTestContext) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            cx.executor().advance_clock(Duration::from_millis(20));
            cx.run_until_parked();
            if ready(cx) {
                return;
            }
            assert!(Instant::now() < deadline, "native channel proof deadline");
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    fn sibling(
        cx: &mut VisualTestContext,
    ) -> (
        Entity<NativeTerminalPane>,
        async_channel::Sender<SessionEvent>,
        ChannelProbe,
    ) {
        let (session, events, probe) = native_channel_fixture();
        let pane = cx.update(|window, cx| {
            cx.new(|cx| {
                NativeTerminalPane::new(session, gpui::font("monospace"), gpui::px(14.), window, cx)
                    .unwrap()
            })
        });
        (pane, events, probe)
    }

    #[gpui::test]
    fn native_channel_receives_mixed_peers_once_in_order_and_membership_does_not_retain_owner(
        cx: &mut TestAppContext,
    ) {
        cx.executor().allow_parking();
        cx.update(gpui_component::init);
        let registry = cx.new(|_| InputChannelRegistry::default());
        let (legacy, legacy_probe) = FakeTerminalSession::boxed(3, 16, "");
        let legacy = cx.new(|_| legacy);
        let origin = legacy.entity_id();
        let (session, events, probe) = native_channel_fixture();
        let (pane, cx) = cx.add_window_view(move |window, cx| {
            NativeTerminalPane::new(session, gpui::font("monospace"), gpui::px(14.), window, cx)
                .unwrap()
        });
        let (peer, peer_events, peer_probe) = sibling(cx);
        let (outside, outside_events, outside_probe) = sibling(cx);
        for view in [&pane, &peer, &outside] {
            view.update(cx, |pane, cx| {
                pane.bind_channels(Some(registry.clone()), cx)
            });
        }
        pane.update(cx, |pane, cx| pane.join_channel(InputChannel::A, cx));
        peer.update(cx, |pane, cx| pane.join_channel(InputChannel::A, cx));
        outside.update(cx, |pane, cx| pane.join_channel(InputChannel::B, cx));
        registry.update(cx, |registry, cx| {
            registry.join(origin, InputChannel::A, legacy, cx);
            for input in [
                BroadcastInput::Bytes(b"\x1bOA"),
                BroadcastInput::Text("中文"),
                BroadcastInput::Paste("line\n"),
                BroadcastInput::Interrupt,
            ] {
                registry.fan_out(origin, input, cx);
            }
        });
        pump(cx, |_| {
            probe.inputs.lock().unwrap().len() == 4 && peer_probe.inputs.lock().unwrap().len() == 4
        });
        let expected = vec![
            (false, b"\x1bOA".to_vec()),
            (false, "中文".as_bytes().to_vec()),
            (true, b"line\n".to_vec()),
            (false, vec![3]),
        ];
        assert_eq!(*probe.inputs.lock().unwrap(), expected);
        assert_eq!(*peer_probe.inputs.lock().unwrap(), expected);
        assert!(
            legacy_probe.writes().is_empty(),
            "origin skipped; peers never recursively fan out"
        );
        assert!(outside_probe.inputs.lock().unwrap().is_empty());
        pane.update(cx, |pane, cx| pane.join_channel(InputChannel::B, cx));
        registry.update(cx, |registry, cx| {
            registry.fan_out(origin, BroadcastInput::Bytes(b"only-peer"), cx)
        });
        pump(cx, |_| peer_probe.inputs.lock().unwrap().len() == 5);
        assert_eq!(probe.inputs.lock().unwrap().len(), 4);
        pane.update(cx, |pane, cx| pane.leave_channel(cx));
        registry.update(cx, |registry, cx| {
            assert_eq!(registry.close(InputChannel::A, cx), 2);
        });
        for view in [&pane, &peer, &outside] {
            view.update(cx, |pane, cx| pane.shutdown(cx));
        }
        drop((events, peer_events, outside_events));
        pump(cx, |_| {
            [&probe, &peer_probe, &outside_probe]
                .iter()
                .all(|p| p.completed.load(Ordering::Relaxed) == 1)
        });
        assert!(registry.read_with(cx, |registry, _| {
            registry.members(InputChannel::B).is_empty()
        }));
    }

    #[gpui::test]
    fn native_channel_queue_rejection_is_visible_and_other_peers_continue(cx: &mut TestAppContext) {
        cx.executor().allow_parking();
        cx.update(gpui_component::init);
        let registry = cx.new(|_| InputChannelRegistry::default());
        let (legacy, _) = FakeTerminalSession::boxed(3, 16, "");
        let legacy = cx.new(|_| legacy);
        let origin = legacy.entity_id();
        let (session, events, probe) = native_channel_fixture();
        let (release, gate) = async_channel::bounded(1);
        *probe.gate.lock().unwrap() = Some(gate);
        let (pane, cx) = cx.add_window_view(move |window, cx| {
            NativeTerminalPane::new(session, gpui::font("monospace"), gpui::px(14.), window, cx)
                .unwrap()
        });
        let (peer, peer_events, peer_probe) = sibling(cx);
        for view in [&pane, &peer] {
            view.update(cx, |pane, cx| {
                pane.bind_channels(Some(registry.clone()), cx);
                pane.join_channel(InputChannel::A, cx);
            });
        }
        registry.update(cx, |registry, cx| {
            registry.join(origin, InputChannel::A, legacy, cx);
            registry.fan_out(origin, BroadcastInput::Bytes(b"first"), cx);
        });
        pump(cx, |_| {
            probe.inputs.lock().unwrap().len() == 1 && peer_probe.inputs.lock().unwrap().len() == 1
        });
        // Do not schedule the worker: both inboxes fill identically.
        registry.update(cx, |registry, cx| {
            for _ in 0..QUEUE_CAPACITY {
                registry.fan_out(origin, BroadcastInput::Bytes(b"queued"), cx);
            }
            registry.fan_out(origin, BroadcastInput::Bytes(b"rejected"), cx);
        });
        assert!(pane.read_with(cx, |pane, _| {
            pane.state
                .lock()
                .unwrap()
                .error
                .as_ref()
                .unwrap()
                .contains("channel")
        }));
        pump(cx, |_| peer_probe.inputs.lock().unwrap().len() == 65);
        registry.update(cx, |registry, cx| {
            registry.fan_out(origin, BroadcastInput::Bytes(b"healthy-peer"), cx)
        });
        pump(cx, |_| peer_probe.inputs.lock().unwrap().len() == 66);
        assert_eq!(probe.inputs.lock().unwrap().len(), 1);
        release.send_blocking(()).unwrap();
        pump(cx, |_| probe.inputs.lock().unwrap().len() == 65);
        assert!(
            !probe
                .inputs
                .lock()
                .unwrap()
                .iter()
                .any(|(_, bytes)| bytes == b"rejected" || bytes == b"healthy-peer")
        );
        assert_eq!(pane.read_with(cx, |pane, _| pane.channels.bytes.get()), 0);
        for view in [&pane, &peer] {
            view.update(cx, |pane, cx| pane.shutdown(cx));
        }
        drop((events, peer_events));
        pump(cx, |_| {
            probe.completed.load(Ordering::Relaxed) == 1
                && peer_probe.completed.load(Ordering::Relaxed) == 1
        });
    }

    #[gpui::test]
    fn native_channel_receiver_failure_isolated_and_shutdown_cancels_pending_inbox(
        cx: &mut TestAppContext,
    ) {
        cx.executor().allow_parking();
        cx.update(gpui_component::init);
        let registry = cx.new(|_| InputChannelRegistry::default());
        let (legacy, _) = FakeTerminalSession::boxed(3, 16, "");
        let legacy = cx.new(|_| legacy);
        let origin = legacy.entity_id();
        let (session, events, probe) = native_channel_fixture();
        let (pane, cx) = cx.add_window_view(move |window, cx| {
            NativeTerminalPane::new(session, gpui::font("monospace"), gpui::px(14.), window, cx)
                .unwrap()
        });
        let (peer, peer_events, peer_probe) = sibling(cx);
        for view in [&pane, &peer] {
            view.update(cx, |pane, cx| {
                pane.bind_channels(Some(registry.clone()), cx);
                pane.join_channel(InputChannel::A, cx);
            });
        }
        probe.reject.store(true, Ordering::Relaxed);
        registry.update(cx, |registry, cx| {
            registry.join(origin, InputChannel::A, legacy, cx);
            registry.fan_out(origin, BroadcastInput::Bytes(b"failure"), cx);
        });
        pump(cx, |cx| {
            peer_probe.inputs.lock().unwrap().len() == 1
                && pane.read_with(cx, |pane, _| pane.state.lock().unwrap().error.is_some())
        });
        assert!(probe.inputs.lock().unwrap().is_empty());
        probe.reject.store(false, Ordering::Relaxed);
        let (_release, gate) = async_channel::bounded(1);
        *probe.gate.lock().unwrap() = Some(gate);
        registry.update(cx, |registry, cx| {
            registry.fan_out(origin, BroadcastInput::Bytes(b"active"), cx);
            registry.fan_out(origin, BroadcastInput::Bytes(b"queued"), cx);
        });
        pump(cx, |_| {
            probe.inputs.lock().unwrap().len() == 1 && peer_probe.inputs.lock().unwrap().len() == 3
        });
        pane.update(cx, |pane, cx| pane.shutdown(cx));
        drop(events);
        pump(cx, |cx| {
            probe.completed.load(Ordering::Relaxed) == 1
                && pane.read_with(cx, |pane, _| pane.channels.bytes.get() == 0)
        });
        assert_eq!(
            probe.inputs.lock().unwrap().len(),
            1,
            "queued input cancelled, never retried"
        );
        registry.update(cx, |registry, cx| {
            registry.fan_out(origin, BroadcastInput::Bytes(b"healthy"), cx)
        });
        pump(cx, |_| peer_probe.inputs.lock().unwrap().len() == 4);
        peer.update(cx, |peer, cx| peer.shutdown(cx));
        drop(peer_events);
        pump(cx, |_| peer_probe.completed.load(Ordering::Relaxed) == 1);
    }

    #[gpui::test]
    fn native_channel_session_end_and_actual_release_remove_members(cx: &mut TestAppContext) {
        cx.executor().allow_parking();
        cx.update(gpui_component::init);
        let registry = cx.new(|_| InputChannelRegistry::default());
        let (session, events, probe) = native_channel_fixture();
        let (pane, cx) = cx.add_window_view(move |window, cx| {
            NativeTerminalPane::new(session, gpui::font("monospace"), gpui::px(14.), window, cx)
                .unwrap()
        });
        let (peer, peer_events, peer_probe) = sibling(cx);
        for view in [&pane, &peer] {
            view.update(cx, |pane, cx| {
                pane.bind_channels(Some(registry.clone()), cx);
                pane.join_channel(InputChannel::A, cx);
            });
        }
        events.send_blocking(SessionEvent::Exited(Some(0))).unwrap();
        peer_events.send_blocking(SessionEvent::Closed).unwrap();
        pump(cx, |cx| {
            registry.read_with(cx, |registry, _| {
                registry.members(InputChannel::A).is_empty()
            })
        });
        pane.update(cx, |pane, cx| pane.join_channel(InputChannel::B, cx));
        assert_eq!(
            registry.read_with(cx, |registry, _| registry.channel_of(pane.entity_id())),
            None
        );
        // A still-live sibling is released while enrolled, not just explicitly shut down.
        let (live, live_events, live_probe) = sibling(cx);
        live.update(cx, |pane, cx| {
            pane.bind_channels(Some(registry.clone()), cx);
            pane.join_channel(InputChannel::B, cx);
        });
        let id = live.entity_id();
        let weak = live.downgrade();
        cx.cx.update(|_| drop(live));
        assert!(weak.upgrade().is_none());
        assert_eq!(
            registry.read_with(cx, |registry, _| registry.channel_of(id)),
            None
        );
        pane.update(cx, |pane, cx| pane.shutdown(cx));
        peer.update(cx, |pane, cx| pane.shutdown(cx));
        drop((events, peer_events, live_events));
        pump(cx, |_| {
            [&probe, &peer_probe, &live_probe]
                .iter()
                .all(|p| p.completed.load(Ordering::Relaxed) == 1)
        });
    }
}

struct Queued {
    input: PeerInput,
    bytes: Rc<Cell<usize>>,
    size: usize,
}

impl Drop for Queued {
    fn drop(&mut self) {
        self.bytes.set(self.bytes.get() - self.size);
    }
}

enum PeerInput {
    Raw(Vec<u8>),
    Paste(String),
}

impl Channels {
    pub(super) fn stop(&mut self) {
        if let Some(sender) = self.sender.take() {
            sender.close();
        }
        self.task.take();
    }
}

impl NativeTerminalPane {
    pub(crate) fn session_alive(&self) -> bool {
        self.session.as_ref().is_some_and(|session| session.alive())
            && !self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .stopped
    }

    pub(crate) fn bind_channels(
        &mut self,
        registry: Option<Entity<InputChannelRegistry>>,
        cx: &mut Context<Self>,
    ) {
        self.leave_channel(cx);
        self.channels.registry = registry;
        self.channels.id = Some(cx.entity_id());
        if let Some(surface) = &self.surface {
            let origin = crate::terminal_view::BroadcastOrigin {
                id: cx.entity_id(),
                channels: self.channels.registry.clone(),
            };
            surface.update(cx, |surface, cx| surface.bind_channel_origin(origin, cx));
        }
        if self.channels.sender.is_some() {
            return;
        }
        let (sender, receiver) = async_channel::bounded::<Queued>(QUEUE_CAPACITY);
        self.channels.sender = Some(sender);
        self.channels.task = Some(cx.spawn(async move |pane, cx| {
            while let Ok(queued) = receiver.recv().await {
                let Ok(session) = pane.update(cx, |pane, _| {
                    pane.session_alive().then(|| pane.session.clone()).flatten()
                }) else {
                    break;
                };
                let Some(session) = session else {
                    break;
                };
                let result = match &queued.input {
                    PeerInput::Raw(bytes) => session.write_visible(bytes).await,
                    PeerInput::Paste(text) => session.paste_visible(text).await,
                };
                // Release budget before the next turn; no session retained by membership.
                drop(queued);
                if let Err(error) = result {
                    if pane
                        .update(cx, |pane, cx| pane.channel_error(error.to_string(), cx))
                        .is_err()
                    {
                        log::warn!("Native input channel failed after pane release: {error}");
                    }
                }
            }
        }));
    }

    pub(crate) fn join_channel(&mut self, channel: InputChannel, cx: &mut Context<Self>) {
        if !self.session_alive() {
            return;
        }
        let Some(registry) = self.channels.registry.clone() else {
            return;
        };
        let pane = cx.entity().downgrade();
        let id = cx.entity_id();
        registry.update(cx, |registry, cx| {
            registry.join_receiver(
                id,
                channel,
                Rc::new(move |input, cx| {
                    if pane
                        .update(cx, |pane, cx| pane.receive_channel(input, cx))
                        .is_err()
                    {
                        log::debug!("Native input channel recipient released");
                    }
                }),
                cx,
            )
        });
    }

    pub(crate) fn leave_channel(&self, cx: &mut App) {
        if let (Some(registry), Some(id)) = (&self.channels.registry, self.channels.id) {
            registry.update(cx, |registry, cx| {
                registry.leave(id, cx);
            });
        }
    }

    fn channel_error(&self, error: String, cx: &mut Context<Self>) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.error = Some(format!("Native input channel failed: {error}"));
        state.dirty = true;
        cx.notify();
    }

    fn receive_channel(&self, input: BroadcastInput<'_>, cx: &mut Context<Self>) {
        if !self.session_alive() {
            self.channel_error(TerminalError::Closed.to_string(), cx);
            return;
        }
        let size = match input {
            BroadcastInput::Bytes(bytes) => bytes.len(),
            BroadcastInput::Text(text) | BroadcastInput::Paste(text) => text.len(),
            BroadcastInput::Interrupt => 1,
        };
        if oneterm_terminal::ghostty_session::validate_input_size(size).is_err()
            || size > BYTE_BUDGET - self.channels.bytes.get()
        {
            self.channel_error("Input payload exceeds the channel byte budget".into(), cx);
            return;
        }
        let Some(sender) = &self.channels.sender else {
            return;
        };
        // Check before copying a bounded payload. Foreground admission is serial.
        if sender.is_full() || sender.is_closed() {
            self.channel_error(TerminalError::QueueFull.to_string(), cx);
            return;
        }
        let input = match input {
            BroadcastInput::Bytes(bytes) => PeerInput::Raw(bytes.to_vec()),
            BroadcastInput::Text(text) => PeerInput::Raw(text.as_bytes().to_vec()),
            BroadcastInput::Paste(text) => PeerInput::Paste(text.to_owned()),
            BroadcastInput::Interrupt => PeerInput::Raw(vec![3]),
        };
        self.channels.bytes.set(self.channels.bytes.get() + size);
        if let Err(error) = sender.try_send(Queued {
            input,
            bytes: self.channels.bytes.clone(),
            size,
        }) {
            self.channel_error(error.to_string(), cx);
        }
    }
}
