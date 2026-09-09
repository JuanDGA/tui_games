use std::time::Duration;

use crossterm::event::{Event, KeyCode};
use rand::seq::SliceRandom;
use ratatui::{
    Frame,
    prelude::*,
    symbols::border,
    widgets::{Block, Paragraph},
};

use super::controls::letter;
use super::{Action, ActionPhase, Game, KeyMap, PlayerId};
use crate::grid::GridLayout;

const BOARD_COLS: usize = 10;
const BOARD_ROWS: usize = 20;
const SPAWN_COL: i16 = 3;
const INFO_WIDTH: u16 = 18;
const BASE_GRAVITY_MS: u64 = 800;
const MIN_GRAVITY_MS: u64 = 80;
const GRAVITY_STEP_MS: u64 = 70;

const WELL: Color = Color::Rgb(18, 18, 28);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    I,
    O,
    T,
    S,
    Z,
    J,
    L,
}

impl Kind {
    const ALL: [Kind; 7] = [
        Kind::I,
        Kind::O,
        Kind::T,
        Kind::S,
        Kind::Z,
        Kind::J,
        Kind::L,
    ];

    fn color(self) -> Color {
        match self {
            Kind::I => Color::Cyan,
            Kind::O => Color::Yellow,
            Kind::T => Color::Magenta,
            Kind::S => Color::Green,
            Kind::Z => Color::Red,
            Kind::J => Color::Blue,
            Kind::L => Color::Rgb(255, 140, 0),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Pos {
    col: i16,
    row: i16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Piece {
    kind: Kind,
    rotation: u8,
    col: i16,
    row: i16,
}

impl Piece {
    fn cells(self) -> [Pos; 4] {
        offsets(self.kind, self.rotation).map(|p| Pos {
            col: self.col + p.col,
            row: self.row + p.row,
        })
    }
}

fn p(col: i16, row: i16) -> Pos {
    Pos { col, row }
}

fn offsets(kind: Kind, rotation: u8) -> [Pos; 4] {
    match (kind, rotation % 4) {
        (Kind::I, 0) => [p(0, 0), p(1, 0), p(2, 0), p(3, 0)],
        (Kind::I, 1) => [p(2, 0), p(2, 1), p(2, 2), p(2, 3)],
        (Kind::I, 2) => [p(0, 1), p(1, 1), p(2, 1), p(3, 1)],
        (Kind::I, _) => [p(1, 0), p(1, 1), p(1, 2), p(1, 3)],

        (Kind::O, _) => [p(1, 0), p(2, 0), p(1, 1), p(2, 1)],

        (Kind::T, 0) => [p(1, 0), p(0, 1), p(1, 1), p(2, 1)],
        (Kind::T, 1) => [p(1, 0), p(1, 1), p(2, 1), p(1, 2)],
        (Kind::T, 2) => [p(0, 1), p(1, 1), p(2, 1), p(1, 2)],
        (Kind::T, _) => [p(1, 0), p(0, 1), p(1, 1), p(1, 2)],

        (Kind::S, 0) => [p(1, 0), p(2, 0), p(0, 1), p(1, 1)],
        (Kind::S, 1) => [p(1, 0), p(1, 1), p(2, 1), p(2, 2)],
        (Kind::S, 2) => [p(1, 1), p(2, 1), p(0, 2), p(1, 2)],
        (Kind::S, _) => [p(0, 0), p(0, 1), p(1, 1), p(1, 2)],

        (Kind::Z, 0) => [p(0, 0), p(1, 0), p(1, 1), p(2, 1)],
        (Kind::Z, 1) => [p(2, 0), p(1, 1), p(2, 1), p(1, 2)],
        (Kind::Z, 2) => [p(0, 1), p(1, 1), p(1, 2), p(2, 2)],
        (Kind::Z, _) => [p(1, 0), p(0, 1), p(1, 1), p(0, 2)],

        (Kind::J, 0) => [p(0, 0), p(0, 1), p(1, 1), p(2, 1)],
        (Kind::J, 1) => [p(1, 0), p(2, 0), p(1, 1), p(1, 2)],
        (Kind::J, 2) => [p(0, 1), p(1, 1), p(2, 1), p(2, 2)],
        (Kind::J, _) => [p(1, 0), p(1, 1), p(0, 2), p(1, 2)],

        (Kind::L, 0) => [p(2, 0), p(0, 1), p(1, 1), p(2, 1)],
        (Kind::L, 1) => [p(1, 0), p(1, 1), p(1, 2), p(2, 2)],
        (Kind::L, 2) => [p(0, 1), p(1, 1), p(2, 1), p(0, 2)],
        (Kind::L, _) => [p(0, 0), p(1, 0), p(1, 1), p(1, 2)],
    }
}

fn gravity_ms(level: u32) -> u64 {
    BASE_GRAVITY_MS
        .saturating_sub(GRAVITY_STEP_MS * u64::from(level.saturating_sub(1)))
        .max(MIN_GRAVITY_MS)
}

fn line_score(cleared: u32, level: u32) -> u32 {
    let base = match cleared {
        1 => 100,
        2 => 300,
        3 => 500,
        4 => 800,
        _ => 0,
    };
    base * level.max(1)
}

#[derive(Debug)]
pub struct TetrisGame {
    board: [[Option<Kind>; BOARD_COLS]; BOARD_ROWS],
    current: Option<Piece>,
    next: Kind,
    bag: Vec<Kind>,
    score: u32,
    lines: u32,
    level: u32,
    game_over: bool,
    tick_accumulator: Duration,
}

impl TetrisGame {
    pub fn new() -> Self {
        let mut game = Self {
            board: [[None; BOARD_COLS]; BOARD_ROWS],
            current: None,
            next: Kind::I,
            bag: Vec::new(),
            score: 0,
            lines: 0,
            level: 1,
            game_over: false,
            tick_accumulator: Duration::ZERO,
        };
        game.reset();
        game
    }

    fn take_next_kind(&mut self) -> Kind {
        if self.bag.is_empty() {
            let mut pieces = Kind::ALL.to_vec();
            pieces.shuffle(&mut rand::thread_rng());
            self.bag = pieces;
        }
        self.bag.pop().expect("7-bag refill produces pieces")
    }

    fn spawn_piece(&mut self) {
        let kind = self.next;
        self.next = self.take_next_kind();
        let piece = Piece {
            kind,
            rotation: 0,
            col: SPAWN_COL,
            row: 0,
        };
        self.current = Some(piece);
        if !self.fits(piece) {
            self.game_over = true;
        }
    }

    fn fits(&self, piece: Piece) -> bool {
        piece.cells().iter().all(|cell| {
            if cell.col < 0 || cell.col >= BOARD_COLS as i16 || cell.row >= BOARD_ROWS as i16 {
                return false;
            }
            if cell.row < 0 {
                return true;
            }
            self.board[cell.row as usize][cell.col as usize].is_none()
        })
    }

    fn try_move(&mut self, dcol: i16, drow: i16) -> bool {
        let Some(mut piece) = self.current else {
            return false;
        };
        piece.col += dcol;
        piece.row += drow;
        if self.fits(piece) {
            self.current = Some(piece);
            true
        } else {
            false
        }
    }

    fn try_rotate(&mut self, dir: i8) {
        let Some(mut piece) = self.current else {
            return;
        };
        if piece.kind == Kind::O {
            return;
        }

        piece.rotation = (piece.rotation as i8 + dir).rem_euclid(4) as u8;
        const KICKS: [(i16, i16); 8] = [
            (0, 0),
            (-1, 0),
            (1, 0),
            (-2, 0),
            (2, 0),
            (0, -1),
            (-1, -1),
            (1, -1),
        ];
        let origin_col = piece.col;
        let origin_row = piece.row;
        for (dcol, drow) in KICKS {
            piece.col = origin_col + dcol;
            piece.row = origin_row + drow;
            if self.fits(piece) {
                self.current = Some(piece);
                return;
            }
        }
    }

    fn tick(&mut self) {
        if self.game_over {
            return;
        }
        if !self.try_move(0, 1) {
            self.lock_current();
        }
    }

    fn lock_current(&mut self) {
        let Some(piece) = self.current.take() else {
            return;
        };
        for cell in piece.cells() {
            if cell.row < 0 {
                self.game_over = true;
                continue;
            }
            if cell.row >= 0
                && (cell.row as usize) < BOARD_ROWS
                && cell.col >= 0
                && (cell.col as usize) < BOARD_COLS
            {
                self.board[cell.row as usize][cell.col as usize] = Some(piece.kind);
            }
        }
        self.clear_lines();
        if !self.game_over {
            self.spawn_piece();
        }
    }

    fn clear_lines(&mut self) {
        let mut next = [[None; BOARD_COLS]; BOARD_ROWS];
        let mut write_row = BOARD_ROWS - 1;
        let mut cleared = 0u32;

        for row in (0..BOARD_ROWS).rev() {
            if self.board[row].iter().all(Option::is_some) {
                cleared += 1;
            } else {
                next[write_row] = self.board[row];
                write_row = write_row.saturating_sub(1);
            }
        }

        if cleared == 0 {
            return;
        }

        self.board = next;
        self.score += line_score(cleared, self.level);
        self.lines += cleared;
        self.level = self.lines / 10 + 1;
    }

    fn hard_drop(&mut self) {
        let mut dropped = 0;
        while self.try_move(0, 1) {
            dropped += 1;
        }
        self.score += dropped * 2;
        self.lock_current();
    }

    fn soft_drop(&mut self) {
        if self.try_move(0, 1) {
            self.score += 1;
            self.tick_accumulator = Duration::ZERO;
        } else {
            self.lock_current();
        }
    }

    fn ghost_offset(&self) -> i16 {
        let Some(piece) = self.current else {
            return 0;
        };
        let mut offset = 0;
        loop {
            let mut shifted = piece;
            shifted.row += offset + 1;
            if !self.fits(shifted) {
                return offset;
            }
            offset += 1;
        }
    }

    fn render_board(&self, frame: &mut Frame, grid: &GridLayout) {
        let title = Line::from(self.name()).bold();
        let block = Block::bordered()
            .title(title.centered())
            .border_set(border::THICK);
        frame.render_widget(&block, grid.border_rect());

        for row in 0..BOARD_ROWS {
            for col in 0..BOARD_COLS {
                let cell = grid.cell_rect(col as u16, row as u16);
                match self.board[row][col] {
                    Some(kind) => fill_cell(frame, cell, "█", filled_style(kind)),
                    None => fill_cell(frame, cell, " ", Style::default().bg(WELL)),
                }
            }
        }

        let Some(piece) = self.current else {
            return;
        };

        let ghost = self.ghost_offset();
        if ghost > 0 {
            for cell in piece.cells() {
                let row = cell.row + ghost;
                if in_board(cell.col, row) {
                    fill_cell(
                        frame,
                        grid.cell_rect(cell.col as u16, row as u16),
                        "░",
                        Style::default().fg(Color::DarkGray).bg(WELL),
                    );
                }
            }
        }

        for cell in piece.cells() {
            if in_board(cell.col, cell.row) {
                fill_cell(
                    frame,
                    grid.cell_rect(cell.col as u16, cell.row as u16),
                    "█",
                    filled_style(piece.kind),
                );
            }
        }
    }

    fn render_info(&self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::vertical([
            Constraint::Length(8),
            Constraint::Length(6),
            Constraint::Min(5),
        ])
        .split(area);

        self.render_next(frame, chunks[0]);

        let stats = Text::from(vec![
            Line::from(format!("Score {}", self.score)),
            Line::from(format!("Lines {}", self.lines)),
            Line::from(format!("Level {}", self.level)),
        ]);
        frame.render_widget(
            Paragraph::new(stats)
                .block(Block::bordered().title(Line::from(" Stats ").bold().centered())),
            chunks[1],
        );

        let help = Text::from(vec![
            Line::from("← → move"),
            Line::from("↑ / x rotate"),
            Line::from("z rotate ccw"),
            Line::from("↓ soft drop"),
            Line::from("space hard drop"),
        ]);
        frame.render_widget(
            Paragraph::new(help)
                .block(Block::bordered().title(Line::from(" Controls ").bold().centered())),
            chunks[2],
        );
    }

    fn render_next(&self, frame: &mut Frame, area: Rect) {
        let block = Block::bordered().title(Line::from(" Next ").bold().centered());
        let inner = block.inner(area);
        frame.render_widget(&block, area);

        let grid = GridLayout::new(inner, 4, 4);
        for cell in preview_cells(self.next) {
            if cell.col >= 0 && cell.row >= 0 && cell.col < 4 && cell.row < 4 {
                fill_cell(
                    frame,
                    grid.cell_rect(cell.col as u16, cell.row as u16),
                    "█",
                    filled_style(self.next),
                );
            }
        }
    }
}

impl Game for TetrisGame {
    fn name(&self) -> &'static str {
        "Tetris"
    }

    fn poll_timeout(&self) -> Duration {
        Duration::from_millis(gravity_ms(self.level)).saturating_sub(self.tick_accumulator)
    }

    fn min_size(&self) -> (u16, u16) {
        (BOARD_COLS as u16 + 2 + INFO_WIDTH, BOARD_ROWS as u16 + 2)
    }

    fn reset(&mut self) {
        self.board = [[None; BOARD_COLS]; BOARD_ROWS];
        self.bag.clear();
        self.score = 0;
        self.lines = 0;
        self.level = 1;
        self.game_over = false;
        self.tick_accumulator = Duration::ZERO;
        self.next = self.take_next_kind();
        self.spawn_piece();
    }

    fn handle_event(&mut self, event: Event) {
        if self.game_over {
            return;
        }
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
            .player_a(Action::LEFT, [KeyCode::Left])
            .player_a(Action::RIGHT, [KeyCode::Right])
            .player_a(Action::SOFT_DROP, [KeyCode::Down])
            .player_a(
                Action::ROTATE,
                [KeyCode::Up, KeyCode::Char('x'), KeyCode::Char('X')],
            )
            .player_a(Action::ROTATE_CCW, letter('z'))
            .player_a(Action::HARD_DROP, [KeyCode::Char(' ')])
    }

    fn handle_action(&mut self, _player: PlayerId, action: Action, phase: ActionPhase) {
        if self.game_over || !phase.is_start() {
            return;
        }
        match action {
            Action::LEFT => {
                self.try_move(-1, 0);
            }
            Action::RIGHT => {
                self.try_move(1, 0);
            }
            Action::SOFT_DROP => self.soft_drop(),
            Action::ROTATE => {
                self.try_rotate(1);
            }
            Action::ROTATE_CCW => {
                self.try_rotate(-1);
            }
            Action::HARD_DROP => self.hard_drop(),
            _ => {}
        }
    }

    fn update(&mut self, delta: Duration) {
        if self.game_over {
            return;
        }

        self.tick_accumulator += delta;
        let tick = Duration::from_millis(gravity_ms(self.level));
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
        let (min_w, min_h) = self.min_size();
        if area.width < min_w || area.height < min_h {
            return;
        }

        let probe_area = Rect {
            x: area.x,
            y: area.y,
            width: area.width.saturating_sub(INFO_WIDTH),
            height: area.height,
        };
        let probe = GridLayout::with_border(probe_area, BOARD_COLS as u16, BOARD_ROWS as u16);
        let border = probe.border_rect();
        let combined_w = border.width.saturating_add(INFO_WIDTH);
        let origin_x = area.x + area.width.saturating_sub(combined_w) / 2;
        let board_area = Rect {
            x: origin_x,
            y: border.y,
            width: border.width,
            height: border.height,
        };
        let info_area = Rect {
            x: origin_x.saturating_add(border.width),
            y: border.y,
            width: INFO_WIDTH,
            height: border.height,
        };
        let grid = GridLayout::with_border(board_area, BOARD_COLS as u16, BOARD_ROWS as u16);
        if !grid.fits(area) {
            return;
        }

        self.render_board(frame, &grid);
        self.render_info(frame, info_area);
    }

    fn is_game_over(&self) -> bool {
        self.game_over
    }

    fn score(&self) -> u32 {
        self.score
    }
}

fn in_board(col: i16, row: i16) -> bool {
    col >= 0 && row >= 0 && (col as usize) < BOARD_COLS && (row as usize) < BOARD_ROWS
}

fn filled_style(kind: Kind) -> Style {
    let color = kind.color();
    Style::default().fg(color).bg(color)
}

fn preview_cells(kind: Kind) -> [Pos; 4] {
    let cells = offsets(kind, 0);
    let min_c = cells.iter().map(|c| c.col).min().unwrap_or(0);
    let max_c = cells.iter().map(|c| c.col).max().unwrap_or(0);
    let min_r = cells.iter().map(|c| c.row).min().unwrap_or(0);
    let max_r = cells.iter().map(|c| c.row).max().unwrap_or(0);
    let width = max_c - min_c + 1;
    let height = max_r - min_r + 1;
    let dcol = (4 - width) / 2 - min_c;
    let drow = (4 - height) / 2 - min_r;
    cells.map(|cell| Pos {
        col: cell.col + dcol,
        row: cell.row + drow,
    })
}

fn fill_cell(frame: &mut Frame, cell: Rect, symbol: &str, style: Style) {
    let cell = cell.intersection(frame.area());
    for y in cell.y..cell.y + cell.height {
        for x in cell.x..cell.x + cell.width {
            frame.buffer_mut()[(x, y)]
                .set_symbol(symbol)
                .set_style(style);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(code: KeyCode) -> Event {
        Event::Key(crossterm::event::KeyEvent::new(
            code,
            crossterm::event::KeyModifiers::NONE,
        ))
    }

    fn set_current(game: &mut TetrisGame, kind: Kind, rotation: u8, col: i16, row: i16) {
        game.current = Some(Piece {
            kind,
            rotation,
            col,
            row,
        });
        game.game_over = false;
    }

    fn fill_row_except(game: &mut TetrisGame, row: usize, hole: usize) {
        for col in 0..BOARD_COLS {
            game.board[row][col] = if col == hole { None } else { Some(Kind::O) };
        }
    }

    fn cells(piece: Piece) -> Vec<(i16, i16)> {
        let mut cells: Vec<_> = piece.cells().iter().map(|c| (c.col, c.row)).collect();
        cells.sort();
        cells
    }

    fn buffer_text(backend: &ratatui::backend::TestBackend) -> String {
        let buf = backend.buffer();
        let mut out = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                out.push_str(buf[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    #[test]
    fn gravity_moves_the_piece_down_one_row() {
        let mut game = TetrisGame::new();
        let start = game.current.unwrap();

        game.tick();

        let now = game.current.unwrap();
        assert_eq!(now.row, start.row + 1);
        assert_eq!(now.col, start.col);
        assert_eq!(now.kind, start.kind);
        assert!(!game.game_over);
    }

    #[test]
    fn a_resting_piece_locks_on_the_next_tick() {
        let mut game = TetrisGame::new();
        set_current(&mut game, Kind::I, 0, 3, 19);

        game.tick();

        assert_eq!(game.board[19][3], Some(Kind::I));
        assert_eq!(game.board[19][4], Some(Kind::I));
        assert_eq!(game.board[19][5], Some(Kind::I));
        assert_eq!(game.board[19][6], Some(Kind::I));
        let spawned = game.current.unwrap();
        assert_eq!(spawned.row, 0);
        assert_eq!(spawned.col, SPAWN_COL);
    }

    #[test]
    fn completing_a_line_clears_it_and_drops_the_stack() {
        let mut game = TetrisGame::new();
        fill_row_except(&mut game, 19, 5);
        set_current(&mut game, Kind::I, 1, 3, 16);

        game.tick();

        assert_eq!(game.lines, 1);
        assert_eq!(game.score, 100);
        assert!(game.board[19][5] == Some(Kind::I));
        assert!(game.board[18][5] == Some(Kind::I));
        assert!(game.board[17][5] == Some(Kind::I));
        assert!(game.board[16][5].is_none());
        assert!(game.board[19].iter().any(Option::is_none));
        for col in 0..BOARD_COLS {
            if col != 5 {
                assert!(
                    game.board[19][col].is_none(),
                    "col {col} should have dropped"
                );
            }
        }
    }

    #[test]
    fn four_lines_score_as_a_tetris() {
        let mut game = TetrisGame::new();
        for row in 16..20 {
            fill_row_except(&mut game, row, 5);
        }
        set_current(&mut game, Kind::I, 1, 3, 16);

        game.tick();

        assert_eq!(game.lines, 4);
        assert_eq!(game.score, 800);
        assert!(game.board.iter().all(|row| row.iter().all(Option::is_none)));
    }

    #[test]
    fn ten_lines_advance_the_level() {
        let mut game = TetrisGame::new();
        game.lines = 9;
        game.level = 1;
        fill_row_except(&mut game, 19, 5);
        set_current(&mut game, Kind::I, 1, 3, 16);

        game.tick();

        assert_eq!(game.lines, 10);
        assert_eq!(game.level, 2);
        assert_eq!(game.score, 100);
    }

    #[test]
    fn cannot_move_through_walls() {
        let mut game = TetrisGame::new();
        set_current(&mut game, Kind::I, 0, 0, 5);

        assert!(!game.try_move(-1, 0));
        assert_eq!(game.current.unwrap().col, 0);

        set_current(&mut game, Kind::I, 0, 6, 5);
        assert!(!game.try_move(1, 0));
        assert_eq!(game.current.unwrap().col, 6);
        assert!(game.try_move(-1, 0));
        assert_eq!(game.current.unwrap().col, 5);
    }

    #[test]
    fn cannot_move_into_locked_cells() {
        let mut game = TetrisGame::new();
        game.board[5][4] = Some(Kind::O);
        game.board[3][6] = Some(Kind::O);
        set_current(&mut game, Kind::O, 0, 3, 3);

        assert!(!game.try_move(0, 1));
        assert_eq!(game.current.unwrap().row, 3);
        assert!(!game.try_move(1, 0));
        assert_eq!(game.current.unwrap().col, 3);
    }

    #[test]
    fn t_piece_rotates_clockwise() {
        let mut game = TetrisGame::new();
        set_current(&mut game, Kind::T, 0, 3, 4);
        let before = cells(game.current.unwrap());
        assert_eq!(before, vec![(3, 5), (4, 4), (4, 5), (5, 5)]);

        game.try_rotate(1);

        assert_eq!(
            cells(game.current.unwrap()),
            vec![(4, 4), (4, 5), (4, 6), (5, 5)]
        );
    }

    #[test]
    fn rotation_kicks_off_the_right_wall() {
        let mut game = TetrisGame::new();
        set_current(&mut game, Kind::T, 1, 8, 5);

        game.try_rotate(1);

        let piece = game.current.unwrap();
        assert!(game.fits(piece));
        assert!(piece.cells().iter().all(|c| c.col < BOARD_COLS as i16));
        assert_eq!(piece.rotation, 2);
        assert_eq!(piece.col, 7);
    }

    #[test]
    fn hard_drop_locks_at_the_bottom_and_scores_distance() {
        let mut game = TetrisGame::new();
        set_current(&mut game, Kind::I, 0, 3, 0);

        game.handle_event(press(KeyCode::Char(' ')));

        assert_eq!(game.board[19][3], Some(Kind::I));
        assert_eq!(game.board[19][6], Some(Kind::I));
        assert_eq!(game.score, 38);
        assert_eq!(game.current.unwrap().row, 0);
    }

    #[test]
    fn soft_drop_moves_one_row_and_scores() {
        let mut game = TetrisGame::new();
        set_current(&mut game, Kind::T, 0, 3, 4);

        game.handle_event(press(KeyCode::Down));

        assert_eq!(game.current.unwrap().row, 5);
        assert_eq!(game.score, 1);
        assert!(!game.game_over);
    }

    #[test]
    fn blocked_spawn_ends_the_game() {
        let mut game = TetrisGame::new();
        game.board[0] = [Some(Kind::I); BOARD_COLS];
        game.next = Kind::T;

        game.spawn_piece();

        assert!(game.game_over);
        assert_eq!(game.current.unwrap().kind, Kind::T);
    }

    #[test]
    fn seven_bag_deals_each_kind_once() {
        let mut game = TetrisGame::new();
        game.bag.clear();

        let mut drawn: Vec<_> = (0..7).map(|_| game.take_next_kind()).collect();
        drawn.sort();
        let mut expected = Kind::ALL.to_vec();
        expected.sort();
        assert_eq!(drawn, expected);
    }

    #[test]
    fn poll_timeout_is_the_time_left_until_gravity() {
        let mut game = TetrisGame::new();
        assert_eq!(game.poll_timeout(), Duration::from_millis(BASE_GRAVITY_MS));
        game.update(Duration::from_millis(40));
        assert_eq!(
            game.poll_timeout(),
            Duration::from_millis(BASE_GRAVITY_MS - 40)
        );
    }

    #[test]
    fn update_runs_every_elapsed_gravity_tick() {
        let mut game = TetrisGame::new();
        let start = game.current.unwrap();
        game.update(Duration::from_millis(BASE_GRAVITY_MS * 3 + 10));
        assert_eq!(game.current.unwrap().row, start.row + 3);
        assert_eq!(
            game.poll_timeout(),
            Duration::from_millis(BASE_GRAVITY_MS - 10)
        );
    }

    #[test]
    fn min_size_fits_the_well_and_the_side_panel() {
        let game = TetrisGame::new();
        assert_eq!(
            game.min_size(),
            (BOARD_COLS as u16 + 2 + INFO_WIDTH, BOARD_ROWS as u16 + 2)
        );
    }

    #[test]
    fn render_does_not_panic_when_the_terminal_is_too_small() {
        let backend = ratatui::backend::TestBackend::new(172, 8);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut game = TetrisGame::new();
        terminal.draw(|frame| game.render(frame)).unwrap();
    }

    #[test]
    fn render_shows_hud_at_min_playable_size() {
        let backend = ratatui::backend::TestBackend::new(30, 22);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut game = TetrisGame::new();
        terminal.draw(|frame| game.render(frame)).unwrap();
        let text = buffer_text(terminal.backend());
        assert!(text.contains("Tetris"), "{text}");
        assert!(text.contains("Score"), "{text}");
        assert!(text.contains("Next"), "{text}");
        assert!(text.contains("Level"), "{text}");
    }

    #[test]
    fn left_and_right_keys_move_the_piece() {
        let mut game = TetrisGame::new();
        set_current(&mut game, Kind::T, 0, 3, 4);

        game.handle_event(press(KeyCode::Left));
        assert_eq!(game.current.unwrap().col, 2);

        game.handle_event(press(KeyCode::Right));
        game.handle_event(press(KeyCode::Right));
        assert_eq!(game.current.unwrap().col, 4);
    }

    #[test]
    fn o_piece_does_not_change_cells_when_rotating() {
        let mut game = TetrisGame::new();
        set_current(&mut game, Kind::O, 0, 3, 4);
        let before = cells(game.current.unwrap());

        game.try_rotate(1);

        assert_eq!(cells(game.current.unwrap()), before);
    }
}
