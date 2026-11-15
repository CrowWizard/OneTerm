//! Reply-first Ghostty transport seam, not a production session implementation.
//! Blocking: use on a backend worker, never GPUI or a tokio executor thread.

use std::fmt;
use std::sync::Arc;

use crate::ghostty::Error as NativeError;
use crate::ghostty::events::{Event, MAX_EVENT_BYTES, MAX_EVENTS};
use crate::ghostty::owner::{EngineOwner, Error as OwnerError, FrameReader, MAX_FEED_BYTES};
use crate::ghostty::render::{Frame, check_budget};
use crate::session::TerminalError;

use super::{OscRouter, PtyTransport};

#[derive(Debug)]
pub enum Error {
    Engine(OwnerError),
    Transport(TerminalError),
    Closed,
    PasteTooLarge { bytes: usize, max_bytes: usize },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Engine(error) => error.fmt(f),
            Self::Transport(error) => write!(f, "Ghostty transport operation failed: {error}"),
            Self::Closed => f.write_str("Ghostty transport pump is closed"),
            Self::PasteTooLarge { bytes, max_bytes } => {
                write!(f, "Paste is {bytes} bytes; maximum is {max_bytes}")
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Engine(error) => Some(error),
            Self::Transport(error) => Some(error),
            Self::Closed | Self::PasteTooLarge { .. } => None,
        }
    }
}

/// Owns exactly one native engine. No direct endpoint access can bypass reply
/// enqueue before publication. Only title/directory/bell policy is adapted yet.
pub struct GhosttyPump<T: PtyTransport> {
    engine: Option<EngineOwner>,
    transport: T,
    router: OscRouter<T>,
    pending: Vec<Event>,
    pending_bytes: usize,
}

impl<T: PtyTransport> GhosttyPump<T> {
    pub fn new(router: OscRouter<T>, cols: u16, rows: u16) -> Result<Self, Error> {
        let engine = EngineOwner::new(cols, rows).map_err(Error::Engine)?;
        Ok(Self {
            engine: Some(engine),
            transport: router.transport().clone(),
            router,
            pending: Vec::new(),
            pending_bytes: 0,
        })
    }

    pub fn reader(&self) -> Result<FrameReader, Error> {
        Ok(self.engine.as_ref().ok_or(Error::Closed)?.reader())
    }

    /// Position queries share the same output/resize ordering as native input.
    /// Rejected tokens/points do not fail-stop an otherwise healthy session.
    pub fn create_anchor(
        &mut self,
        point: crate::ghostty::anchors::Point,
    ) -> Result<crate::ghostty::anchors::AnchorId, Error> {
        self.anchor_request(|engine| engine.create_anchor(point))
    }

    pub fn resolve_anchor(
        &mut self,
        id: &crate::ghostty::anchors::AnchorId,
        space: crate::ghostty::anchors::Space,
    ) -> Result<Option<crate::ghostty::anchors::Position>, Error> {
        self.anchor_request(|engine| engine.resolve_anchor(id, space))
    }

    pub fn release_anchor(&mut self, id: &crate::ghostty::anchors::AnchorId) -> Result<(), Error> {
        self.anchor_request(|engine| engine.release_anchor(id))
    }

    pub fn set_selection(
        &mut self,
        start: &crate::ghostty::anchors::AnchorId,
        end: &crate::ghostty::anchors::AnchorId,
        rectangle: bool,
    ) -> Result<(), Error> {
        self.anchor_request(|engine| engine.set_selection(start, end, rectangle))
    }

    pub fn clear_selection(&mut self) -> Result<(), Error> {
        self.anchor_request(EngineOwner::clear_selection)
    }

    pub fn selection_text(&mut self) -> Result<Option<String>, Error> {
        self.anchor_request(EngineOwner::selection_text)
    }

    pub fn search(
        &mut self,
        needle: String,
        options: crate::ghostty::search::Options,
    ) -> Result<crate::ghostty::search::Snapshot, Error> {
        self.anchor_request(|engine| engine.search(needle, options))
    }

    pub fn search_step(&mut self) -> Result<crate::ghostty::search::Snapshot, Error> {
        self.anchor_request(EngineOwner::search_step)
    }

