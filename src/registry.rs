use crate::game::Game;

pub struct GameEntry {
    pub name: &'static str,
    pub factory: fn() -> Box<dyn Game>,
}

pub fn entries() -> &'static [GameEntry] {
    &[
        GameEntry {
            name: "Snake",
            factory: || Box::new(crate::game::snake::SnakeGame::new()),
        },
        GameEntry {
            name: "Tetris",
            factory: || Box::new(crate::game::tetris::TetrisGame::new()),
        },
        GameEntry {
            name: "Pong",
            factory: || Box::new(crate::game::pong::PongGame::new()),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Multiplayer;

    #[test]
    fn every_multiplayer_game_launches_with_a_cpu_player() {
        for entry in entries() {
            let mut game = (entry.factory)();
            if game.as_multiplayer().is_none() {
                continue;
            }
            game.setup_default_match();
            let mp = game.as_multiplayer().expect(entry.name);
            assert!(
                mp.match_config().has_cpu(),
                "{} default match must include a CPU",
                entry.name
            );
            assert!(
                mp.spec().min_players() >= 2,
                "{} must require at least two players",
                entry.name
            );
        }
    }

    #[test]
    fn pong_accepts_vs_player() {
        let mut game = crate::game::pong::PongGame::new();
        let spec = game.spec();
        let config = crate::game::MatchConfig::vs_player(spec);
        assert!(game.apply_match(&config).is_ok());
        assert!(!game.match_config().has_cpu());
    }

    #[test]
    fn snake_and_tetris_are_single_player() {
        for name in ["Snake", "Tetris"] {
            let entry = entries().iter().find(|e| e.name == name).unwrap();
            let mut game = (entry.factory)();
            assert!(
                game.as_multiplayer().is_none(),
                "{name} should not expose multiplayer"
            );
        }
    }
}
