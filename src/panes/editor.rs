//! Text buffer with cursor, soft scrolling and basic editing commands.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use unicode_width::UnicodeWidthChar;

pub struct Editor {
    pub path: Option<PathBuf>,
    pub lines: Vec<String>,
    pub cx: usize,
    pub cy: usize,
    pub scroll_y: usize,
    pub scroll_x: usize,
    pub modified: bool,
    tab_width: usize,
    page_height: usize,
}

impl Editor {
    pub fn scratch() -> Self {
        Self {
            path: None,
            lines: vec![String::new()],
            cx: 0,
            cy: 0,
            scroll_y: 0,
            scroll_x: 0,
            modified: false,
            tab_width: 4,
            page_height: 10,
        }
    }

    pub fn open(path: &Path) -> io::Result<Self> {
        let bytes = fs::read(path)?;
        let text = String::from_utf8(bytes)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "file is not valid UTF-8"))?;
        let mut lines: Vec<String> = text
            .split('\n')
            .map(|line| line.strip_suffix('\r').unwrap_or(line).to_string())
            .collect();
        if lines.len() > 1 && lines.last().is_some_and(|line| line.is_empty()) {
            lines.pop();
        }
        Ok(Self {
            path: Some(path.to_path_buf()),
            lines,
            modified: false,
            ..Self::scratch()
        })
    }

    pub fn title(&self) -> String {
        self.path
            .as_ref()
            .and_then(|path| path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "[scratch]".into())
    }

    pub fn tab_width(&self) -> usize {
        self.tab_width
    }

    pub fn save(&mut self) -> io::Result<()> {
        let Some(path) = &self.path else {
            return Err(io::Error::other("scratch buffer has no file path"));
        };
        let mut text = self.lines.join("\n");
        if !text.is_empty() {
            text.push('\n');
        }
        fs::write(path, text)?;
        self.modified = false;
        Ok(())
    }

    pub fn insert_char(&mut self, ch: char) {
        let at = byte_index(&self.lines[self.cy], self.cx);
        self.lines[self.cy].insert(at, ch);
        self.cx += 1;
        self.modified = true;
    }

    /// Inserts pasted text, normalizing line endings.
    pub fn insert_str(&mut self, text: &str) {
        let text = text.replace("\r\n", "\n").replace('\r', "\n");
        let parts: Vec<&str> = text.split('\n').collect();
        let at = byte_index(&self.lines[self.cy], self.cx);
        if parts.len() == 1 {
            self.lines[self.cy].insert_str(at, parts[0]);
            self.cx += parts[0].chars().count();
        } else {
            let tail = self.lines[self.cy].split_off(at);
            self.lines[self.cy].push_str(parts[0]);
            let last = parts.len() - 1;
            for (offset, part) in parts.iter().enumerate().skip(1) {
                let content = if offset == last {
                    format!("{part}{tail}")
                } else {
                    (*part).to_string()
                };
                self.lines.insert(self.cy + offset, content);
            }
            self.cy += last;
            self.cx = parts[last].chars().count();
        }
        self.modified = true;
    }

    pub fn newline(&mut self) {
        let at = byte_index(&self.lines[self.cy], self.cx);
        let tail = self.lines[self.cy].split_off(at);
        self.lines.insert(self.cy + 1, tail);
        self.cy += 1;
        self.cx = 0;
        self.modified = true;
    }

    pub fn backspace(&mut self) {
        if self.cx > 0 {
            let at = byte_index(&self.lines[self.cy], self.cx - 1);
            self.lines[self.cy].remove(at);
            self.cx -= 1;
            self.modified = true;
        } else if self.cy > 0 {
            let line = self.lines.remove(self.cy);
            self.cy -= 1;
            self.cx = self.lines[self.cy].chars().count();
            self.lines[self.cy].push_str(&line);
            self.modified = true;
        }
    }

    pub fn delete(&mut self) {
        if self.cx < self.lines[self.cy].chars().count() {
            let at = byte_index(&self.lines[self.cy], self.cx);
            self.lines[self.cy].remove(at);
            self.modified = true;
        } else if self.cy + 1 < self.lines.len() {
            let next = self.lines.remove(self.cy + 1);
            self.lines[self.cy].push_str(&next);
            self.modified = true;
        }
    }

    pub fn move_left(&mut self) {
        if self.cx > 0 {
            self.cx -= 1;
        } else if self.cy > 0 {
            self.cy -= 1;
            self.cx = self.lines[self.cy].chars().count();
        }
    }

    pub fn move_right(&mut self) {
        if self.cx < self.lines[self.cy].chars().count() {
            self.cx += 1;
        } else if self.cy + 1 < self.lines.len() {
            self.cy += 1;
            self.cx = 0;
        }
    }

    pub fn move_up(&mut self, lines: usize) {
        self.cy = self.cy.saturating_sub(lines);
        self.cx = self.cx.min(self.lines[self.cy].chars().count());
    }

    pub fn move_down(&mut self, lines: usize) {
        self.cy = (self.cy + lines).min(self.lines.len() - 1);
        self.cx = self.cx.min(self.lines[self.cy].chars().count());
    }

    pub fn page_up(&mut self) {
        self.move_up(self.page_height.saturating_sub(1).max(1));
    }

    pub fn page_down(&mut self) {
        self.move_down(self.page_height.saturating_sub(1).max(1));
    }

    pub fn home(&mut self) {
        self.cx = 0;
    }

    pub fn end(&mut self) {
        self.cx = self.lines[self.cy].chars().count();
    }

    /// Places the cursor at a click inside the text viewport.
    pub fn click(&mut self, row: u16, column: u16) {
        let line = (self.scroll_y + row as usize).min(self.lines.len() - 1);
        self.cy = line;
        self.cx = char_at_display(
            &self.lines[line],
            self.scroll_x + column as usize,
            self.tab_width,
        );
    }

    pub fn cursor_display_col(&self) -> usize {
        display_col(&self.lines[self.cy], self.cx, self.tab_width)
    }

    /// Keeps the cursor inside the viewport; also remembers page size.
    pub fn scroll_to_cursor(&mut self, height: usize, width: usize) {
        self.page_height = height;
        if height == 0 || width == 0 {
            return;
        }
        self.cy = self.cy.min(self.lines.len() - 1);
        self.scroll_y = self.scroll_y.min(self.lines.len() - 1);
        if self.cy < self.scroll_y {
            self.scroll_y = self.cy;
        }
        if self.cy >= self.scroll_y + height {
            self.scroll_y = self.cy + 1 - height;
        }
        let column = self.cursor_display_col();
        if column < self.scroll_x {
            self.scroll_x = column;
        }
        if column >= self.scroll_x + width {
            self.scroll_x = column + 1 - width;
        }
    }
}

