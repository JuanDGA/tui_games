use std::io;
use std::panic;

mod app;
mod game;
mod grid;
mod kitty;
mod menu;
mod registry;

use app::App;

fn main() -> io::Result<()> {
    let mut terminal = ratatui::init();
    kitty::enable()?;

    // Pop kitty flags before ratatui restores the screen. Alternate-screen
    // keyboard mode is independent of the main screen; popping after leave
    // would mutate the wrong stack.
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let _ = kitty::disable();
        original_hook(info);
    }));

    let result = App::new().run(&mut terminal);
    let _ = kitty::disable();
    ratatui::restore();
    result
}
