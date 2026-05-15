use crate::game::Game;

#[allow(dead_code)]
pub struct GameEntry {
    pub name: &'static str,
    pub instructions: &'static str,
    pub factory: fn() -> Box<dyn Game>,
}

pub fn entries() -> &'static [GameEntry] {
    &[
        GameEntry {
            name: "Square Demo",
            instructions: "Arrow keys to move. X to end game.",
            factory: || Box::new(crate::game::demo::DemoGame::new()),
        },
    ]
}
