//! Pane types and their shared shell.

pub mod editor;
pub mod explorer;

use editor::Editor;
use explorer::Explorer;

pub enum PaneKind {
    Editor(Editor),
    Explorer(Explorer),
}

pub struct Pane {
    pub id: usize,
    pub rect: ratatui::layout::Rect,
    pub kind: PaneKind,
}

impl Pane {
    pub fn new(id: usize, kind: PaneKind) -> Self {
        Self {
            id,
            rect: ratatui::layout::Rect::default(),
            kind,
        }
    }
}
