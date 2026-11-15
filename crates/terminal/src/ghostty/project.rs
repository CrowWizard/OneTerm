//! Native viewport to shared paint values. Not a complete session snapshot:
//! ordered history, hyperlinks, graphics and semantic marks are not
//! available through this staging projection. Never select it as a session yet.

use std::ops::Range;
use std::sync::Arc;

use crate::ghostty::owner::{Damage, FrameReader, State};
use crate::ghostty::{Error, render};
use crate::{
    Attrs, CellWidth, Color, RenderRow, Rgb, Semantic, SnapshotCell, SnapshotContent, Style,
    StyleRun,
};

/// Immutable, exact native row content, retained by equality-only cache keys.
#[derive(Debug, PartialEq, Eq)]
pub struct ProjectedRow {
    identity: render::RowIdentity,
    pub(crate) wrapped: bool,
    continuation: bool,
    pub(crate) cells: Vec<SnapshotCell>,
    pub(crate) runs: Vec<StyleRun>,
    pub(crate) clusters: Vec<char>,
    pub(crate) foreground_exact: Vec<bool>,
    pub(crate) foreground_default: Vec<bool>,
}

/// Rows and metadata from one immutable capture. This object intentionally does
/// not invent legacy modes or ordered history positions.
pub struct ProjectedFrame {
    pub(crate) grid_revision: Arc<()>,
    source: Arc<render::Frame>,
    rows: Vec<Arc<ProjectedRow>>,
    selection: Vec<SelectionSpan>,
}

/// Exact viewport mask from the same immutable capture; columns are half-open.
#[derive(Debug, PartialEq, Eq)]
pub struct SelectionSpan {
    pub row: u16,
    pub columns: Range<u16>,
}

impl ProjectedFrame {
    /// Retain the original capture without copying its viewport again.
    pub fn new(frame: Arc<render::Frame>) -> Result<Self, Error> {
        let height = u16::try_from(frame.rows.len()).map_err(|_| Error::SnapshotLimit)?;
        render::check_budget(frame.cols, height)?;
        let mut bytes = 0usize;
        for row in &frame.rows {
            if row.cells.len() != usize::from(frame.cols) {
                return Err(Error::InvalidSnapshot(
                    "projected row width differs from viewport",
                ));
            }
            for cell in &row.cells {
                bytes = bytes
                    .checked_add(cell.text.len())
                    .ok_or(Error::SnapshotLimit)?;
                if bytes > render::MAX_TEXT_BYTES {
                    return Err(Error::SnapshotLimit);
                }
            }
        }
        let mut rows = Vec::new();
        let mut selection = Vec::new();
        rows.try_reserve_exact(frame.rows.len())
            .map_err(|_| Error::Allocation)?;
        for (y, row) in frame.rows.iter().enumerate() {
            rows.push(Arc::new(project_row(row, &frame)?));
            let mut x = 0;
            while x < row.cells.len() {
                if !row.cells[x].selected {
                    x += 1;
                    continue;
                }
                let start = x;
                while x < row.cells.len() && row.cells[x].selected {
                    x += 1;
                }
                selection.try_reserve(1).map_err(|_| Error::Allocation)?;
                selection.push(SelectionSpan {
                    row: y as u16,
                    columns: start as u16..x as u16,
                });
            }
        }
        Ok(Self {
            grid_revision: Arc::new(()),
            source: frame,
            rows,
            selection,
        })
    }

    pub fn source(&self) -> &Arc<render::Frame> {
        &self.source
    }

    pub fn row(&self, index: usize) -> Option<RenderRow<'_>> {
        self.rows.get(index).map(RenderRow::Ghostty)
    }

    pub(crate) fn projected_rows(&self) -> &[Arc<ProjectedRow>] {
        &self.rows
    }

    pub fn selection_spans(&self) -> &[SelectionSpan] {
        &self.selection
    }
}

/// Backend-side paint preparation; never call allocating poll from GPUI
/// prepaint or directly on a tokio executor. The last good frame survives errors.
pub struct ProjectedReader {
    reader: FrameReader,
    current: Option<Arc<ProjectedFrame>>,
}

pub struct ProjectedUpdate {
    pub frame: Option<Arc<ProjectedFrame>>,
    pub damage: Damage,
    pub state: State,
}

impl ProjectedReader {
    pub fn new(reader: FrameReader) -> Self {
        Self {
            reader,
            current: None,
        }
    }

    pub fn current(&self) -> Option<&Arc<ProjectedFrame>> {
        self.current.as_ref()
    }

    pub fn poll(&mut self) -> Result<ProjectedUpdate, Error> {
        self.poll_with(ProjectedFrame::new)
    }

    fn poll_with(
        &mut self,
        project: impl FnOnce(Arc<render::Frame>) -> Result<ProjectedFrame, Error>,
    ) -> Result<ProjectedUpdate, Error> {
        let current = &self.current;
        let mut prepared = None;
        let update = self.reader.poll_prepared(|frame| {
            if let Some(frame) = frame {
                if !current
                    .as_ref()
                    .is_some_and(|old| Arc::ptr_eq(old.source(), frame))
                {
                    prepared = Some(Arc::new(project(frame.clone())?));
                }
            }
            Ok(())
        })?;
        if let Some(prepared) = prepared {
            self.current = Some(prepared);
        }
        Ok(ProjectedUpdate {
            frame: self.current.clone(),
            damage: update.damage,
            state: update.state,
        })
    }
}

