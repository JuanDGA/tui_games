use std::collections::VecDeque;
use std::time::Duration;

use crossterm::event::{Event, KeyCode};
use rand::Rng;
use ratatui::{Frame, prelude::*, symbols::border, widgets::Block};

use super::controls::letter;
use super::{Action, ActionPhase, Game, KeyMap, PlayerId};
use crate::grid::GridLayout;

const GRID_COLS: u16 = 20;
const GRID_ROWS: u16 = 20;
const TICK_MS: u64 = 150;

const SNAKE_YELLOW: Color = Color::LightYellow;
const SNAKE_BODY: Color = Color::LightGreen;
const DIGESTING: Color = Color::Rgb(38, 102, 48);

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
    digesting: VecDeque<Pos>,
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
            digesting: VecDeque::new(),
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
        let Some(new_head) = Self::step(head, self.direction) else {
            self.game_over = true;
            return;
        };

        let growing = self.digesting.front().copied() == self.snake.back().copied();
        let hits_body = self.snake.iter().enumerate().any(|(i, seg)| {
            let vacating_tail = !growing && i + 1 == self.snake.len();
            !vacating_tail && *seg == new_head
        });
        if hits_body {
            self.game_over = true;
            return;
        }

        self.snake.push_front(new_head);

        let ate = new_head == self.food;
        if ate {
            self.score += 1;
            self.digesting.push_back(new_head);
        }

        if growing {
            self.digesting.pop_front();
        } else {
            self.snake.pop_back();
        }

        if ate {
            self.spawn_food();
        }
    }

    fn step(head: Pos, direction: Direction) -> Option<Pos> {
        let (col, row) = match direction {
            Direction::Up if head.row > 0 => (head.col, head.row - 1),
            Direction::Down if head.row + 1 < GRID_ROWS => (head.col, head.row + 1),
            Direction::Left if head.col > 0 => (head.col - 1, head.row),
            Direction::Right if head.col + 1 < GRID_COLS => (head.col + 1, head.row),
            _ => return None,
        };
        Some(Pos { col, row })
    }

    fn segment_color(&self, index: usize, segment: Pos) -> Color {
        if index == 0 {
            SNAKE_YELLOW
        } else if self.digesting.iter().any(|pos| *pos == segment) {
            DIGESTING
        } else {
            SNAKE_BODY
        }
    }
}

impl Game for SnakeGame {
    fn name(&self) -> &'static str {
        "Snake"
    }

    fn poll_timeout(&self) -> Duration {
        Duration::from_millis(TICK_MS).saturating_sub(self.tick_accumulator)
    }

    fn min_size(&self) -> (u16, u16) {
        (GRID_COLS + 2, GRID_ROWS + 2)
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
        self.digesting.clear();
        self.score = 0;
        self.game_over = false;
        self.tick_accumulator = Duration::ZERO;
        self.spawn_food();
    }

    fn handle_event(&mut self, event: Event) {
        let Event::Key(key) = event else { return };
        if !crate::kitty::is_action(key.kind) {
            return;
        }
        if let Some((player, action)) = self.controls().lookup(key.code) {
            self.handle_action(player, action, ActionPhase::Start);
        }
    }

    fn controls(&self) -> KeyMap {
        KeyMap::new()
            .player_a(Action::UP, [KeyCode::Up])
            .player_a(Action::DOWN, [KeyCode::Down])
            .player_a(Action::LEFT, [KeyCode::Left])
            .player_a(Action::RIGHT, [KeyCode::Right])
            .player_a(Action::END, letter('x'))
    }

    fn handle_action(&mut self, _player: PlayerId, action: Action, phase: ActionPhase) {
        if !phase.is_start() {
            return;
        }
        match action {
            Action::UP => self.next_direction = Direction::Up,
            Action::DOWN => self.next_direction = Direction::Down,
            Action::LEFT => self.next_direction = Direction::Left,
            Action::RIGHT => self.next_direction = Direction::Right,
            Action::END => self.game_over = true,
            _ => {}
        }
    }

    fn update(&mut self, delta: Duration) {
        if self.game_over {
            return;
        }

        self.tick_accumulator += delta;
        let tick = Duration::from_millis(TICK_MS);
        while self.tick_accumulator >= tick {
            self.tick_accumulator -= tick;
            self.tick();
            if self.game_over {
                break;
            }
        }
    }

    fn render(&mut self, frame: &mut Frame) {
        let area = frame.area();
        let grid = GridLayout::with_border(area, GRID_COLS, GRID_ROWS);
        if !grid.fits(area) {
            return;
        }
        let title = Line::from(self.name()).bold();
        let score = Line::from(format!(" Score: {} ", self.score));
        let block = Block::bordered()
            .title(title.centered())
            .title_bottom(score.centered())
            .border_set(border::THICK);
        frame.render_widget(&block, grid.border_rect());

        fill_cell(
            frame,
            grid.cell_rect(self.food.col, self.food.row),
            Style::default().fg(Color::Red),
        );

        for (i, segment) in self.snake.iter().enumerate() {
            let color = self.segment_color(i, *segment);
            fill_cell(
                frame,
                grid.cell_rect(segment.col, segment.row),
                Style::default().fg(color).bg(color),
            );
        }
    }

    fn is_game_over(&self) -> bool {
        self.game_over
    }

    fn score(&self) -> u32 {
        self.score
    }
}

