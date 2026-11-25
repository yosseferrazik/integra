//! integra: a small mosaic TUI editor.

mod app;
mod input;
mod layout;
mod panes;
mod ui;

// s

use std::io::{self, Stdout, stdout};
use std::path::PathBuf;
use std::time::Duration;

use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::event::{
    self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{
    Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};

use crate::app::App;

type Tui = Terminal<CrosstermBackend<Stdout>>;

fn main() -> io::Result<()> {
    let (file, dir) = parse_args();
    install_panic_hook();

    let mut terminal = setup()?;
    let mut app = App::new(file, dir);
    let result = run(&mut terminal, &mut app);
    let restored = restore(&mut terminal);
    result.and(restored)
}

fn parse_args() -> (Option<PathBuf>, Option<PathBuf>) {
    let mut file = None;
    let mut dir = None;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "-h" | "--help" => {
                println!("{}", usage());
                std::process::exit(0);
            }
            "-V" | "--version" => {
                println!("integra {}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            _ if arg.starts_with('-') => {
                eprintln!("unknown option: {arg}\n\n{}", usage());
                std::process::exit(2);
            }
            _ => {
                let path = PathBuf::from(&arg);
                if path.is_dir() {
                    dir = Some(path);
                    file = None;
                } else {
                    file = Some(path);
                }
            }
        }
    }
    (file, dir)
}

fn usage() -> String {
    format!(
        "integra {}\n\nUsage: integra [FILE|DIR]\n\n\
         Ctrl-S save  Ctrl-Q quit  Ctrl-N split  Ctrl-T split down\n\
         Ctrl-W close  Ctrl-arrows resize  Tab focus\n\
         Mouse: click to focus/edit, drag dividers, wheel scroll",
        env!("CARGO_PKG_VERSION")
    )
}

fn install_panic_hook() {
    let original = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(
            stdout(),
            LeaveAlternateScreen,
            DisableMouseCapture,
            DisableBracketedPaste
        );
        original(info);
    }));
}

fn setup() -> io::Result<Tui> {
    enable_raw_mode()?;
    let mut out = stdout();
    execute!(
        out,
        EnterAlternateScreen,
        Clear(ClearType::All),
        EnableMouseCapture,
        EnableBracketedPaste
    )?;
    // Avoid Terminal::clear(): it queries the cursor, which can hang on dumb ptys.
    let terminal = Terminal::new(CrosstermBackend::new(out))?;
    Ok(terminal)
}

fn restore(terminal: &mut Tui) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture,
        DisableBracketedPaste
    )?;
    terminal.show_cursor()
}

fn run(terminal: &mut Tui, app: &mut App) -> io::Result<()> {
    while app.running {
        terminal.draw(|frame| ui::draw(frame, app))?;
        if event::poll(Duration::from_millis(200))? {
            input::handle(app, event::read()?);
        }
    }
    Ok(())
}
