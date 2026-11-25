//! Directory listing pane.

use std::fs;
use std::path::PathBuf;

pub struct Entry {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
}

pub struct Explorer {
    pub dir: PathBuf,
    pub entries: Vec<Entry>,
    pub selected: usize,
    pub scroll: usize,
}

impl Explorer {
    pub fn new(dir: PathBuf) -> Self {
        let mut explorer = Self {
            dir: absolute_dir(dir),
            entries: Vec::new(),
            selected: 0,
            scroll: 0,
        };
        explorer.reload();
        explorer
    }

    pub fn reload(&mut self) {
        let mut entries = Vec::new();
        // Parent entry first, then directories, then files.
        if let Some(parent) = self
            .dir
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            entries.push(Entry {
                path: parent.to_path_buf(),
                name: "..".into(),
                is_dir: true,
            });
        }
        let mut children: Vec<Entry> = fs::read_dir(&self.dir)
            .into_iter()
            .flatten()
            .filter_map(|item| {
                let item = item.ok()?;
                let is_dir = item.file_type().map(|kind| kind.is_dir()).unwrap_or(false);
                Some(Entry {
                    path: item.path(),
                    name: item.file_name().to_string_lossy().into_owned(),
                    is_dir,
                })
            })
            .collect();
        children.sort_by(|a, b| {
            b.is_dir
                .cmp(&a.is_dir)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        entries.extend(children);
        self.entries = entries;
        self.selected = 0;
        self.scroll = 0;
    }

    pub fn navigate(&mut self, dir: PathBuf) {
        let dir = absolute_dir(dir);
        if dir.as_os_str().is_empty() {
            return;
        }
        self.dir = dir;
        self.reload();
    }

    pub fn go_up(&mut self) {
        if let Some(parent) = self
            .dir
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            self.navigate(parent.to_path_buf());
        }
    }

    pub fn select(&mut self, delta: isize) {
        if self.entries.is_empty() {
            return;
        }
        let last = self.entries.len() as isize - 1;
        self.selected = (self.selected as isize + delta).clamp(0, last) as usize;
    }

    pub fn select_first(&mut self) {
        self.selected = 0;
    }

    pub fn select_last(&mut self) {
        self.selected = self.entries.len().saturating_sub(1);
    }

    pub fn selected_entry(&self) -> Option<&Entry> {
        self.entries.get(self.selected)
    }
}

/// Absolute paths keep `parent()` meaningful when navigating upwards.
fn absolute_dir(dir: PathBuf) -> PathBuf {
    let dir = if dir.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        dir
    };
    dir.canonicalize().unwrap_or(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_paths_stay_navigable() {
        let mut explorer = Explorer::new(PathBuf::from("."));
        assert!(explorer.dir.is_absolute());
        assert!(!explorer.entries.is_empty());

        let start = explorer.dir.clone();
        explorer.go_up();
        if explorer.dir != start {
            assert!(explorer.dir.is_absolute());
            assert!(!explorer.entries.is_empty());
            assert!(fs::read_dir(&explorer.dir).is_ok());
        }
    }
}
