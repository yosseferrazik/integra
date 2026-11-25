# integra

A small, fast TUI code editor built around **mosaic buffers**: file explorers and
text editors are panes in a resizable grid, driven mostly from the keyboard.

Written in Rust with [ratatui](https://github.com/ratatui/ratatui) and crossterm.

## Install

Needs Rust 1.85+ (edition 2024) and a working C linker.

```sh
cargo build --release
./target/release/integra            # scratch buffer + explorer
./target/release/integra src/main.rs
./target/release/integra src/       # start explorer in a directory
```

Or `cargo install --path .` to put `integra` on your `PATH`.

## Keys

| Key | Action |
| --- | --- |
| `Ctrl-S` | Save the focused file |
| `Ctrl-Q` | Quit (press twice if there are unsaved changes, `Esc` cancels) |
| `Ctrl-N` | Split focused pane and open a new editor to the right |
| `Ctrl-T` | Split focused pane and open a new editor below |
| `Ctrl-W` | Close the focused pane |
| `Ctrl-←/→` | Resize the nearest vertical split |
| `Ctrl-↑/↓` | Resize the nearest horizontal split |
| `Tab` / `Shift-Tab` | Move focus between panes |

Inside an editor: arrows, `Home`, `End`, `PgUp`, `PgDn`, `Enter`,
`Backspace`, `Delete`; Paste is supported.

Inside the explorer: arrows or `PgUp`/`PgDn`, `Home`/`End`, `Enter` opens a
file or enters a directory, `←`/`Backspace` goes to the parent directory.

Mouse: click a pane to focus it, click text to place the cursor, click an
explorer entry to open it, drag a divider to resize, and use the wheel to scroll.

## Layout

The default mosaic is a 30/70 explorer/editor split. Splitting or closing panes
reshapes a binary tree; divider positions are ratios, so layouts survive terminal
resizes.

## Code map

```
src/
  main.rs      terminal setup/restore, CLI, event loop
  app.rs       app state, pane actions, focus, quit/save
  layout.rs    split tree, pane rectangles, divider drag math
  input.rs     key and mouse mapping
  ui.rs        rendering, status bar, quit prompt
  panes/
    mod.rs     Pane + PaneKind (editor/explorer; terminal/log are future kinds)
    editor.rs  text buffer, cursor, scrolling, display-width helpers
    explorer.rs directory listing and navigation
```

## Limits (current)

- No undo/redo, search, selection or syntax highlighting yet.
- UTF-8 files only; invalid UTF-8 is rejected instead of rewritten.
- Files are saved with a trailing LF newline (CRLF is normalized).
- New scratch buffers must be saved through an existing file: open a file
  from the explorer before pressing `Ctrl-S`.
