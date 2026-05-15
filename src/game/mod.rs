use std::time::Duration;

use crossterm::event::Event;
use ratatui::Frame;

pub mod demo;

#[allow(dead_code)]
pub trait Game {
    fn name(&self) -> &'static str;
    fn instructions(&self) -> &'static str;
    fn poll_timeout(&self) -> Duration;

    fn reset(&mut self);
    fn handle_event(&mut self, event: Event);
    fn update(&mut self, delta: Duration);
    fn render(&mut self, frame: &mut Frame);

    fn is_game_over(&self) -> bool;
    fn score(&self) -> u32;
}
