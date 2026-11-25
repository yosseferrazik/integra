//! Application state: panes, focus, mosaic tree and high-level actions.

use std::path::PathBuf;

use ratatui::layout::Rect;

use crate::layout::{self, Axis, Divider, Node};
use crate::panes::{Pane, PaneKind, editor::Editor, explorer::Explorer};

#[derive(Clone)]
pub struct Drag {
    pub path: Vec<bool>,
    pub parent: Rect,
    pub axis: Axis,
}

pub struct App {
    pub root: Node,
    pub panes: Vec<Pane>,
    pub focus: usize,
    pub dividers: Vec<Divider>,
    pub drag: Option<Drag>,
    pub status: String,
    pub quit_armed: bool,
    pub running: bool,
    next_id: usize,
}

impl App {
    pub fn new(file: Option<PathBuf>, start_dir: Option<PathBuf>) -> Self {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let mut status = String::new();
        let mut panes = vec![Pane::new(
            0,
            PaneKind::Explorer(Explorer::new(start_dir.unwrap_or(cwd))),
        )];
        // Keep an editor open by default; a missing file becomes a scratch buffer.
        let editor = match file {
            Some(path) => match Editor::open(&path) {
                Ok(editor) => editor,
                Err(error) => {
                    status = format!("Cannot open {}: {error}", path.display());
                    Editor::scratch()
                }
            },
            None => Editor::scratch(),
        };
        panes.push(Pane::new(1, PaneKind::Editor(editor)));
        Self {
            root: Node::split(Axis::X, 30, Node::leaf(0), Node::leaf(1)),
            panes,
            focus: 1,
            dividers: Vec::new(),
            drag: None,
            status,
            quit_armed: false,
            running: true,
            next_id: 2,
        }
    }

    pub fn focused_pane(&self) -> Option<&Pane> {
        self.panes.iter().find(|pane| pane.id == self.focus)
    }

    pub fn focused_editor_mut(&mut self) -> Option<&mut Editor> {
        match &mut self.focused_pane_mut()?.kind {
            PaneKind::Editor(editor) => Some(editor),
            PaneKind::Explorer(_) => None,
        }
    }

    fn focused_pane_mut(&mut self) -> Option<&mut Pane> {
        self.panes.iter_mut().find(|pane| pane.id == self.focus)
    }

    pub fn focused_explorer_mut(&mut self) -> Option<&mut Explorer> {
        match &mut self.focused_pane_mut()?.kind {
            PaneKind::Explorer(explorer) => Some(explorer),
            PaneKind::Editor(_) => None,
        }
    }

    fn focused_explorer(&self) -> Option<&Explorer> {
        match &self.focused_pane()?.kind {
            PaneKind::Explorer(explorer) => Some(explorer),
            PaneKind::Editor(_) => None,
        }
    }

    pub fn pane_mut(&mut self, id: usize) -> Option<&mut Pane> {
        self.panes.iter_mut().find(|pane| pane.id == id)
    }

    /// Pane under a screen position, ignoring zero-sized panes.
    pub fn pane_at(&self, column: u16, row: u16) -> Option<usize> {
        self.panes
            .iter()
            .find(|pane| {
                pane.rect.width > 0 && pane.rect.height > 0 && rect_contains(pane.rect, column, row)
            })
            .map(|pane| pane.id)
    }

    pub fn divider_at(&self, column: u16, row: u16) -> Option<Divider> {
        self.dividers
            .iter()
            .find(|divider| rect_contains(divider.rect, column, row))
            .cloned()
    }

    pub fn drag_ratio(&mut self, column: u16, row: u16) {
        let Some(drag) = self.drag.clone() else {
            return;
        };
        let total = match drag.axis {
            Axis::X => drag.parent.width,
            Axis::Y => drag.parent.height,
        } as u32;
        if total == 0 {
            return;
        }
        let position = match drag.axis {
            Axis::X => column.saturating_sub(drag.parent.x),
            Axis::Y => row.saturating_sub(drag.parent.y),
        } as u32;
        let ratio = (position * 100 / total)
            .clamp(layout::MIN_RATIO as u32, layout::MAX_RATIO as u32) as u16;
        layout::set_ratio(&mut self.root, &drag.path, ratio);
    }

    pub fn focus_next(&mut self, step: isize) {
        if self.panes.is_empty() {
            return;
        }
        let Some(current) = self.panes.iter().position(|pane| pane.id == self.focus) else {
            self.focus = self.panes[0].id;
            return;
        };
        let last = self.panes.len() as isize;
        let next = (current as isize + step).rem_euclid(last) as usize;
        self.focus = self.panes[next].id;
        self.status.clear();
    }