    fn anchor_request<R>(
        &mut self,
        call: impl FnOnce(&mut EngineOwner) -> Result<R, OwnerError>,
    ) -> Result<R, Error> {
        match call(self.engine.as_mut().ok_or(Error::Closed)?) {
            Ok(reply) => Ok(reply),
            Err(error @ OwnerError::Anchor(_)) => Err(Error::Engine(error)),
            Err(error @ OwnerError::Selection(_)) => Err(Error::Engine(error)),
            Err(error @ OwnerError::Search(_)) => Err(Error::Engine(error)),
            Err(error) => Err(self.stop(Error::Engine(error))),
        }
    }

    /// Encode against live native state and enqueue exactly once. Local
    /// validation errors leave the pump usable; native/delivery failures stop it.
    pub fn key(&mut self, key: crate::ghostty::input::KeyEvent) -> Result<(), Error> {
        if self.engine.is_none() {
            return Err(Error::Closed);
        }
        key.validate()
            .map_err(|error| Error::Engine(OwnerError::Native(error)))?;
        let bytes = match self.engine.as_mut().ok_or(Error::Closed)?.encode_key(key) {
            Ok(bytes) => bytes,
            Err(error) => return Err(self.stop(Error::Engine(error))),
        };
        self.deliver(&bytes)
    }

    pub fn mouse(&mut self, mouse: crate::ghostty::input::MouseEvent) -> Result<(), Error> {
        if self.engine.is_none() {
            return Err(Error::Closed);
        }
        mouse
            .validate()
            .map_err(|error| Error::Engine(OwnerError::Native(error)))?;
        let bytes = match self
            .engine
            .as_mut()
            .ok_or(Error::Closed)?
            .encode_mouse(mouse)
        {
            Ok(bytes) => bytes,
            Err(error) => return Err(self.stop(Error::Engine(error))),
        };
        self.deliver(&bytes)
    }

    /// Preserve the application's paste policy; only its mode comes from the
    /// native owner. No clipboard callbacks or paste-event grants are enabled.
    pub fn paste(&mut self, text: &str) -> Result<(), Error> {
        use crate::paste::{PasteMode, PastePolicy, PasteResult, encode_paste};
        if self.engine.is_none() {
            return Err(Error::Closed);
        }
        let policy = PastePolicy::default();
        if text.len() > policy.max_bytes {
            return Err(Error::PasteTooLarge {
                bytes: text.len(),
                max_bytes: policy.max_bytes,
            });
        }
        let enabled = match self.engine.as_mut().ok_or(Error::Closed)?.bracketed_paste() {
            Ok(enabled) => enabled,
            Err(error) => return Err(self.stop(Error::Engine(error))),
        };
        let mode = if enabled {
            PasteMode::Bracketed
        } else {
            PasteMode::Plain
        };
        match encode_paste(text, mode, &policy) {
            PasteResult::Ok(bytes) => self.deliver(&bytes),
            PasteResult::TooLarge(bytes) => Err(Error::PasteTooLarge {
                bytes,
                max_bytes: policy.max_bytes,
            }),
        }
    }

    /// Consume a read without publishing. Larger reads are split at the native
    /// request bound, draining replies between chunks, never reparsing on error.
    pub fn process_chunk(&mut self, bytes: &[u8]) -> Result<(), Error> {
        if self.engine.is_none() {
            return Err(Error::Closed);
        }
        self.router.logging().process(bytes);
        for chunk in bytes.chunks(MAX_FEED_BYTES) {
            let replies = match self.engine.as_mut().ok_or(Error::Closed)?.feed(chunk) {
                Ok(replies) => replies,
                Err(error) => return Err(self.stop(Error::Engine(error))),
            };
            self.deliver(&replies)?;
            let events = match self.engine.as_mut().ok_or(Error::Closed)?.take_events() {
                Ok(events) => events,
                Err(error) => return Err(self.stop(Error::Engine(error))),
            };
            let bytes: usize = events.iter().map(Event::text_len).sum();
            if events.len() > MAX_EVENTS - self.pending.len()
                || bytes > MAX_EVENT_BYTES - self.pending_bytes
            {
                return Err(self.stop(Error::Engine(OwnerError::Native(
                    NativeError::EventOverflow,
                ))));
            }
            if self.pending.try_reserve(events.len()).is_err() {
                return Err(self.stop(Error::Engine(OwnerError::Native(NativeError::Allocation))));
            }
            self.pending.extend(events);
            self.pending_bytes += bytes;
        }
        Ok(())
    }

