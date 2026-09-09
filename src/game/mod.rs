use std::time::Duration;

use crossterm::event::Event;
use ratatui::Frame;

pub mod pong;
pub mod snake;
pub mod tetris;

pub trait Game {
    fn name(&self) -> &'static str;
    fn poll_timeout(&self) -> Duration;
    fn min_size(&self) -> (u16, u16);

    fn reset(&mut self);
    fn handle_event(&mut self, event: Event);
    fn update(&mut self, delta: Duration);
    fn render(&mut self, frame: &mut Frame);

    fn is_game_over(&self) -> bool;
    fn score(&self) -> u32;
}