fn byte_index(line: &str, char_index: usize) -> usize {
    line.char_indices()
        .nth(char_index)
        .map(|(index, _)| index)
        .unwrap_or(line.len())
}

fn char_width(ch: char, column: usize, tab_width: usize) -> usize {
    if ch == '\t' {
        tab_width - (column % tab_width)
    } else if ch.is_control() {
        1
    } else {
        UnicodeWidthChar::width(ch).unwrap_or(0)
    }
}

/// Display column where character `cx` starts.
pub fn display_col(line: &str, cx: usize, tab_width: usize) -> usize {
    let mut column = 0;
    for (index, ch) in line.chars().enumerate() {
        if index == cx {
            return column;
        }
        column += char_width(ch, column, tab_width);
    }
    column
}

/// Horizontal slice of a line, expanding tabs and skipping `skip` columns.
pub fn display_slice(line: &str, skip: usize, width: usize, tab_width: usize) -> String {
    let mut out = String::new();
    let mut column = 0;
    let mut used = 0;
    for ch in line.chars() {
        let start = column;
        let char_width = char_width(ch, column, tab_width);
        column += char_width;
        if start < skip {
            continue;
        }
        if used + char_width > width {
            break;
        }
        if ch == '\t' || ch.is_control() {
            out.push_str(&" ".repeat(char_width));
        } else {
            out.push(ch);
        }
        used += char_width;
    }
    out
}

/// Character index closest to a display column.
pub fn char_at_display(line: &str, target: usize, tab_width: usize) -> usize {
    let mut column = 0;
    for (index, ch) in line.chars().enumerate() {
        let width = char_width(ch, column, tab_width);
        if target < column + width.max(1) {
            return index;
        }
        column += width;
    }
    line.chars().count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editing_keeps_lines_and_cursor_consistent() {
        let mut editor = Editor::scratch();
        editor.insert_str("hello\nworld");
        assert_eq!(editor.lines, ["hello", "world"]);
        editor.newline();
        editor.insert_char('!');
        assert_eq!(editor.lines, ["hello", "world", "!"]);
        editor.backspace();
        assert_eq!(editor.lines, ["hello", "world", ""]);
        editor.backspace();
        assert_eq!(editor.lines, ["hello", "world"]);
        assert_eq!(editor.cy, 1);
        editor.home();
        editor.backspace();
        assert_eq!(editor.lines, ["helloworld"]);
        editor.delete();
        assert_eq!(editor.lines, ["helloorld"]);
    }

    #[test]
    fn tabs_and_wide_chars_map_to_columns() {
        let line = "\t中x";
        assert_eq!(display_col(line, 0, 4), 0);
        assert_eq!(display_col(line, 1, 4), 4);
        assert_eq!(display_col(line, 2, 4), 6);
        assert_eq!(display_slice(line, 4, 10, 4), "中x");
        assert_eq!(char_at_display(line, 5, 4), 1);
    }
}
