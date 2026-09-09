//! Shared contract for games with more than one player.
//!
//! A multiplayer game declares a player range, accepts a [`MatchConfig`], and
//! must drive every seat filled by [`Controller::Cpu`]. The host launches with
//! [`MatchConfig::vs_cpu`]: one local human and CPU in every remaining seat.

use std::time::Duration;

/// Seat index in a match. Games interpret `0..n` in their own order.
/// Player A is `0`, player B is `1`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PlayerId(pub u8);

impl PlayerId {
    pub const A: Self = Self(0);
    pub const B: Self = Self(1);
}

/// Who fills a seat.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Controller {
    Human,
    Cpu,
}

/// One assigned player slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Seat {
    pub id: PlayerId,
    pub controller: Controller,
}

impl Seat {
    pub const fn human(id: u8) -> Self {
        Self {
            id: PlayerId(id),
            controller: Controller::Human,
        }
    }

    pub const fn cpu(id: u8) -> Self {
        Self {
            id: PlayerId(id),
            controller: Controller::Cpu,
        }
    }
}

/// How many players a game can run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MultiplayerSpec {
    min_players: u8,
    max_players: u8,
}

impl MultiplayerSpec {
    pub const TWO_PLAYER: Self = match Self::try_new(2, 2) {
        Some(spec) => spec,
        None => unreachable!(),
    };

    /// `None` unless `min_players >= 2` and `max_players >= min_players`.
    pub const fn try_new(min_players: u8, max_players: u8) -> Option<Self> {
        if min_players < 2 || max_players < min_players {
            return None;
        }
        Some(Self {
            min_players,
            max_players,
        })
    }

    pub const fn min_players(self) -> u8 {
        self.min_players
    }

    pub const fn max_players(self) -> u8 {
        self.max_players
    }
}

/// Validated lineup for one match.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchConfig {
    seats: Vec<Seat>,
}

impl MatchConfig {
    pub fn new(spec: MultiplayerSpec, seats: Vec<Seat>) -> Result<Self, MatchError> {
        let got = u8::try_from(seats.len()).unwrap_or(u8::MAX);
        if got < spec.min_players() || got > spec.max_players() {
            return Err(MatchError::PlayerCount {
                got,
                min: spec.min_players(),
                max: spec.max_players(),
            });
        }

        let mut seen = [false; 256];
        for seat in &seats {
            let i = seat.id.0 as usize;
            if seen[i] {
                return Err(MatchError::DuplicatePlayer(seat.id));
            }
            seen[i] = true;
        }

        Ok(Self { seats })
    }

    /// One human at seat 0, CPU in every other required seat.
    ///
    /// This is the default launch lineup. It always includes at least one CPU
    /// because [`MultiplayerSpec`] requires two or more players.
    pub fn vs_cpu(spec: MultiplayerSpec) -> Self {
        let seats = (0..spec.min_players)
            .map(|i| if i == 0 { Seat::human(i) } else { Seat::cpu(i) })
            .collect();
        Self::new(spec, seats).expect("vs_cpu is valid for any MultiplayerSpec")
    }

    /// Two local humans at seats 0 and 1, CPU in every remaining required seat.
    pub fn vs_player(spec: MultiplayerSpec) -> Self {
        let seats = (0..spec.min_players)
            .map(|i| if i < 2 { Seat::human(i) } else { Seat::cpu(i) })
            .collect();
        Self::new(spec, seats).expect("vs_player is valid for any MultiplayerSpec")
    }

    pub fn seats(&self) -> &[Seat] {
        &self.seats
    }

    pub fn player_count(&self) -> u8 {
        self.seats.len() as u8
    }

    pub fn has_cpu(&self) -> bool {
        self.seats
            .iter()
            .any(|seat| seat.controller == Controller::Cpu)
    }

    pub fn has_human(&self) -> bool {
        self.seats
            .iter()
            .any(|seat| seat.controller == Controller::Human)
    }

