//! Borrowed paint values, independent of ordered history coordinates.

use oneterm_vt::{RowId, SeqNo, SnapshotCell, SnapshotRow, Style, StyleRun};

/// Equality-only cache key. A native key is not a history position or sequence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RenderRowKey {
    Snapshot {
        id: RowId,
        seq: SeqNo,
    },
    #[cfg(feature = "ghostty-native")]
    Ghostty(std::sync::Arc<crate::ghostty_projection::ProjectedRow>),
}

/// A row borrowed from one immutable frame. No parser or native handle is held.
#[derive(Clone, Copy)]
pub enum RenderRow<'a> {
    Snapshot(&'a SnapshotRow),
    #[cfg(feature = "ghostty-native")]
    Ghostty(&'a std::sync::Arc<crate::ghostty_projection::ProjectedRow>),
}

impl<'a> RenderRow<'a> {
    pub fn key(self) -> RenderRowKey {
        match self {
            Self::Snapshot(row) => RenderRowKey::Snapshot {
                id: row.id,
                seq: row.seq,
            },
            #[cfg(feature = "ghostty-native")]
            Self::Ghostty(row) => RenderRowKey::Ghostty(row.clone()),
        }
    }

    /// Only an actual history anchor may answer this. Opaque native identities
    /// cannot stand in for the old engine's ordered logical coordinates.
    pub fn logical_id(self) -> Option<RowId> {
        match self {
            Self::Snapshot(row) => Some(row.id),
            #[cfg(feature = "ghostty-native")]
            Self::Ghostty(_) => None,
        }
    }

    pub fn cells(self) -> &'a [SnapshotCell] {
        match self {
            Self::Snapshot(row) => &row.cells,
            #[cfg(feature = "ghostty-native")]
            Self::Ghostty(row) => &row.cells,
        }
    }

    pub fn runs(self) -> &'a [StyleRun] {
        match self {
            Self::Snapshot(row) => &row.runs,
            #[cfg(feature = "ghostty-native")]
            Self::Ghostty(row) => &row.runs,
        }
    }

    pub fn wrapped(self) -> bool {
        match self {
            Self::Snapshot(row) => row.wrapped,
            #[cfg(feature = "ghostty-native")]
            Self::Ghostty(row) => row.wrapped,
        }
    }

    /// Native RGB projection retains whether the foreground was an explicit
    /// truecolor/extended-palette request. None leaves snapshot policy intact.
    #[cfg(feature = "ghostty-native")]
    pub fn native_foreground_is_exact(self, column: usize) -> Option<bool> {
        match self {
            Self::Snapshot(_) => None,
            Self::Ghostty(row) => row.foreground_exact.get(column).copied(),
        }
    }

    /// Native effective RGB retains default foreground eligibility for the
    /// shared semantic overlay without conflating equal-valued ANSI colors.
    #[cfg(feature = "ghostty-native")]
    pub fn native_foreground_is_default(self, column: usize) -> Option<bool> {
        match self {
            Self::Snapshot(_) => None,
            Self::Ghostty(row) => row.foreground_default.get(column).copied(),
        }
    }

    pub fn style_of(self, cell: &SnapshotCell) -> &'a Style {
        self.runs()
            .get(usize::from(cell.run))
            .map_or(&Style::DEFAULT, |run| &run.style)
    }

    pub fn cluster(self, start: u32, len: u32) -> &'a [char] {
        let clusters = match self {
            Self::Snapshot(row) => &row.clusters,
            #[cfg(feature = "ghostty-native")]
            Self::Ghostty(row) => &row.clusters,
        };
        let start = start as usize;
        start
            .checked_add(len as usize)
            .and_then(|end| clusters.get(start..end))
            .unwrap_or(&[])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oneterm_vt::{CellWidth, Semantic, SnapshotContent};

    #[test]
    fn snapshot_values_are_borrowed_without_copying_or_changing_keys() {
        let row = SnapshotRow {
            id: RowId(17),
            seq: SeqNo(29),
            wrapped: true,
            cells: vec![SnapshotCell {
                content: SnapshotContent::Cluster { start: 0, len: 2 },
                width: CellWidth::Narrow,
                semantic: Semantic::None,
                run: 0,
                hyperlink: None,
                graphic: None,
            }],
            runs: vec![StyleRun {
                cols: 0..1,
                style: Style::DEFAULT,
            }],
            clusters: vec!['e', '\u{301}'],
        };
        let view = RenderRow::Snapshot(&row);
        assert_eq!(
            view.key(),
            RenderRowKey::Snapshot {
                id: row.id,
                seq: row.seq
            }
        );
        assert_eq!(view.logical_id(), Some(row.id));
        assert!(view.wrapped());
        assert!(std::ptr::eq(view.cells().as_ptr(), row.cells.as_ptr()));
        assert!(std::ptr::eq(
            view.style_of(&row.cells[0]),
            &row.runs[0].style
        ));
        assert!(std::ptr::eq(
            view.cluster(0, 2).as_ptr(),
            row.clusters.as_ptr()
        ));
        assert!(view.cluster(u32::MAX, u32::MAX).is_empty());
    }
}