    /// Publish only after every preceding reply was accepted by the transport.
    /// Reliable events apply backpressure outside native work, then repaint is
    /// coalesced through the existing event sink. Missing OSC families remain gaps.
    pub fn finish_batch(&mut self) -> Result<Arc<Frame>, Error> {
        if self.engine.is_none() {
            return Err(Error::Closed);
        }
        self.flush_events();
        let frame = match self.engine.as_mut().ok_or(Error::Closed)?.capture() {
            Ok(frame) => frame,
            Err(error) => return Err(self.stop(Error::Engine(error))),
        };
        if self.router.state().claim_repaint_hint() && !self.router.events().post_repaint() {
            self.router.state().release_repaint_hint();
        }
        Ok(frame)
    }

    fn flush_events(&mut self) {
        let mut routed = Vec::new();
        for event in self.pending.drain(..) {
            self.router.route_ghostty(event, &mut routed);
            for event in routed.drain(..) {
                self.router.events().send_blocking(event);
            }
        }
        self.pending_bytes = 0;
    }

    pub fn resize(
        &mut self,
        cols: u16,
        rows: u16,
        cell_width_px: u32,
        cell_height_px: u32,
    ) -> Result<(), Error> {
        if self.engine.is_none() {
            return Err(Error::Closed);
        }
        // Reject user dimensions before either native or transport mutation.
        if cols == 0 || rows == 0 {
            return Err(Error::Engine(OwnerError::Native(NativeError::InvalidSize)));
        }
        check_budget(cols, rows).map_err(|error| Error::Engine(OwnerError::Native(error)))?;
        let replies = match self.engine.as_mut().ok_or(Error::Closed)?.resize(
            cols,
            rows,
            cell_width_px,
            cell_height_px,
        ) {
            Ok(replies) => replies,
            Err(error) => return Err(self.stop(Error::Engine(error))),
        };
        if let Err(error) = self.transport.pty_resize(rows, cols) {
            return Err(self.stop(Error::Transport(error)));
        }
        self.deliver(&replies)
    }

    /// Consuming, explicit worker-path close; transport close is out of band.
    pub fn close(mut self) -> Result<(), Error> {
        if let Err(error) = self.router.logging().stop() {
            log::warn!("Native terminal logging final flush failed: {error}");
        }
        self.flush_events();
        match self.engine.take() {
            Some(engine) => {
                let transport = self.transport.pty_close();
                let native = engine.close();
                if let Err(error) = &transport {
                    log::warn!("Ghostty pump transport close failed: {error}");
                }
                native.map_err(Error::Engine)?;
                transport.map_err(Error::Transport)
            }
            None => Ok(()),
        }
    }

    fn deliver(&mut self, replies: &[u8]) -> Result<(), Error> {
        if !replies.is_empty() {
            if let Err(error) = self.transport.pty_write(replies) {
                return Err(self.stop(Error::Transport(error)));
            }
        }
        Ok(())
    }

    /// Backend-worker viewport command. Like feed/resize, this does not publish
    /// until finish_batch, preserving pending reliable-event ordering.
    pub fn scroll(&mut self, scroll: crate::ghostty::state::Scroll) -> Result<(), Error> {
        match self.engine.as_mut().ok_or(Error::Closed)?.scroll(scroll) {
            Ok(()) => Ok(()),
            Err(error) => Err(self.stop(Error::Engine(error))),
        }
    }

    fn stop(&mut self, cause: Error) -> Error {
        if let Err(error) = self.router.logging().stop() {
            log::warn!("Native terminal logging flush after {cause} failed: {error}");
        }
        self.pending.clear();
        self.pending_bytes = 0;
        if let Err(error) = self.transport.pty_close() {
            log::warn!("Ghostty pump transport close after {cause} failed: {error}");
        }
        if let Some(engine) = self.engine.take() {
            if let Err(error) = engine.close() {
                log::debug!("Ghostty pump worker teardown after {cause}: {error}");
            }
        }
        cause
    }
}

impl<T: PtyTransport> Drop for GhosttyPump<T> {
    fn drop(&mut self) {
        if let Err(error) = self.router.logging().stop() {
            log::warn!("Native terminal logging flush during drop failed: {error}");
        }
        if self.engine.is_some() {
            if let Err(error) = self.transport.pty_close() {
                log::warn!("Ghostty pump transport close during drop failed: {error}");
            }
        }
        // EngineOwner Drop disconnects without joining on this caller.
    }
}

#[cfg(test)]
#[path = "ghostty_pump_tests.rs"]
mod tests;
