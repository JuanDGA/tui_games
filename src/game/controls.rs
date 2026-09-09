//! Action-to-key maps declared by each game.
//!
//! Single-player games bind actions to player A. Multiplayer games bind the
//! same actions for player A and player B. The host routes key events through
//! this map instead of letting each game hard-code key codes.

use crossterm::event::{Event, KeyCode, KeyEventKind};

use super::{Controller, Game, MatchConfig, PlayerId};

/// A game-defined input. Names are stable within a game (`"up"`, `"rotate"`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Action {
    pub name: &'static str,
}

impl Action {
    pub const UP: Self = Self { name: "up" };
    pub const DOWN: Self = Self { name: "down" };
    pub const LEFT: Self = Self { name: "left" };
    pub const RIGHT: Self = Self { name: "right" };
    pub const ROTATE: Self = Self { name: "rotate" };
    pub const ROTATE_CCW: Self = Self { name: "rotate_ccw" };
    pub const SOFT_DROP: Self = Self { name: "soft_drop" };
    pub const HARD_DROP: Self = Self { name: "hard_drop" };
    pub const END: Self = Self { name: "end" };
}

/// Press/repeat vs key release. Hold-to-move games use both.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionPhase {
    Start,
    Stop,
}

impl ActionPhase {
    pub fn from_key_kind(kind: KeyEventKind) -> Self {
        match kind {
            KeyEventKind::Release => Self::Stop,
            KeyEventKind::Press | KeyEventKind::Repeat => Self::Start,
        }
    }

    pub fn is_start(self) -> bool {
        matches!(self, Self::Start)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyBinding {
    pub player: PlayerId,
    pub action: Action,
    pub key: KeyCode,
}

/// Canonical key map for a game. Multiplayer maps always include A and B.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyMap {
    bindings: Vec<KeyBinding>,
}

impl KeyMap {
    pub fn new() -> Self {
        Self {
            bindings: Vec::new(),
        }
    }

    pub fn bind(
        mut self,
        player: PlayerId,
        action: Action,
        keys: impl IntoIterator<Item = KeyCode>,
    ) -> Self {
        for key in keys {
            self.bindings.push(KeyBinding {
                player,
                action,
                key,
            });
        }
        self
    }

    pub fn player_a(self, action: Action, keys: impl IntoIterator<Item = KeyCode>) -> Self {
        self.bind(PlayerId::A, action, keys)
    }

    pub fn player_b(self, action: Action, keys: impl IntoIterator<Item = KeyCode>) -> Self {
        self.bind(PlayerId::B, action, keys)
    }

    pub fn bindings(&self) -> &[KeyBinding] {
        &self.bindings
    }

    /// Exact binding: which player owns this key in the canonical A/B map.
    pub fn lookup(&self, key: KeyCode) -> Option<(PlayerId, Action)> {
        self.bindings
            .iter()
            .find(|binding| binding.key == key)
            .map(|binding| (binding.player, binding.action))
    }

    /// Binding after applying the match lineup.
    ///
    /// CPU-owned keys are ignored, except when there is a single human: that
    /// player also receives the unused CPU keys so vs-CPU keeps both schemes.
    pub fn resolve(
        &self,
        key: KeyCode,
        match_config: Option<&MatchConfig>,
    ) -> Option<(PlayerId, Action)> {
        let (player, action) = self.lookup(key)?;
        let Some(config) = match_config else {
            return Some((player, action));
        };
        match config.controller(player) {
            Some(Controller::Human) => Some((player, action)),
            Some(Controller::Cpu) => {
                let humans: Vec<PlayerId> = config
                    .seats()
                    .iter()
                    .filter(|seat| seat.controller == Controller::Human)
                    .map(|seat| seat.id)
                    .collect();
                match humans.as_slice() {
                    [only] => Some((*only, action)),
                    _ => None,
                }
            }
            None => None,
        }
    }

    /// Compact key list for one player, e.g. `"W/S"` or `"↑/↓"`.
    pub fn summarize(&self, player: PlayerId) -> String {
        let mut labels = Vec::new();
        for binding in &self.bindings {
            if binding.player != player {
                continue;
            }
            if let Some(label) = key_label(binding.key) {
                if !labels.iter().any(|seen| seen == &label) {
                    labels.push(label);
                }
            }
        }
        labels.join("/")
    }

