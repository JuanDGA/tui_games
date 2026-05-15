use std::time::Duration;

use crossterm::event::{Event, KeyCode, KeyEventKind};
use ratatui::{Frame, prelude::*, symbols::border, widgets::{Block, Paragraph}};

use crate::grid::GridLayout;
use super::Game;

const GRID_COLS: u16 = 16;
const GRID_ROWS: u16 = 16;

#[derive(Debug)]
pub struct DemoGame {
    head: Pos,
    score: u32,
    game_over: bool,
}

#[derive(Debug, Default, Clone, Copy)]
struct Pos {
    col: u16,
    row: u16,
}

impl DemoGame {
    pub fn new() -> Self {
        Self {
            head: Pos::default(),
            score: 0,
            game_over: false,
        }
    }
}

impl Game for DemoGame {
    fn name(&self) -> &'static str {
        "Square Demo"
    }

    fn instructions(&self) -> &'static str {
        "Arrow keys to move. X to end game."
    }

    fn poll_timeout(&self) -> Duration {
        Duration::from_millis(16)
    }

    fn grid_size(&self) -> (u16, u16) {
        (GRID_COLS, GRID_ROWS)
    }

    fn reset(&mut self) {
        self.head = Pos::default();
        self.score = 0;
        self.game_over = false;
    }

    fn handle_event(&mut self, event: Event) {
        let Event::Key(key) = event else { return };
        if key.kind != KeyEventKind::Press {
            return;
        }

        match key.code {
            KeyCode::Left if self.head.col > 0 => self.head.col -= 1,
            KeyCode::Right if self.head.col + 1 < GRID_COLS => self.head.col += 1,
            KeyCode::Up if self.head.row > 0 => self.head.row -= 1,
            KeyCode::Down if self.head.row + 1 < GRID_ROWS => self.head.row += 1,
            KeyCode::Char('x') | KeyCode::Char('X') => self.game_over = true,
            _ => {}
        }
    }

    fn update(&mut self, _delta: Duration) {
        if !self.game_over {
            self.score += 1;
        }
    }

    fn render(&mut self, frame: &mut Frame) {
        let area = frame.area();

        let title = Line::from(self.name()).bold();
        let block = Block::bordered()
            .title(title.centered())
            .border_set(border::THICK);

        frame.render_widget(&block, area);
        let inner = block.inner(area);

        let grid = GridLayout::new(inner, GRID_COLS, GRID_ROWS);
        let cell = grid.cell_rect(self.head.col, self.head.row);

        for y in cell.y..cell.y + cell.height {
            for x in cell.x..cell.x + cell.width {
                frame.buffer_mut()[(x, y)].set_symbol("█");
            }
        }

        let score_text = Text::from(format!("Score: {}", self.score));
        let score_area = Rect {
            x: inner.x,
            y: inner.y + inner.height.saturating_sub(1),
            width: inner.width,
            height: 1,
        };
        frame.render_widget(Paragraph::new(score_text), score_area);
    }

    fn is_game_over(&self) -> bool {
        self.game_over
    }

    fn score(&self) -> u32 {
        self.score
    }
}
