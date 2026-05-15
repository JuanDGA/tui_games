use std::io;
use std::panic;

mod app;
mod game;
mod grid;
mod menu;
mod registry;

use app::App;

fn main() -> io::Result<()> {
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = crossterm::execute!(
            std::io::stderr(),
            crossterm::terminal::LeaveAlternateScreen,
            crossterm::cursor::Show
        );
        original_hook(info);
    }));

    ratatui::run(|terminal| App::new().run(terminal))
}
