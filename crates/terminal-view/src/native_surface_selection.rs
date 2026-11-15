//! One gesture task and one replaceable motion sample, never one task per move.

use std::{cell::RefCell, rc::Rc};

use gpui::{Context, MouseButton, Pixels, Point, Task};
use oneterm_terminal::ghostty::{
    anchors::Coordinate,
    selection::{Gesture, GestureId, Update},
};
use oneterm_terminal::ghostty_content::Viewport;

use super::{NativeSurfaceError, NativeTerminalSurface};

type Sample = (Viewport, Coordinate);

#[derive(Default)]
struct Pending {
    sample: Option<Sample>,
    finish: bool,
}

#[derive(Default)]
pub(super) struct Drag {
    pending: Rc<RefCell<Pending>>,
    wake: Option<async_channel::Sender<()>>,
    task: Option<Task<()>>,
}

impl Drop for Drag {
    fn drop(&mut self) {
        // View teardown must finish an idle gesture too: there may be no admitted
        // endpoint future whose cancellation guard could otherwise close it.
        self.pending.borrow_mut().finish = true;
        if let Some(wake) = self.wake.take() {
            match wake.try_send(()) {
                Ok(()) | Err(async_channel::TrySendError::Full(_)) => {}
                Err(async_channel::TrySendError::Closed(_)) => {
                    log::debug!("Native drag already finished during teardown")
                }
            }
        }
        if let Some(task) = self.task.take() {
            task.detach();
        }
    }
}

#[cfg(test)]
impl Drag {
    pub(super) fn active(&self) -> bool {
        self.wake.is_some()
    }
}

impl NativeTerminalSurface {
    fn selection_sample(&self, position: Point<Pixels>) -> Option<Sample> {
        let state = self.state.borrow();
        let geometry = state.geometry?;
        let viewport = state.frame.native_viewport()?;
        if viewport.columns() != geometry.size.cols
            || viewport.lines() != usize::from(geometry.size.rows)
        {
            return None;
        }
        let (row, col) = geometry.pixel_to_grid(position)?;
        if !row.is_finite() || !col.is_finite() {
            return None;
        }
        Some((
            viewport,
            Coordinate {
                column: col.floor() as u16,
                row: row.floor() as u32,
            },
        ))
    }

    pub(super) fn selection_down(
        &mut self,
        position: Point<Pixels>,
        shift: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some((viewport, coordinate)) = self.selection_sample(position) else {
            return false;
        };
        let modes = &viewport.metadata().modes;
        if !shift
            && (modes.mouse_x10 || modes.mouse_normal || modes.mouse_button || modes.mouse_any)
        {
            return false;
        }
        if self.drag.wake.is_some() {
            (self.on_error)(
                NativeSurfaceError::Edit(
                    oneterm_terminal::backend::AsyncGhosttyPumpError::Transport(
                        oneterm_terminal::TerminalError::QueueFull,
                    ),
                ),
                cx,
            );
            return true;
        }
        let (tx, rx) = async_channel::bounded(1);
        self.drag.pending = Rc::new(RefCell::new(Pending::default()));
        self.drag.wake = Some(tx);
        let pending = self.drag.pending.clone();
        let session = self.session.clone();
        self.drag.task = Some(cx.spawn(async move |surface, cx| {
            let id = GestureId::default();
            let mut result = session
                .selection(Update::Gesture(Gesture::Begin {
                    id: id.clone(),
                    viewport,
                    coordinate,
                }))
                .await;
            while result.is_ok() {
                if surface.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
                if rx.recv().await.is_err() {
                    break;
                }
                let (sample, finish) = {
                    let mut pending = pending.borrow_mut();
                    (pending.sample.take(), pending.finish)
                };
                if let Some((viewport, coordinate)) = sample {
                    result = session
                        .selection(Update::Gesture(Gesture::Move {
                            id: id.clone(),
                            viewport,
                            coordinate,
                        }))
                        .await;
                }
                if finish {
                    break;
                }
            }
            // Finish is idempotent, also after a stale/rejected begin or move.
            let cleanup = session
                .selection(Update::Gesture(Gesture::Finish(id)))
                .await;
            if let Err(error) = surface.update(cx, |surface, cx| {
                if Rc::ptr_eq(&surface.drag.pending, &pending) {
                    surface.drag.wake = None;
                }
                if let Err(error) = result {
                    (surface.on_error)(NativeSurfaceError::Edit(error), cx);
                }
                if let Err(error) = cleanup {
                    (surface.on_error)(NativeSurfaceError::Edit(error), cx);
                }
                cx.notify();
            }) {
                log::debug!("Native drag recipient closed: {error}");
            }
        }));
        true
    }

    pub(super) fn selection_move(
        &mut self,
        position: Point<Pixels>,
        button: Option<MouseButton>,
    ) -> bool {
        if self.drag.wake.is_none() {
            return false;
        }
        if button != Some(MouseButton::Left) {
            self.finish_selection();
            return true;
        }
        if !self.drag.pending.borrow().finish {
            if let Some(sample) = self.selection_sample(position) {
                self.drag.pending.borrow_mut().sample = Some(sample);
                self.wake_selection();
            }
        }
        true
    }

    pub(super) fn finish_selection(&mut self) -> bool {
        if self.drag.wake.is_none() {
            return false;
        }
        self.drag.pending.borrow_mut().finish = true;
        self.wake_selection();
        true
    }

    fn wake_selection(&self) {
        if let Some(wake) = &self.drag.wake {
            match wake.try_send(()) {
                Ok(()) | Err(async_channel::TrySendError::Full(_)) => {}
                Err(async_channel::TrySendError::Closed(_)) => {
                    log::debug!("Native drag task ended")
                }
            }
        }
    }
}