fn fill_cell(frame: &mut Frame, cell: Rect, style: Style) {
    let cell = cell.intersection(frame.area());
    for y in cell.y..cell.y + cell.height {
        for x in cell.x..cell.x + cell.width {
            frame.buffer_mut()[(x, y)].set_symbol("█").set_style(style);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game_with_food(col: u16, row: u16) -> SnakeGame {
        let mut game = SnakeGame::new();
        game.food = Pos { col, row };
        game
    }

    fn park_food(game: &mut SnakeGame) {
        game.food = Pos { col: 0, row: 0 };
    }

    fn set_snake(game: &mut SnakeGame, cells: &[(u16, u16)], direction: Direction) {
        game.snake.clear();
        for &(col, row) in cells {
            game.snake.push_back(Pos { col, row });
        }
        game.direction = direction;
        game.next_direction = direction;
        game.digesting.clear();
        game.game_over = false;
    }

    #[test]
    fn eating_does_not_grow_immediately() {
        let mut game = game_with_food(GRID_COLS / 2 + 1, GRID_ROWS / 2);
        let len = game.snake.len();

        game.tick();

        assert!(!game.game_over);
        assert_eq!(game.snake.len(), len);
        assert_eq!(game.score, 1);
        assert_eq!(game.digesting.len(), 1);
        assert_eq!(game.digesting[0], *game.snake.front().unwrap());
        assert_eq!(
            game.segment_color(0, *game.snake.front().unwrap()),
            SNAKE_YELLOW
        );
    }

    #[test]
    fn snake_grows_when_tail_leaves_the_eaten_square() {
        let mut game = game_with_food(GRID_COLS / 2 + 1, GRID_ROWS / 2);
        let len = game.snake.len();
        let eat_at = game.food;

        game.tick();
        park_food(&mut game);
        assert_eq!(game.snake.len(), len);
        assert!(game.snake.contains(&eat_at));

        game.tick();
        assert_eq!(game.snake.len(), len);
        assert!(game.snake.contains(&eat_at));

        game.tick();
        assert_eq!(game.snake.len(), len);
        assert_eq!(game.snake.back().copied(), Some(eat_at));

        game.tick();
        assert_eq!(game.snake.len(), len + 1);
        assert!(game.digesting.is_empty());
        assert_eq!(game.snake.back().copied(), Some(eat_at));
    }

    #[test]
    fn swallowed_food_travels_one_segment_per_tick() {
        let mut game = game_with_food(GRID_COLS / 2 + 1, GRID_ROWS / 2);
        let eat_at = game.food;

        game.tick();
        park_food(&mut game);
        assert_eq!(game.snake[0], eat_at);

        game.tick();
        assert_eq!(game.snake[1], eat_at);
        assert!(game.digesting.iter().any(|pos| *pos == eat_at));

        game.tick();
        assert_eq!(game.snake[2], eat_at);
    }

    #[test]
    fn two_meals_grow_in_order_as_each_reaches_the_tail() {
        let mut game = game_with_food(GRID_COLS / 2 + 1, GRID_ROWS / 2);
        let start_len = game.snake.len();

        game.tick();
        assert_eq!(game.snake.len(), start_len);

        game.food = Pos {
            col: GRID_COLS / 2 + 2,
            row: GRID_ROWS / 2,
        };
        game.tick();
        park_food(&mut game);
        assert_eq!(game.score, 2);
        assert_eq!(game.digesting.len(), 2);
        assert_eq!(game.snake.len(), start_len);

        game.tick();
        assert_eq!(game.snake.len(), start_len);
        game.tick();
        assert_eq!(game.snake.len(), start_len + 1);
        assert_eq!(game.digesting.len(), 1);

        game.tick();
        assert_eq!(game.snake.len(), start_len + 1);
        game.tick();
        assert_eq!(game.snake.len(), start_len + 2);
        assert!(game.digesting.is_empty());
    }

    #[test]
    fn eating_and_continuing_straight_does_not_kill() {
        let mut game = game_with_food(GRID_COLS / 2 + 1, GRID_ROWS / 2);

        game.tick();
        park_food(&mut game);
        assert!(!game.game_over);
        assert_eq!(game.score, 1);

        game.tick();
        assert!(!game.game_over);
        assert_eq!(
            game.segment_color(0, *game.snake.front().unwrap()),
            SNAKE_YELLOW
        );
        assert_eq!(game.snake[1], game.digesting[0]);
        assert_eq!(game.segment_color(1, game.snake[1]), DIGESTING);

        game.tick();
        assert!(!game.game_over);
    }

    #[test]
    fn moving_into_vacating_tail_is_safe() {
        let mut game = SnakeGame::new();
        set_snake(
            &mut game,
            &[(1, 0), (1, 1), (0, 1), (0, 0)],
            Direction::Left,
        );
        park_food(&mut game);

        game.tick();

        assert!(!game.game_over);
        assert_eq!(game.snake.front().copied(), Some(Pos { col: 0, row: 0 }));
    }

    #[test]
    fn moving_into_tail_kills_when_that_cell_would_grow() {
        let mut game = SnakeGame::new();
        set_snake(
            &mut game,
            &[(1, 0), (1, 1), (0, 1), (0, 0)],
            Direction::Left,
        );
        game.digesting.push_back(Pos { col: 0, row: 0 });
        park_food(&mut game);

        game.tick();

        assert!(game.game_over);
    }

    #[test]
    fn head_stays_yellow_while_food_is_in_the_mouth() {
        let mut game = game_with_food(GRID_COLS / 2 + 1, GRID_ROWS / 2);
        game.tick();
        let head = *game.snake.front().unwrap();
        assert_eq!(game.segment_color(0, head), SNAKE_YELLOW);
        assert_eq!(game.digesting[0], head);
    }

    fn press(code: KeyCode) -> Event {
        Event::Key(crossterm::event::KeyEvent::new(
            code,
            crossterm::event::KeyModifiers::NONE,
        ))
    }

    fn repeat(code: KeyCode) -> Event {
        Event::Key(crossterm::event::KeyEvent::new_with_kind(
            code,
            crossterm::event::KeyModifiers::NONE,
            crossterm::event::KeyEventKind::Repeat,
        ))
    }

    fn release(code: KeyCode) -> Event {
        Event::Key(crossterm::event::KeyEvent::new_with_kind(
            code,
            crossterm::event::KeyModifiers::NONE,
            crossterm::event::KeyEventKind::Release,
        ))
    }

    #[test]
    fn queued_turn_applies_on_the_next_tick() {
        let mut game = SnakeGame::new();
        park_food(&mut game);
        let head = game.snake.front().copied().unwrap();

        game.handle_event(press(KeyCode::Up));
        assert_eq!(game.snake.front().copied(), Some(head));

        game.tick();
        assert_eq!(
            game.snake.front().copied(),
            Some(Pos {
                col: head.col,
                row: head.row - 1
            })
        );
    }

    #[test]
    fn kitty_repeat_turns_and_release_does_not() {
        let mut game = SnakeGame::new();
        park_food(&mut game);

        game.handle_event(repeat(KeyCode::Up));
        assert_eq!(game.next_direction, Direction::Up);

        game.handle_event(release(KeyCode::Down));
        assert_eq!(game.next_direction, Direction::Up);
    }

    #[test]
    fn poll_timeout_is_the_time_left_until_the_next_move() {
        let mut game = SnakeGame::new();
        assert_eq!(game.poll_timeout(), Duration::from_millis(TICK_MS));
        game.update(Duration::from_millis(40));
        assert_eq!(game.poll_timeout(), Duration::from_millis(TICK_MS - 40));
    }

    #[test]
    fn update_runs_every_elapsed_tick() {
        let mut game = SnakeGame::new();
        park_food(&mut game);
        let start = game.snake.front().copied().unwrap();
        game.update(Duration::from_millis(TICK_MS * 3 + 10));
        assert_eq!(
            game.snake.front().copied(),
            Some(Pos {
                col: start.col + 3,
                row: start.row
            })
        );
        assert_eq!(game.poll_timeout(), Duration::from_millis(TICK_MS - 10));
    }

    #[test]
    fn min_size_fits_one_cell_cells_and_a_border() {
        let game = SnakeGame::new();
        assert_eq!(game.min_size(), (GRID_COLS + 2, GRID_ROWS + 2));
    }

    #[test]
    fn render_does_not_panic_when_the_terminal_is_too_small() {
        let backend = ratatui::backend::TestBackend::new(172, 8);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut game = SnakeGame::new();
        terminal.draw(|frame| game.render(frame)).unwrap();
    }
}