    /// Per-action key lists for the ready overlay, e.g. `("↑/X", "rotate")`.
    pub fn describe(&self, player: PlayerId) -> Vec<(String, String)> {
        let mut actions: Vec<Action> = Vec::new();
        let mut keys: Vec<Vec<String>> = Vec::new();
        for binding in &self.bindings {
            if binding.player != player || binding.action == Action::END {
                continue;
            }
            let Some(label) = key_label(binding.key) else {
                continue;
            };
            if let Some(i) = actions.iter().position(|action| *action == binding.action) {
                if !keys[i].iter().any(|seen| seen == &label) {
                    keys[i].push(label);
                }
            } else {
                actions.push(binding.action);
                keys.push(vec![label]);
            }
        }
        actions
            .into_iter()
            .zip(keys)
            .map(|(action, labels)| (labels.join("/"), action.name.replace('_', " ")))
            .collect()
    }
}

impl Default for KeyMap {
    fn default() -> Self {
        Self::new()
    }
}

/// Lower and upper letter keys so Shift does not drop the binding.
pub fn letter(c: char) -> [KeyCode; 2] {
    [
        KeyCode::Char(c.to_ascii_lowercase()),
        KeyCode::Char(c.to_ascii_uppercase()),
    ]
}

pub fn key_label(key: KeyCode) -> Option<String> {
    match key {
        KeyCode::Up => Some("↑".into()),
        KeyCode::Down => Some("↓".into()),
        KeyCode::Left => Some("←".into()),
        KeyCode::Right => Some("→".into()),
        KeyCode::Char(' ') => Some("Space".into()),
        KeyCode::Char(c) if c.is_ascii_lowercase() || !c.is_ascii() => {
            Some(c.to_ascii_uppercase().to_string())
        }
        KeyCode::Char(_) => None,
        other => Some(format!("{other:?}")),
    }
}

/// Route a key through the game's map, remapping unused CPU keys to a lone human.
pub fn dispatch(game: &mut dyn Game, event: Event) {
    let Event::Key(key) = event else {
        return;
    };
    let config = game.as_multiplayer().map(|mp| mp.match_config().clone());
    let phase = ActionPhase::from_key_kind(key.kind);
    if let Some((player, action)) = game.controls().resolve(key.code, config.as_ref()) {
        game.handle_action(player, action, phase);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pong_map() -> KeyMap {
        KeyMap::new()
            .player_a(Action::UP, letter('w'))
            .player_a(Action::DOWN, letter('s'))
            .player_b(Action::UP, [KeyCode::Up])
            .player_b(Action::DOWN, [KeyCode::Down])
    }

    #[test]
    fn two_player_map_splits_wasd_and_arrows() {
        let map = pong_map();
        assert_eq!(
            map.lookup(KeyCode::Char('w')),
            Some((PlayerId::A, Action::UP))
        );
        assert_eq!(
            map.lookup(KeyCode::Char('S')),
            Some((PlayerId::A, Action::DOWN))
        );
        assert_eq!(map.lookup(KeyCode::Up), Some((PlayerId::B, Action::UP)));
        assert_eq!(map.lookup(KeyCode::Left), None);
        assert!(
            map.bindings()
                .iter()
                .any(|binding| binding.player == PlayerId::B && binding.action == Action::UP)
        );
    }

    #[test]
    fn vs_cpu_gives_unused_cpu_keys_to_the_human() {
        let map = pong_map();
        let config = MatchConfig::vs_cpu(super::super::MultiplayerSpec::TWO_PLAYER);
        assert_eq!(
            map.resolve(KeyCode::Up, Some(&config)),
            Some((PlayerId::A, Action::UP))
        );
        assert_eq!(
            map.resolve(KeyCode::Char('w'), Some(&config)),
            Some((PlayerId::A, Action::UP))
        );
    }

    #[test]
    fn two_humans_do_not_share_keys() {
        let map = pong_map();
        let config = MatchConfig::vs_player(super::super::MultiplayerSpec::TWO_PLAYER);
        assert_eq!(
            map.resolve(KeyCode::Char('w'), Some(&config)),
            Some((PlayerId::A, Action::UP))
        );
        assert_eq!(
            map.resolve(KeyCode::Up, Some(&config)),
            Some((PlayerId::B, Action::UP))
        );
    }

    #[test]
    fn summarize_skips_uppercase_duplicates() {
        let map = pong_map();
        assert_eq!(map.summarize(PlayerId::A), "W/S");
        assert_eq!(map.summarize(PlayerId::B), "↑/↓");
    }

    #[test]
    fn describe_groups_keys_by_action() {
        let map = KeyMap::new()
            .player_a(Action::LEFT, [KeyCode::Left])
            .player_a(Action::ROTATE, [KeyCode::Up])
            .player_a(Action::ROTATE, letter('x'))
            .player_a(Action::END, letter('x'));
        assert_eq!(
            map.describe(PlayerId::A),
            vec![("←".into(), "left".into()), ("↑/X".into(), "rotate".into()),]
        );
    }
}
