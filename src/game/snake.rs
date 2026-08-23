use std::collections::VecDeque;
use std::time::Duration;

use crossterm::event::{Event, KeyCode, KeyEventKind};
use rand::Rng;
use ratatui::{Frame, prelude::*, symbols::border, widgets::{Block, Paragraph}};

use crate::grid::GridLayout;
use super::Game;

const GRID_COLS: u16 = 20;
const GRID_ROWS: u16 = 20;
const TICK_MS: u64 = 150;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Pos {
    col: u16,
    row: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    Up,
    Down,
    Left,
    Right,
}

impl Direction {
    fn opposite(self) -> Self {
        match self {
            Direction::Up => Direction::Down,
            Direction::Down => Direction::Up,
            Direction::Left => Direction::Right,
            Direction::Right => Direction::Left,
        }
    }
}

#[derive(Debug)]
pub struct SnakeGame {
    snake: VecDeque<Pos>,
    direction: Direction,
    next_direction: Direction,
    food: Pos,
    score: u32,
    game_over: bool,
    tick_accumulator: Duration,
}

impl SnakeGame {
    pub fn new() -> Self {
        let mut game = Self {
            snake: VecDeque::new(),
            direction: Direction::Right,
            next_direction: Direction::Right,
            food: Pos { col: 0, row: 0 },
            score: 0,
            game_over: false,
            tick_accumulator: Duration::ZERO,
        };
        game.reset();
        game
    }

    fn spawn_food(&mut self) {
        let mut rng = rand::thread_rng();
        loop {
            let col = rng.gen_range(0..GRID_COLS);
            let row = rng.gen_range(0..GRID_ROWS);
            if !self.snake.iter().any(|s| s.col == col && s.row == row) {
                self.food = Pos { col, row };
                break;
            }
        }
    }

    fn tick(&mut self) {
        if self.game_over {
            return;
        }

        if self.next_direction != self.direction.opposite() {
            self.direction = self.next_direction;
        }

        let head = self.snake.front().copied().unwrap();
        let new_head = match self.direction {
            Direction::Up => Pos {
                col: head.col,
                row: head.row.saturating_sub(1),
            },
            Direction::Down => Pos {
                col: head.col,
                row: head.row + 1,
            },
            Direction::Left => Pos {
                col: head.col.saturating_sub(1),
                row: head.row,
            },
            Direction::Right => Pos {
                col: head.col + 1,
                row: head.row,
            },
        };

        if new_head.col >= GRID_COLS || new_head.row >= GRID_ROWS {
            self.game_over = true;
            return;
        }

        if self.snake.iter().any(|s| s.col == new_head.col && s.row == new_head.row) {
            self.game_over = true;
            return;
        }

        self.snake.push_front(new_head);

        if new_head.col == self.food.col && new_head.row == self.food.row {
            self.score += 1;
            self.spawn_food();
        } else {
            self.snake.pop_back();
        }
    }
}

impl Game for SnakeGame {
    fn name(&self) -> &'static str {
        "Snake"
    }

    fn instructions(&self) -> &'static str {
        "Arrow keys to steer. Esc to pause."
    }

    fn poll_timeout(&self) -> Duration {
        Duration::from_millis(TICK_MS)
    }

    fn grid_size(&self) -> (u16, u16) {
        (GRID_COLS, GRID_ROWS)
    }

    fn reset(&mut self) {
        self.snake.clear();
        let mid_col = GRID_COLS / 2;
        let mid_row = GRID_ROWS / 2;
        self.snake.push_back(Pos {
            col: mid_col,
            row: mid_row,
        });
        self.snake.push_back(Pos {
            col: mid_col - 1,
            row: mid_row,
        });
        self.snake.push_back(Pos {
            col: mid_col - 2,
            row: mid_row,
        });
        self.direction = Direction::Right;
        self.next_direction = Direction::Right;
        self.score = 0;
        self.game_over = false;
        self.tick_accumulator = Duration::ZERO;
        self.spawn_food();
    }

    fn handle_event(&mut self, event: Event) {
        let Event::Key(key) = event else { return };
        if key.kind != KeyEventKind::Press {
            return;
        }

        match key.code {
            KeyCode::Up => self.next_direction = Direction::Up,
            KeyCode::Down => self.next_direction = Direction::Down,
            KeyCode::Left => self.next_direction = Direction::Left,
            KeyCode::Right => self.next_direction = Direction::Right,
            KeyCode::Char('x') | KeyCode::Char('X') => self.game_over = true,
            _ => {}
        }
    }

    fn update(&mut self, delta: Duration) {
        if self.game_over {
            return;
        }

        self.tick_accumulator += delta;
        let tick = Duration::from_millis(TICK_MS);
        if self.tick_accumulator >= tick {
            self.tick_accumulator -= tick;
            self.tick();
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

        let food_cell = grid.cell_rect(self.food.col, self.food.row);
        let food_style = Style::default().fg(Color::Red);
        for y in food_cell.y..food_cell.y + food_cell.height {
            for x in food_cell.x..food_cell.x + food_cell.width {
                frame.buffer_mut()[(x, y)].set_symbol("█").set_style(food_style);
            }
        }

        for (i, segment) in self.snake.iter().enumerate() {
            let cell = grid.cell_rect(segment.col, segment.row);
            let style = if i == 0 {
                Style::default().fg(Color::LightGreen)
            } else {
                Style::default().fg(Color::Green)
            };
            for y in cell.y..cell.y + cell.height {
                for x in cell.x..cell.x + cell.width {
                    frame.buffer_mut()[(x, y)].set_symbol("█").set_style(style);
                }
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
