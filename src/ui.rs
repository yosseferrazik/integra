//! Rendering: panes, dividers, status bar and quit prompt.

use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::app::App;
use crate::layout::{self, Axis, Divider};
use crate::panes::{PaneKind, editor, editor::Editor, explorer::Explorer};

const HINTS: &str = "Ctrl-S save  Ctrl-Q quit  Ctrl-N split  Ctrl-W close  Tab focus";

pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    if area.width == 0 || area.height == 0 {
        return;
    }

    let body = Rect {
        height: area.height.saturating_sub(1),
        ..area
    };
    let status_area = Rect {
        x: area.x,
        y: area.y + area.height - 1,
        width: area.width,
        height: 1,
    };

    let (rects, dividers) = layout::compute(&app.root, body);
    app.dividers = dividers;

    for (id, rect) in rects.iter().copied() {
        let focused = app.focus == id;
        if let Some(pane) = app.pane_mut(id) {
            pane.rect = rect;
            if rect.width == 0 || rect.height == 0 {
                continue;
            }
            match &mut pane.kind {
                PaneKind::Editor(editor) => draw_editor(frame, editor, rect, focused),
                PaneKind::Explorer(explorer) => draw_explorer(frame, explorer, rect, focused),
            }
        }
    }

    draw_dividers(frame, &app.dividers);
    draw_status(frame, app, status_area);

    if app.quit_armed {
        draw_quit_prompt(frame, area);
    }
}

fn draw_editor(frame: &mut Frame, editor: &mut Editor, area: Rect, focused: bool) {
    let marker = if editor.modified { " *" } else { "" };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {}{marker} ", editor.title()))
        .border_style(border_style(focused));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    editor.scroll_to_cursor(inner.height as usize, inner.width as usize);
    for row in 0..inner.height {
        let line = editor.scroll_y + row as usize;
        if line >= editor.lines.len() {
            break;
        }
        let text = editor::display_slice(
            &editor.lines[line],
            editor.scroll_x,
            inner.width as usize,
            editor.tab_width(),
        );
        frame
            .buffer_mut()
            .set_string(inner.x, inner.y + row, &text, Style::default());
    }

    if focused {
        let column = editor.cursor_display_col();
        if column >= editor.scroll_x {
            let x = inner.x + (column - editor.scroll_x) as u16;
            let y = inner.y + (editor.cy - editor.scroll_y) as u16;
            if x < inner.right() {
                frame.set_cursor_position((x, y));
            }
        }
    }
}

fn draw_explorer(frame: &mut Frame, explorer: &mut Explorer, area: Rect, focused: bool) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {} ", explorer.dir.display()))
        .border_style(border_style(focused));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let height = inner.height as usize;
    if explorer.selected < explorer.scroll {
        explorer.scroll = explorer.selected;
    }
    if explorer.selected >= explorer.scroll + height {
        explorer.scroll = explorer.selected + 1 - height;
    }

    let lines = explorer
        .entries
        .iter()
        .enumerate()
        .skip(explorer.scroll)
        .take(height)
        .map(|(index, entry)| {
            let suffix = if entry.is_dir { "/" } else { "" };
            let style = if index == explorer.selected {
                Style::default().add_modifier(Modifier::REVERSED)
            } else if entry.is_dir {
                Style::default().fg(Color::Blue)
            } else {
                Style::default()
            };
            Line::from(Span::styled(format!(" {}{suffix}", entry.name), style))
        })
        .collect::<Vec<_>>();
    frame.render_widget(Paragraph::new(lines), inner);
}

fn draw_dividers(frame: &mut Frame, dividers: &[Divider]) {
    let style = Style::default().fg(Color::DarkGray);
    for divider in dividers {
        match divider.axis {
            Axis::X => {
                for row in divider.rect.y..divider.rect.bottom() {
                    frame
                        .buffer_mut()
                        .set_string(divider.rect.x, row, "│", style);
                }
            }
            Axis::Y => {
                for column in divider.rect.x..divider.rect.right() {
                    frame
                        .buffer_mut()
                        .set_string(column, divider.rect.y, "─", style);
                }
            }
        }
    }
}

fn draw_status(frame: &mut Frame, app: &App, area: Rect) {
    let left = if app.status.is_empty() {
        HINTS.to_string()
    } else {
        app.status.clone()
    };
    let right = match app.focused_pane().map(|pane| &pane.kind) {
        Some(PaneKind::Editor(editor)) => format!("Ln {}, Col {} ", editor.cy + 1, editor.cx + 1),
        Some(PaneKind::Explorer(explorer)) => format!("{} items ", explorer.entries.len()),
        None => String::new(),
    };

    let width = area.width as usize;
    let right_width = right.chars().count();
    // Reserve the right side for cursor info; trim the message instead.
    let max_left = if right_width > 0 && right_width + 2 <= width {
        width - right_width - 1
    } else {
        width
    };
    let mut text = truncate(&left, max_left);
    if right_width > 0 && right_width + 1 < width {
        let used = text.chars().count();
        text.push_str(&" ".repeat(width - used - right_width));
        text.push_str(&right);
    }

    let style = Style::default().bg(Color::DarkGray).fg(Color::White);
    frame.buffer_mut().set_style(area, style);
    frame.render_widget(Paragraph::new(text).style(style), area);
}

fn draw_quit_prompt(frame: &mut Frame, area: Rect) {
    let width = area.width.min(64);
    let height = area.height.min(3);
    if width < 8 || height < 3 {
        return;
    }
    let rect = Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    );
    frame.render_widget(Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .title(" quit ");
    let text = "Unsaved changes: Ctrl-Q again to quit, Esc to cancel";
    frame.render_widget(
        Paragraph::new(text)
            .block(block)
            .alignment(Alignment::Center),
        rect,
    );
}

fn border_style(focused: bool) -> Style {
    if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    }
}

fn truncate(text: &str, width: usize) -> String {
    text.chars().take(width).collect()
}