    pub fn controller(&self, id: PlayerId) -> Option<Controller> {
        self.seats
            .iter()
            .find(|seat| seat.id == id)
            .map(|seat| seat.controller)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchError {
    PlayerCount { got: u8, min: u8, max: u8 },
    DuplicatePlayer(PlayerId),
    WrongPlayers,
}

/// Required behavior for every multiplayer game.
///
/// `control_cpu` is the CPU player. The host calls [`Self::tick_cpus`] each
/// frame so a game cannot ship multiplayer without AI for CPU seats.
pub trait Multiplayer {
    fn spec(&self) -> MultiplayerSpec;
    fn match_config(&self) -> &MatchConfig;
    fn apply_match(&mut self, config: &MatchConfig) -> Result<(), MatchError>;
    fn control_cpu(&mut self, player: PlayerId, delta: Duration);

    fn tick_cpus(&mut self, delta: Duration) {
        let cpus: Vec<PlayerId> = self
            .match_config()
            .seats()
            .iter()
            .filter(|seat| seat.controller == Controller::Cpu)
            .map(|seat| seat.id)
            .collect();
        for id in cpus {
            self.control_cpu(id, delta);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_rejects_single_player_ranges() {
        assert!(MultiplayerSpec::try_new(1, 2).is_none());
        assert!(MultiplayerSpec::try_new(2, 1).is_none());
        assert!(MultiplayerSpec::try_new(0, 0).is_none());
        let spec = MultiplayerSpec::try_new(2, 4).unwrap();
        assert_eq!(spec.min_players(), 2);
        assert_eq!(spec.max_players(), 4);
    }

    #[test]
    fn vs_cpu_is_one_human_and_the_rest_cpu() {
        let spec = MultiplayerSpec::try_new(2, 4).unwrap();
        let config = MatchConfig::vs_cpu(spec);
        assert_eq!(config.seats(), &[Seat::human(0), Seat::cpu(1)]);
        assert!(config.has_cpu());
        assert!(config.has_human());
        assert_eq!(config.controller(PlayerId(0)), Some(Controller::Human));
        assert_eq!(config.controller(PlayerId(1)), Some(Controller::Cpu));
    }

    #[test]
    fn vs_cpu_fills_min_players_so_a_four_player_minimum_still_has_cpus() {
        let spec = MultiplayerSpec::try_new(4, 4).unwrap();
        let config = MatchConfig::vs_cpu(spec);
        assert_eq!(config.player_count(), 4);
        assert_eq!(
            config
                .seats()
                .iter()
                .filter(|s| s.controller == Controller::Cpu)
                .count(),
            3
        );
    }

    #[test]
    fn new_rejects_counts_outside_the_spec() {
        let spec = MultiplayerSpec::TWO_PLAYER;
        let err = MatchConfig::new(spec, vec![Seat::human(0)]).unwrap_err();
        assert_eq!(
            err,
            MatchError::PlayerCount {
                got: 1,
                min: 2,
                max: 2
            }
        );
    }

    #[test]
    fn new_rejects_duplicate_ids() {
        let spec = MultiplayerSpec::TWO_PLAYER;
        let err = MatchConfig::new(spec, vec![Seat::human(0), Seat::cpu(0)]).unwrap_err();
        assert_eq!(err, MatchError::DuplicatePlayer(PlayerId(0)));
    }

    #[test]
    fn all_human_lineups_are_allowed_when_constructed_explicitly() {
        let spec = MultiplayerSpec::TWO_PLAYER;
        let config = MatchConfig::new(spec, vec![Seat::human(0), Seat::human(1)]).unwrap();
        assert!(!config.has_cpu());
        assert!(config.has_human());
    }

    #[test]
    fn vs_player_is_two_humans_and_cpu_for_any_extra_seats() {
        let two = MatchConfig::vs_player(MultiplayerSpec::TWO_PLAYER);
        assert_eq!(two.seats(), &[Seat::human(0), Seat::human(1)]);
        assert!(!two.has_cpu());

        let four = MatchConfig::vs_player(MultiplayerSpec::try_new(4, 4).unwrap());
        assert_eq!(
            four.seats(),
            &[Seat::human(0), Seat::human(1), Seat::cpu(2), Seat::cpu(3)]
        );
        assert!(four.has_cpu());
    }
}
