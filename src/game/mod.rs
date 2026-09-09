use std::time::Duration;

use crossterm::event::Event;
use ratatui::Frame;

pub mod arkanoid;
pub mod controls;
pub mod multiplayer;
pub mod pong;
pub mod snake;
pub mod tetris;

pub use controls::{Action, ActionPhase, KeyMap};
pub use multiplayer::{
    Controller, MatchConfig, MatchError, Multiplayer, MultiplayerSpec, PlayerId, Seat,
};

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

    /// Action-to-key map. Multiplayer games must bind player A and player B.
    fn controls(&self) -> KeyMap;

    /// Apply a mapped action. CPU seats should ignore this.
    fn handle_action(&mut self, player: PlayerId, action: Action, phase: ActionPhase);

    fn is_multiplayer(&self) -> bool {
        false
    }

    /// Multiplayer games return their CPU-capable match API. Single-player
    /// games leave this as `None`.
    fn as_multiplayer(&mut self) -> Option<&mut dyn Multiplayer> {
        None
    }

    /// Apply the default vs-CPU lineup when this game is multiplayer.
    fn setup_default_match(&mut self) {
        let Some(mp) = self.as_multiplayer() else {
            return;
        };
        let config = MatchConfig::vs_cpu(mp.spec());
        debug_assert!(
            config.has_cpu() && config.has_human(),
            "default multiplayer lineup is one human plus CPU"
        );
        if mp.apply_match(&config).is_err() {
            debug_assert!(false, "multiplayer games must accept MatchConfig::vs_cpu");
        }
    }
}