    pub fn split_focused(&mut self, axis: Axis) {
        if self.add_pane(PaneKind::Editor(Editor::scratch()), axis) {
            self.status = "New editor".into();
        } else {
            self.status = "Cannot split pane".into();
        }
    }

    pub fn close_focused(&mut self) {
        if self.panes.len() <= 1 {
            self.status = "Cannot close the last pane".into();
            return;
        }
        let id = self.focus;
        let Some(root) = layout::remove_leaf(&self.root, id) else {
            self.status = "Cannot close pane".into();
            return;
        };
        self.root = root;
        self.panes.retain(|pane| pane.id != id);
        self.focus = self
            .panes
            .iter()
            .find(|pane| matches!(&pane.kind, PaneKind::Editor(_)))
            .or_else(|| self.panes.first())
            .map(|pane| pane.id)
            .unwrap_or(0);
        self.status = "Pane closed".into();
    }

    pub fn resize_focused(&mut self, axis: Axis, delta: i16) {
        if !layout::resize(&mut self.root, self.focus, axis, delta) {
            self.status = "No split on that axis".into();
        }
    }

    pub fn save_focused(&mut self) {
        let Some(editor) = self.focused_editor_mut() else {
            self.status = "Focus is not an editor".into();
            return;
        };
        let message = match editor.save() {
            Ok(()) => format!("Saved {}", editor.title()),
            Err(error) => format!("Save failed: {error}"),
        };
        self.status = message;
    }

    pub fn quit(&mut self) {
        let modified = self
            .panes
            .iter()
            .any(|pane| matches!(&pane.kind, PaneKind::Editor(editor) if editor.modified));
        if modified && !self.quit_armed {
            self.quit_armed = true;
            self.status = "Unsaved changes; Ctrl-Q again to quit".into();
        } else {
            self.running = false;
        }
    }

    pub fn open_file(&mut self, path: PathBuf) {
        let editor = match Editor::open(&path) {
            Ok(editor) => editor,
            Err(error) => {
                self.status = format!("Cannot open {}: {error}", path.display());
                return;
            }
        };
        // Reuse a clean editor, preferring the focused one, so nothing is lost.
        let target = self
            .panes
            .iter()
            .find(|pane| {
                pane.id == self.focus
                    && matches!(&pane.kind, PaneKind::Editor(editor) if !editor.modified)
            })
            .or_else(|| {
                self.panes
                    .iter()
                    .find(|pane| matches!(&pane.kind, PaneKind::Editor(editor) if !editor.modified))
            })
            .map(|pane| pane.id);
        if let Some(id) = target {
            if let Some(pane) = self.pane_mut(id) {
                pane.kind = PaneKind::Editor(editor);
            }
            self.focus = id;
            self.status = format!("Opened {}", path.display());
        } else if self.add_pane(PaneKind::Editor(editor), Axis::X) {
            self.status = format!("Opened {}", path.display());
        }
    }

    /// Opens the selected explorer entry (directories are entered).
    pub fn activate_explorer(&mut self) {
        let selected = self
            .focused_explorer()
            .and_then(|explorer| explorer.selected_entry())
            .map(|entry| (entry.path.clone(), entry.is_dir));
        match selected {
            Some((path, true)) => {
                if let Some(explorer) = self.focused_explorer_mut() {
                    explorer.navigate(path);
                }
            }
            Some((path, false)) => self.open_file(path),
            None => {}
        }
    }

    fn add_pane(&mut self, kind: PaneKind, axis: Axis) -> bool {
        let id = self.next_id;
        self.panes.push(Pane::new(id, kind));
        if layout::split_leaf(&mut self.root, self.focus, id, axis) {
            self.next_id += 1;
            self.focus = id;
            true
        } else {
            self.panes.pop();
            false
        }
    }
}

pub fn rect_contains(rect: Rect, column: u16, row: u16) -> bool {
    column >= rect.x
        && column < rect.x.saturating_add(rect.width)
        && row >= rect.y
        && row < rect.y.saturating_add(rect.height)
}

pub fn pane_inner(rect: Rect) -> Rect {
    Rect {
        x: rect.x.saturating_add(1),
        y: rect.y.saturating_add(1),
        width: rect.width.saturating_sub(2),
        height: rect.height.saturating_sub(2),
    }
}