fn project_row(row: &render::Row, frame: &render::Frame) -> Result<ProjectedRow, Error> {
    let mut out = ProjectedRow {
        identity: row.identity,
        wrapped: row.wrapped,
        continuation: row.wrap_continuation,
        cells: Vec::new(),
        runs: Vec::new(),
        clusters: Vec::new(),
        foreground_exact: Vec::new(),
        foreground_default: Vec::new(),
    };
    out.cells
        .try_reserve_exact(row.cells.len())
        .map_err(|_| Error::Allocation)?;
    out.runs
        .try_reserve_exact(row.cells.len())
        .map_err(|_| Error::Allocation)?;
    out.foreground_exact
        .try_reserve_exact(row.cells.len())
        .map_err(|_| Error::Allocation)?;
    out.foreground_default
        .try_reserve_exact(row.cells.len())
        .map_err(|_| Error::Allocation)?;
    let scalars: usize = row.cells.iter().map(|cell| cell.text.chars().count()).sum();
    out.clusters
        .try_reserve_exact(scalars)
        .map_err(|_| Error::Allocation)?;
    for (col, cell) in row.cells.iter().enumerate() {
        let style = project_style(cell, frame)?;
        out.foreground_default
            .push(matches!(cell.style.foreground, render::Color::Default));
        out.foreground_exact.push(matches!(
            cell.style.foreground,
            render::Color::Rgb(_) | render::Color::Palette(16..=255)
        ));
        let col = col as u16;
        if out.runs.last().is_some_and(|run| run.style == style) {
            if let Some(run) = out.runs.last_mut() {
                run.cols.end = col + 1;
            }
        } else {
            out.runs.push(StyleRun {
                cols: col..col + 1,
                style,
            });
        }
        let mut chars = cell.text.chars();
        let first = chars.next().unwrap_or(' ');
        let content = if chars.next().is_none() {
            SnapshotContent::Scalar(first)
        } else {
            let start = out.clusters.len() as u32;
            out.clusters.extend(cell.text.chars());
            SnapshotContent::Cluster {
                start,
                len: out.clusters.len() as u32 - start,
            }
        };
        out.cells.push(SnapshotCell {
            content,
            width: match cell.width {
                render::Width::Narrow => CellWidth::Narrow,
                render::Width::Wide => CellWidth::Wide,
                render::Width::SpacerTail => CellWidth::WideSpacer,
                render::Width::SpacerHead => CellWidth::LeadingWideSpacer,
            },
            semantic: Semantic::None,
            run: (out.runs.len() - 1) as u16,
            hyperlink: None,
            graphic: None,
        });
    }
    Ok(out)
}

fn project_style(cell: &render::Cell, frame: &render::Frame) -> Result<Style, Error> {
    let flags = cell.style.flags;
    if flags & 0xf800 != 0 {
        return Err(Error::InvalidSnapshot("reserved native style flags"));
    }
    let mut attrs = Attrs::empty();
    for (bit, attr) in [
        Attrs::BOLD,
        Attrs::ITALIC,
        Attrs::DIM,
        Attrs::BLINK_SLOW,
        Attrs::INVERSE,
        Attrs::HIDDEN,
        Attrs::STRIKEOUT,
        Attrs::OVERLINE,
    ]
    .into_iter()
    .enumerate()
    {
        if flags & (1 << bit) != 0 {
            attrs |= attr;
        }
    }
    attrs |= match (flags >> 8) & 7 {
        0 => Attrs::empty(),
        1 => Attrs::UNDERLINE,
        2 => Attrs::DOUBLE_UNDERLINE,
        3 => Attrs::UNDERCURL,
        4 => Attrs::DOTTED_UNDERLINE,
        5 => Attrs::DASHED_UNDERLINE,
        _ => return Err(Error::InvalidSnapshot("unknown native underline style")),
    };
    let rgb = |color: render::Rgb| {
        Color::Rgb(Rgb {
            r: color.r,
            g: color.g,
            b: color.b,
        })
    };
    let resolve = |color| match color {
        render::Color::Default => None,
        render::Color::Palette(index) => Some(rgb(frame.palette[usize::from(index)])),
        render::Color::Rgb(color) => Some(rgb(color)),
    };
    Ok(Style {
        fg: cell
            .foreground
            .map(rgb)
            .or_else(|| resolve(cell.style.foreground))
            .unwrap_or_else(|| rgb(frame.foreground)),
        bg: cell
            .background
            .map(rgb)
            .or_else(|| resolve(cell.style.background))
            .unwrap_or_else(|| rgb(frame.background)),
        underline_color: resolve(cell.style.underline),
        attrs,
    })
}

#[cfg(test)]
#[path = "project_tests.rs"]
mod tests;
