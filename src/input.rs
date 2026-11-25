//! Keyboard and mouse mapping.

use ratatui::crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

use crate::app::{App, Drag, pane_inner, rect_contains};
use crate::layout::Axis;
use crate::panes::PaneKind;

pub fn handle(app: &mut App, event: Event) {
    let is_quit = matches!(
        &event,
        Event::Key(key) if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('q' | 'Q'))
    );
    if app.quit_armed && !is_quit {
        app.quit_armed = false;
        app.status.clear();
    }

    match event {
        Event::Key(key) if key.kind != KeyEventKind::Release => handle_key(app, key),
        Event::Mouse(mouse) => handle_mouse(app, mouse),
        Event::Paste(text) => match app.focused_editor_mut() {
            Some(editor) => editor.insert_str(&text),
            None => app.status = "Paste ignored: focus is not an editor".into(),
        },
        _ => {}
    }
}

fn handle_key(app: &mut App, key: KeyEvent) {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('q' | 'Q') => app.quit(),
            KeyCode::Char('s' | 'S') => app.save_focused(),
            KeyCode::Char('n' | 'N') => app.split_focused(Axis::X),
            KeyCode::Char('t' | 'T') => app.split_focused(Axis::Y),
            KeyCode::Char('w' | 'W') => app.close_focused(),
            // Move the nearest divider of the matching orientation.
            KeyCode::Left => app.resize_focused(Axis::X, -2),
            KeyCode::Right => app.resize_focused(Axis::X, 2),
            KeyCode::Up => app.resize_focused(Axis::Y, -2),
            KeyCode::Down => app.resize_focused(Axis::Y, 2),
            _ => {}
        }
        return;
    }

    match key.code {
        KeyCode::Tab => {
            let step = if key.modifiers.contains(KeyModifiers::SHIFT) {
                -1
            } else {
                1
            };
            app.focus_next(step);
        }
        KeyCode::BackTab => app.focus_next(-1),
        KeyCode::Esc => {}
        _ => handle_pane_key(app, key),
    }
}

fn handle_pane_key(app: &mut App, key: KeyEvent) {
    let editor_focused = match app.focused_pane().map(|pane| &pane.kind) {
        Some(PaneKind::Editor(_)) => Some(true),
        Some(PaneKind::Explorer(_)) => Some(false),
        None => None,
    };

    match editor_focused {
        Some(true) => {
            let Some(editor) = app.focused_editor_mut() else {
                return;
            };
            match key.code {
                KeyCode::Char(ch) => {
                    if !key.modifiers.contains(KeyModifiers::ALT) {
                        editor.insert_char(ch);
                    }
                }
                KeyCode::Enter => editor.newline(),
                KeyCode::Tab => {
                    for _ in 0..editor.tab_width() {
                        editor.insert_char(' ');
                    }
                }
                KeyCode::Backspace => editor.backspace(),
                KeyCode::Delete => editor.delete(),
                KeyCode::Left => editor.move_left(),
                KeyCode::Right => editor.move_right(),
                KeyCode::Up => editor.move_up(1),
                KeyCode::Down => editor.move_down(1),
                KeyCode::Home => editor.home(),
                KeyCode::End => editor.end(),
                KeyCode::PageUp => editor.page_up(),
                KeyCode::PageDown => editor.page_down(),
                _ => {}
            }
        }
        Some(false) => handle_explorer_key(app, key),
        None => {}
    }
}

fn handle_explorer_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Up => {
            if let Some(explorer) = app.focused_explorer_mut() {
                explorer.select(-1);
            }
        }
        KeyCode::Down => {
            if let Some(explorer) = app.focused_explorer_mut() {
                explorer.select(1);
            }
        }
        KeyCode::PageUp => {
            if let Some(explorer) = app.focused_explorer_mut() {
                explorer.select(-10);
            }
        }
        KeyCode::PageDown => {
            if let Some(explorer) = app.focused_explorer_mut() {
                explorer.select(10);
            }
        }
        KeyCode::Home => {
            if let Some(explorer) = app.focused_explorer_mut() {
                explorer.select_first();
            }
        }
        KeyCode::End => {
            if let Some(explorer) = app.focused_explorer_mut() {
                explorer.select_last();
            }
        }
        KeyCode::Enter => app.activate_explorer(),
        KeyCode::Left | KeyCode::Backspace => {
            if let Some(explorer) = app.focused_explorer_mut() {
                explorer.go_up();
            }
        }
        _ => {}
    }
}

fn handle_mouse(app: &mut App, mouse: MouseEvent) {
    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            app.drag = None;
            if let Some(divider) = app.divider_at(mouse.column, mouse.row) {
                app.drag = Some(Drag {
                    path: divider.path,
                    parent: divider.parent,
                    axis: divider.axis,
                });
                return;
            }

            let Some(id) = app.pane_at(mouse.column, mouse.row) else {
                return;
            };
            app.focus = id;
            let mut open_entry = false;
            if let Some(pane) = app.pane_mut(id) {
                let inner = pane_inner(pane.rect);
                if rect_contains(inner, mouse.column, mouse.row) {
                    match &mut pane.kind {
                        PaneKind::Editor(editor) => {
                            editor.click(mouse.row - inner.y, mouse.column - inner.x);
                        }
                        PaneKind::Explorer(explorer) => {
                            let index = explorer.scroll + (mouse.row - inner.y) as usize;
                            if index < explorer.entries.len() {
                                explorer.selected = index;
                                open_entry = true;
                            }
                        }
                    }
                }
            }
            if open_entry {
                app.activate_explorer();
            }
        }
        MouseEventKind::Drag(MouseButton::Left) => app.drag_ratio(mouse.column, mouse.row),
        MouseEventKind::Up(MouseButton::Left) => app.drag = None,
        MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
            let Some(id) = app.pane_at(mouse.column, mouse.row) else {
                return;
            };
            let up = matches!(mouse.kind, MouseEventKind::ScrollUp);
            if let Some(pane) = app.pane_mut(id) {
                match &mut pane.kind {
                    PaneKind::Editor(editor) => {
                        if up {
                            editor.move_up(3);
                        } else {
                            editor.move_down(3);
                        }
                    }
                    PaneKind::Explorer(explorer) => explorer.select(if up { -1 } else { 1 }),
                }
            }
        }
        _ => {}
    }
}
