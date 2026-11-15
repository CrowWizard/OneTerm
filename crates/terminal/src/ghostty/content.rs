//! Per-consumer native content. No parser access, projection or repaint ack.

use std::sync::Arc;

use crate::RenderRow;
use crate::ghostty::search;
use crate::ghostty::{owner::Damage, render, state};
use crate::ghostty_bridge::{Error, State};
use crate::ghostty_model::{ModelReader, SharedModel};
use crate::ghostty_projection::{ProjectedFrame, SelectionSpan};

/// The render-only portion of an explicit native session. This does not claim
/// the legacy TerminalSession's input, history, graphics or search contracts.
pub trait RenderSource: Send + Sync {
    fn render_model(&self) -> SharedModel;
}

impl RenderSource for SharedModel {
    fn render_model(&self) -> SharedModel {
        self.clone()
    }
}

/// One view's reusable reader and damage watermark. Not Clone: a second view
/// opens its own Content rather than sharing another view's damage progress.
pub struct Content {
    reader: ModelReader,
    viewport: Option<Viewport>,
    damage: Damage,
    state: State,
}

impl Content {
    pub fn new(source: &dyn RenderSource) -> Self {
        Self {
            reader: source.render_model().reader(),
            viewport: None,
            damage: Damage::None,
            state: State::Starting,
        }
    }

    /// Retain one prepared publication and advance only this consumer's damage.
    /// Projection/comparison errors leave the previous content untouched.
    pub fn poll(&mut self) -> Result<(), Error> {
        let update = self.reader.poll()?;
        self.viewport = update.frame.map(Viewport);
        self.damage = update.damage;
        self.state = update.state;
        Ok(())
    }

    pub fn viewport(&self) -> Option<&Viewport> {
        self.viewport.as_ref()
    }

    /// Compare publication ownership, not the current frame or native pointer.
    /// A view reused for another session must open a fresh damage consumer.
    pub fn is_source(&self, source: &dyn RenderSource) -> bool {
        self.reader.is_source(&source.render_model())
    }

    pub fn damage(&self) -> &Damage {
        &self.damage
    }

    pub fn state(&self) -> &State {
        &self.state
    }
}

/// One immutable prepared viewport; cloning retains the exact publication even
/// after the source resizes, scrolls, fails or closes. No legacy RowId is made.
#[derive(Clone)]
pub struct Viewport(pub(crate) Arc<ProjectedFrame>);

/// Damage-free read of one last-published state, not a live owner-mode query.
pub struct Query {
    pub viewport: Option<Viewport>,
    pub state: State,
    pub(crate) grid_revision: Arc<()>,
}

impl Query {
    /// Also rejects mutation that has not yet produced a replacement viewport.
    pub fn matches_search(&self, result: &search::Snapshot) -> bool {
        result
            .grid_revision
            .as_ref()
            .is_some_and(|revision| Arc::ptr_eq(revision, &self.grid_revision))
            && self
                .viewport
                .as_ref()
                .is_some_and(|viewport| viewport.matches_search(result))
    }
}

impl Viewport {
    /// Whether an ephemeral native-search response belongs to this exact grid revision.
    pub fn matches_search(&self, result: &search::Snapshot) -> bool {
        result
            .grid_revision
            .as_ref()
            .is_some_and(|revision| Arc::ptr_eq(revision, &self.0.grid_revision))
    }
    pub fn columns(&self) -> u16 {
        self.0.source().cols
    }

    pub fn lines(&self) -> usize {
        self.0.source().rows.len()
    }

    pub fn row(&self, index: usize) -> Option<RenderRow<'_>> {
        self.0.row(index)
    }

    /// A clamped display-row window, not ordered history. No allocation, capture
    /// or damage advancement; even usize::MAX inputs are safe.
    pub fn rows(&self, start: usize, count: usize) -> impl ExactSizeIterator<Item = RenderRow<'_>> {
        let start = start.min(self.lines());
        let end = start.saturating_add(count).min(self.lines());
        self.0.projected_rows()[start..end]
            .iter()
            .map(RenderRow::Ghostty)
    }

    pub fn cursor(&self) -> &render::Cursor {
        &self.0.source().cursor
    }

    pub fn metadata(&self) -> &state::Metadata {
        &self.0.source().metadata
    }

    pub fn selection_spans(&self) -> &[SelectionSpan] {
        self.0.selection_spans()
    }

    /// Effective colors, including program OSC overrides. These must not be
    /// fed back to a session as caller defaults.
    pub fn foreground(&self) -> render::Rgb {
        self.0.source().foreground
    }

    pub fn background(&self) -> render::Rgb {
        self.0.source().background
    }

    pub fn cursor_color(&self) -> Option<render::Rgb> {
        self.0.source().cursor_color
    }

    pub fn palette(&self) -> &[render::Rgb; 256] {
        &self.0.source().palette
    }
}
