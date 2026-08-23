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
    ]
}
