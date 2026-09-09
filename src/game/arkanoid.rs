use std::time::Duration;

use crossterm::event::{Event, KeyCode};
use rand::Rng;
use ratatui::{Frame, prelude::*, symbols::border, widgets::Block};

use super::controls::letter;
use super::{Action, ActionPhase, Game, KeyMap, PlayerId};
use crate::grid::GridLayout;

const COURT_COLS: u16 = 39;
const COURT_ROWS: u16 = 20;
const BRICK_COLS: usize = 13;
const BRICK_ROWS: usize = 6;
const BRICK_W: f32 = 3.0;
const BRICK_H: f32 = 1.0;
const BRICK_TOP: f32 = 1.0;
const PADDLE_LEN: f32 = 5.0;
const PADDLE_SPEED: f32 = 28.0;
const BALL_SPEED: f32 = 16.0;
const BALL_SPEED_MAX: f32 = 30.0;
const HIT_SPEEDUP: f32 = 1.04;
const START_LIVES: u32 = 3;
const SERVE_PAUSE: Duration = Duration::from_millis(700);
const PHYSICS_STEP: f32 = 1.0 / 120.0;

const PADDLE_COLOR: Color = Color::LightCyan;
const BALL_COLOR: Color = Color::White;

const BRICK_COLORS: [Color; BRICK_ROWS] = [
    Color::Red,
    Color::LightRed,
    Color::Yellow,
    Color::LightGreen,
    Color::Cyan,
    Color::Magenta,
];

fn arkanoid_key_map() -> KeyMap {
    KeyMap::new()
        .player_a(Action::LEFT, letter('a'))
        .player_a(Action::LEFT, [KeyCode::Left])
        .player_a(Action::RIGHT, letter('d'))
        .player_a(Action::RIGHT, [KeyCode::Right])
}

#[derive(Debug)]
pub struct ArkanoidGame {
    paddle_x: f32,
    left_held: bool,
    right_held: bool,
    ball_x: f32,
    ball_y: f32,
    ball_vx: f32,
    ball_vy: f32,
    bricks: [[u8; BRICK_COLS]; BRICK_ROWS],
    score: u32,
    lives: u32,
    serve_delay: Duration,
    game_over: bool,
}

impl ArkanoidGame {
    pub fn new() -> Self {
        let mut game = Self {
            paddle_x: 0.0,
            left_held: false,
            right_held: false,
            ball_x: 0.0,
            ball_y: 0.0,
            ball_vx: 0.0,
            ball_vy: 0.0,
            bricks: [[0; BRICK_COLS]; BRICK_ROWS],
            score: 0,
            lives: START_LIVES,
            serve_delay: Duration::ZERO,
            game_over: false,
        };
        game.reset();
        game
    }

    fn max_paddle_x() -> f32 {
        COURT_COLS as f32 - PADDLE_LEN
    }

    fn paddle_y() -> f32 {
        COURT_ROWS as f32 - 1.0
    }

    fn center_paddle(&mut self) {
        self.paddle_x = Self::max_paddle_x() / 2.0;
    }

    fn build_wall(&mut self) {
        for row in 0..BRICK_ROWS {
            let hp = if row < 2 { 2 } else { 1 };
            self.bricks[row] = [hp; BRICK_COLS];
        }
    }

    fn bricks_left(&self) -> u32 {
        self.bricks.iter().flatten().filter(|hp| **hp > 0).count() as u32
    }

    fn brick_origin(col: usize, row: usize) -> (f32, f32) {
        (col as f32 * BRICK_W, BRICK_TOP + row as f32 * BRICK_H)
    }

    fn brick_points(row: usize) -> u32 {
        (BRICK_ROWS - row) as u32 * 50
    }

    fn sit_ball_on_paddle(&mut self) {
        self.ball_x = self.paddle_x + PADDLE_LEN / 2.0 - 0.5;
        self.ball_y = Self::paddle_y() - 1.0;
    }

    fn serve(&mut self) {
        self.sit_ball_on_paddle();
        let mut rng = rand::thread_rng();
        let angle = rng.gen_range(-0.7f32..0.7);
        self.ball_vx = BALL_SPEED * angle.sin();
        self.ball_vy = -BALL_SPEED * angle.cos();
        self.serve_delay = SERVE_PAUSE;
    }

    fn move_paddle(&mut self, dt: f32) {
        let mut dir = 0.0;
        if self.left_held {
            dir -= 1.0;
        }
        if self.right_held {
            dir += 1.0;
        }
        self.paddle_x = (self.paddle_x + dir * PADDLE_SPEED * dt).clamp(0.0, Self::max_paddle_x());
    }

    fn step_physics(&mut self, dt: f32) {
        self.ball_x += self.ball_vx * dt;
        self.ball_y += self.ball_vy * dt;

        if self.ball_x < 0.0 {
            self.ball_x = 0.0;
            self.ball_vx = self.ball_vx.abs();
        } else if self.ball_x + 1.0 > COURT_COLS as f32 {
            self.ball_x = COURT_COLS as f32 - 1.0;
            self.ball_vx = -self.ball_vx.abs();
        }

        if self.ball_y < 0.0 {
            self.ball_y = 0.0;
            self.ball_vy = self.ball_vy.abs();
        }

        self.hit_bricks();

        if self.ball_vy > 0.0 && self.overlaps_paddle() {
            self.ball_y = Self::paddle_y() - 1.0;
            self.bounce_off_paddle();
        }

        if self.ball_y > COURT_ROWS as f32 {
            self.lose_life();
        }
    }

    fn overlap_rect(a: [f32; 4], b: [f32; 4]) -> Option<(f32, f32)> {
        let [ax, ay, aw, ah] = a;
        let [bx, by, bw, bh] = b;
        let ox = (ax + aw).min(bx + bw) - ax.max(bx);
        let oy = (ay + ah).min(by + bh) - ay.max(by);
        if ox > 0.0 && oy > 0.0 {
            Some((ox, oy))
        } else {
            None
        }
    }

    fn hit_bricks(&mut self) {
        let mut best: Option<(usize, usize, f32, f32)> = None;
        for row in 0..BRICK_ROWS {
            for col in 0..BRICK_COLS {
                if self.bricks[row][col] == 0 {
                    continue;
                }
                let (bx, by) = Self::brick_origin(col, row);
                let Some((ox, oy)) = Self::overlap_rect(
                    [self.ball_x, self.ball_y, 1.0, 1.0],
                    [bx, by, BRICK_W, BRICK_H],
                ) else {
                    continue;
                };
                let better = best.is_none_or(|(_, _, px, py)| ox * oy > px * py);
                if better {
                    best = Some((col, row, ox, oy));
                }
            }
        }

        let Some((col, row, ox, oy)) = best else {
            return;
        };

        self.bricks[row][col] -= 1;
        if self.bricks[row][col] == 0 {
            self.score += Self::brick_points(row);
        }

        let (bx, by) = Self::brick_origin(col, row);
        if ox < oy {
            self.ball_vx = -self.ball_vx;
            if self.ball_x + 0.5 < bx + BRICK_W / 2.0 {
                self.ball_x = bx - 1.0;
            } else {
                self.ball_x = bx + BRICK_W;
            }
        } else {
            self.ball_vy = -self.ball_vy;
            if self.ball_y + 0.5 < by + BRICK_H / 2.0 {
                self.ball_y = by - 1.0;
            } else {
                self.ball_y = by + BRICK_H;
            }
        }
        self.speed_up();

        if self.bricks_left() == 0 {
            self.game_over = true;
        }
    }

    fn overlaps_paddle(&self) -> bool {
        Self::overlap_rect(
            [self.ball_x, self.ball_y, 1.0, 1.0],
            [self.paddle_x, Self::paddle_y(), PADDLE_LEN, 1.0],
        )
        .is_some()
    }

    fn bounce_off_paddle(&mut self) {
        let paddle_center = self.paddle_x + PADDLE_LEN / 2.0;
        let hit = (self.ball_x + 0.5) - paddle_center;
        let spin = hit / (PADDLE_LEN / 2.0);
        self.ball_vy = -self.ball_vy.abs();
        self.ball_vx += spin * BALL_SPEED * 0.5;
        self.speed_up();
        let min_vy = BALL_SPEED * 0.35;
        if self.ball_vy.abs() < min_vy {
            let sign = if self.ball_vy <= 0.0 { -1.0 } else { 1.0 };
            self.ball_vy = sign * min_vy;
            self.normalize_speed(
                (self.ball_vx * self.ball_vx + self.ball_vy * self.ball_vy)
                    .sqrt()
                    .max(BALL_SPEED),
            );
        }
    }

    fn speed_up(&mut self) {
        let speed = (self.ball_vx * self.ball_vx + self.ball_vy * self.ball_vy)
            .sqrt()
            .max(0.001)
            * HIT_SPEEDUP;
        self.normalize_speed(speed.min(BALL_SPEED_MAX));
    }

    fn normalize_speed(&mut self, speed: f32) {
        let current = (self.ball_vx * self.ball_vx + self.ball_vy * self.ball_vy)
            .sqrt()
            .max(0.001);
        let inv = speed / current;
        self.ball_vx *= inv;
        self.ball_vy *= inv;
    }

    fn lose_life(&mut self) {
        self.lives = self.lives.saturating_sub(1);
        if self.lives == 0 {
            self.game_over = true;
            return;
        }
        self.left_held = false;
        self.right_held = false;
        self.center_paddle();
        self.serve();
    }

    fn cell(pos: f32, max: u16) -> u16 {
        (pos.floor() as i32).clamp(0, max as i32 - 1) as u16
    }
}

impl Game for ArkanoidGame {
    fn name(&self) -> &'static str {
        "Arkanoid"
    }

    fn poll_timeout(&self) -> Duration {
        Duration::from_millis(16)
    }

    fn min_size(&self) -> (u16, u16) {
        (COURT_COLS + 2, COURT_ROWS + 2)
    }

    fn reset(&mut self) {
        self.score = 0;
        self.lives = START_LIVES;
        self.left_held = false;
        self.right_held = false;
        self.game_over = false;
        self.build_wall();
        self.center_paddle();
        self.serve();
    }

    fn handle_event(&mut self, event: Event) {
        if self.game_over {
            return;
        }
        let Event::Key(key) = event else {
            return;
        };
        let phase = ActionPhase::from_key_kind(key.kind);
        if let Some((player, action)) = self.controls().lookup(key.code) {
            self.handle_action(player, action, phase);
        }
    }

    fn controls(&self) -> KeyMap {
        arkanoid_key_map()
    }

    fn handle_action(&mut self, _player: PlayerId, action: Action, phase: ActionPhase) {
        if self.game_over {
            return;
        }
        let held = phase.is_start();
        match action {
            Action::LEFT => self.left_held = held,
            Action::RIGHT => self.right_held = held,
            _ => {}
        }
    }

    fn update(&mut self, delta: Duration) {
        if self.game_over {
            return;
        }

        let dt = delta.as_secs_f32();
        self.move_paddle(dt);

        if self.serve_delay > Duration::ZERO {
            self.serve_delay = self.serve_delay.saturating_sub(delta);
            self.sit_ball_on_paddle();
            return;
        }

        let mut remaining = dt;
        while remaining > 0.0 && !self.game_over && self.serve_delay == Duration::ZERO {
            let step = remaining.min(PHYSICS_STEP);
            self.step_physics(step);
            remaining -= step;
        }
    }

    fn render(&mut self, frame: &mut Frame) {
        let area = frame.area();
        let grid = GridLayout::with_border(area, COURT_COLS, COURT_ROWS);
        if !grid.fits(area) {
            return;
        }

        let title = Line::from(format!(
            " Arkanoid   {:>4}   lives {:>1} ",
            self.score, self.lives
        ))
        .bold();
        let help = Line::from(" A/D or ←/→ hold to move ");
        let block = Block::bordered()
            .title(title.centered())
            .title_bottom(help.centered())
            .border_set(border::THICK);
        frame.render_widget(&block, grid.border_rect());

        for (row, bricks) in self.bricks.iter().enumerate() {
            for (col, hp) in bricks.iter().enumerate() {
                if *hp == 0 {
                    continue;
                }
                let color = if *hp > 1 {
                    Color::Gray
                } else {
                    BRICK_COLORS[row]
                };
                let (bx, by) = Self::brick_origin(col, row);
                let start_col = Self::cell(bx, COURT_COLS);
                let start_row = Self::cell(by, COURT_ROWS);
                let width = BRICK_W.round() as u16;
                for i in 0..width {
                    fill_cell(
                        frame,
                        grid.cell_rect((start_col + i).min(COURT_COLS - 1), start_row),
                        "█",
                        Style::default().fg(color).bg(color),
                    );
                }
            }
        }

        draw_paddle(frame, &grid, self.paddle_x, Self::paddle_y());
        draw_ball(frame, &grid, self.ball_x, self.ball_y);
    }

    fn is_game_over(&self) -> bool {
        self.game_over
    }

    fn score(&self) -> u32 {
        self.score
    }
}

fn draw_paddle(frame: &mut Frame, grid: &GridLayout, x: f32, y: f32) {
    let start = ArkanoidGame::cell(x, COURT_COLS);
    let row = ArkanoidGame::cell(y, COURT_ROWS);
    let len = PADDLE_LEN.round() as u16;
    for i in 0..len {
        let col = (start + i).min(COURT_COLS - 1);
        fill_cell(
            frame,
            grid.cell_rect(col, row),
            "█",
            Style::default().fg(PADDLE_COLOR).bg(PADDLE_COLOR),
        );
    }
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

/// 2x2 quadrant glyphs. Bits: UL=1 UR=2 LL=4 LR=8.
const QUAD_GLYPHS: [&str; 16] = [
    " ", "▘", "▝", "▀", "▖", "▌", "▞", "▛", "▗", "▚", "▐", "▜", "▄", "▙", "▟", "█",
];

fn draw_ball(frame: &mut Frame, grid: &GridLayout, ball_x: f32, ball_y: f32) {
    let used = grid.used_rect().intersection(frame.area());
    if used.width == 0 || used.height == 0 {
        return;
    }
    let cell_w = used.width as f32 / COURT_COLS as f32;
    let cell_h = used.height as f32 / COURT_ROWS as f32;
    let cx = used.x as f32 + (ball_x + 0.5) * cell_w;
    let cy = used.y as f32 + (ball_y + 0.5) * cell_h;
    paint_disk(
        frame,
        used,
        cx,
        cy,
        (cell_w * 0.5).max(0.55),
        (cell_h * 0.5).max(0.55),
    );
}

fn paint_disk(frame: &mut Frame, clip: Rect, cx: f32, cy: f32, rx: f32, ry: f32) {
    let clip = clip.intersection(frame.area());
    if clip.width == 0 || clip.height == 0 {
        return;
    }

    let x0 = ((cx - rx).floor() as i32).clamp(clip.x as i32, clip.right() as i32);
    let x1 = ((cx + rx).ceil() as i32).clamp(clip.x as i32, clip.right() as i32);
    let y0 = ((cy - ry).floor() as i32).clamp(clip.y as i32, clip.bottom() as i32);
    let y1 = ((cy + ry).ceil() as i32).clamp(clip.y as i32, clip.bottom() as i32);

    let mut hits: Vec<(u16, u16, u8)> = Vec::new();
    for y in y0..y1 {
        for x in x0..x1 {
            let mut mask = 0u8;
            for (bit, (ox, oy)) in [
                (1u8, (0.25, 0.25)),
                (2, (0.75, 0.25)),
                (4, (0.25, 0.75)),
                (8, (0.75, 0.75)),
            ] {
                if inside_ellipse(x as f32 + ox, y as f32 + oy, cx, cy, rx, ry) {
                    mask |= bit;
                }
            }
            if mask != 0 {
                hits.push((x as u16, y as u16, mask));
            }
        }
    }

    let style = Style::default().fg(BALL_COLOR);
    let single_round = hits.len() == 1 && hits[0].2 == 15;
    for (x, y, mask) in hits {
        let glyph = if single_round {
            "●"
        } else {
            QUAD_GLYPHS[mask as usize]
        };
        frame.buffer_mut()[(x, y)]
            .set_symbol(glyph)
            .set_style(style);
    }
}

fn inside_ellipse(px: f32, py: f32, cx: f32, cy: f32, rx: f32, ry: f32) -> bool {
    let nx = (px - cx) / rx;
    let ny = (py - cy) / ry;
    nx * nx + ny * ny <= 1.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEventKind;
    use ratatui::{Terminal, backend::TestBackend};

    fn press(code: KeyCode) -> Event {
        Event::Key(crossterm::event::KeyEvent::new(
            code,
            crossterm::event::KeyModifiers::NONE,
        ))
    }

    fn release(code: KeyCode) -> Event {
        Event::Key(crossterm::event::KeyEvent::new_with_kind(
            code,
            crossterm::event::KeyModifiers::NONE,
            KeyEventKind::Release,
        ))
    }

    fn parked() -> ArkanoidGame {
        let mut game = ArkanoidGame::new();
        game.serve_delay = Duration::from_secs(60);
        game.ball_vx = 0.0;
        game.ball_vy = 0.0;
        game.center_paddle();
        game.sit_ball_on_paddle();
        game
    }

    fn clear_bricks(game: &mut ArkanoidGame) {
        game.bricks = [[0; BRICK_COLS]; BRICK_ROWS];
    }

    #[test]
    fn min_size_includes_border() {
        let game = ArkanoidGame::new();
        assert_eq!(game.min_size(), (COURT_COLS + 2, COURT_ROWS + 2));
    }

    #[test]
    fn hold_moves_the_paddle_and_release_stops_it() {
        let mut game = parked();
        let start = game.paddle_x;

        game.handle_event(press(KeyCode::Right));
        game.update(Duration::from_millis(80));
        assert!(
            game.paddle_x > start,
            "held right should move toward the wall"
        );

        let mid = game.paddle_x;
        game.handle_event(release(KeyCode::Right));
        game.update(Duration::from_millis(200));
        assert!(
            (game.paddle_x - mid).abs() < f32::EPSILON,
            "release should freeze the paddle, got {} vs {}",
            game.paddle_x,
            mid
        );
    }

    #[test]
    fn release_of_the_other_key_does_not_stop_the_held_key() {
        let mut game = parked();
        game.handle_event(press(KeyCode::Left));
        game.handle_event(press(KeyCode::Right));
        game.handle_event(release(KeyCode::Right));
        let x = game.paddle_x;
        game.update(Duration::from_millis(80));
        assert!(game.paddle_x < x);
    }

    #[test]
    fn press_keeps_moving_through_the_key_repeat_delay() {
        let mut game = parked();
        game.paddle_x = 0.0;
        game.handle_event(press(KeyCode::Right));
        game.update(Duration::from_millis(80));
        let early = game.paddle_x;
        assert!((early - PADDLE_SPEED * 0.08).abs() < 0.001);

        game.update(Duration::from_millis(400));
        assert!((game.paddle_x - PADDLE_SPEED * 0.48).abs() < 0.001);
        assert!(game.paddle_x > early);
    }

    #[test]
    fn letters_and_arrows_share_the_paddle() {
        let mut game = parked();
        let start = game.paddle_x;
        game.handle_event(press(KeyCode::Char('a')));
        game.update(Duration::from_millis(80));
        assert!(game.paddle_x < start);
        game.handle_event(release(KeyCode::Char('a')));

        let mid = game.paddle_x;
        game.handle_event(press(KeyCode::Char('d')));
        game.update(Duration::from_millis(80));
        assert!(game.paddle_x > mid);
        game.handle_event(release(KeyCode::Char('D')));
    }

    #[test]
    fn ball_bounces_off_the_top_wall() {
        let mut game = parked();
        clear_bricks(&mut game);
        game.serve_delay = Duration::ZERO;
        game.ball_x = 10.0;
        game.ball_y = 0.1;
        game.ball_vx = 0.0;
        game.ball_vy = -20.0;
        game.update(Duration::from_millis(50));
        assert!(game.ball_vy > 0.0);
        assert!(game.ball_y >= 0.0);
    }

    #[test]
    fn paddle_reflects_the_ball_upward() {
        let mut game = parked();
        clear_bricks(&mut game);
        game.serve_delay = Duration::ZERO;
        game.paddle_x = 10.0;
        game.ball_x = 12.0;
        game.ball_y = COURT_ROWS as f32 - 1.4;
        game.ball_vx = 0.0;
        game.ball_vy = 18.0;
        game.update(Duration::from_millis(40));
        assert!(game.ball_vy < 0.0);
        assert_eq!(game.lives, START_LIVES);
        assert!(!game.game_over);
    }

    #[test]
    fn missing_the_paddle_costs_a_life() {
        let mut game = parked();
        clear_bricks(&mut game);
        game.serve_delay = Duration::ZERO;
        game.paddle_x = 0.0;
        game.ball_x = 20.0;
        game.ball_y = COURT_ROWS as f32 + 0.2;
        game.ball_vx = 0.0;
        game.ball_vy = 20.0;
        game.update(Duration::from_millis(50));
        assert_eq!(game.lives, START_LIVES - 1);
        assert!(game.serve_delay > Duration::ZERO);
        assert!(!game.game_over);
    }

    #[test]
    fn last_life_ends_the_game() {
        let mut game = parked();
        clear_bricks(&mut game);
        game.lives = 1;
        game.serve_delay = Duration::ZERO;
        game.paddle_x = 0.0;
        game.ball_x = 20.0;
        game.ball_y = COURT_ROWS as f32 + 0.2;
        game.ball_vx = 0.0;
        game.ball_vy = 20.0;
        game.update(Duration::from_millis(50));
        assert_eq!(game.lives, 0);
        assert!(game.game_over);
    }

    #[test]
    fn hitting_a_brick_scores_and_removes_it() {
        let mut game = parked();
        clear_bricks(&mut game);
        game.bricks[5][4] = 1;
        game.serve_delay = Duration::ZERO;
        let (bx, by) = ArkanoidGame::brick_origin(4, 5);
        game.ball_x = bx + 1.0;
        game.ball_y = by + 0.8;
        game.ball_vx = 0.0;
        game.ball_vy = -20.0;
        game.update(Duration::from_millis(40));
        assert_eq!(game.bricks[5][4], 0);
        assert_eq!(game.score, ArkanoidGame::brick_points(5));
        assert!(game.ball_vy > 0.0);
    }

    #[test]
    fn silver_bricks_take_two_hits() {
        let mut game = parked();
        clear_bricks(&mut game);
        game.bricks[0][0] = 2;
        game.serve_delay = Duration::ZERO;
        let (bx, by) = ArkanoidGame::brick_origin(0, 0);
        game.ball_x = bx + 1.0;
        game.ball_y = by + BRICK_H + 0.2;
        game.ball_vx = 0.0;
        game.ball_vy = -20.0;
        game.update(Duration::from_millis(30));
        assert_eq!(game.bricks[0][0], 1);
        assert_eq!(game.score, 0);
    }

    #[test]
    fn clearing_the_wall_wins() {
        let mut game = parked();
        clear_bricks(&mut game);
        game.bricks[5][0] = 1;
        game.serve_delay = Duration::ZERO;
        let (bx, by) = ArkanoidGame::brick_origin(0, 5);
        game.ball_x = bx + 1.0;
        game.ball_y = by + 0.8;
        game.ball_vx = 0.0;
        game.ball_vy = -20.0;
        game.update(Duration::from_millis(40));
        assert_eq!(game.bricks_left(), 0);
        assert!(game.game_over);
        assert_eq!(game.score(), ArkanoidGame::brick_points(5));
    }

    #[test]
    fn new_game_starts_with_a_full_wall_and_three_lives() {
        let mut game = ArkanoidGame::new();
        assert_eq!(game.lives, START_LIVES);
        assert_eq!(game.bricks_left(), (BRICK_COLS * BRICK_ROWS) as u32);
        assert_eq!(game.bricks[0][0], 2);
        assert_eq!(game.bricks[5][0], 1);
        assert!(game.as_multiplayer().is_none());
    }

    #[test]
    fn controls_map_left_and_right() {
        let game = ArkanoidGame::new();
        let map = game.controls();
        assert_eq!(
            map.lookup(KeyCode::Char('a')),
            Some((PlayerId::A, Action::LEFT))
        );
        assert_eq!(
            map.lookup(KeyCode::Right),
            Some((PlayerId::A, Action::RIGHT))
        );
        assert_eq!(map.describe(PlayerId::A)[0].1, "left");
    }

    #[test]
    fn poll_timeout_is_frame_sized() {
        assert_eq!(
            ArkanoidGame::new().poll_timeout(),
            Duration::from_millis(16)
        );
    }

    #[test]
    fn render_does_not_panic_when_the_terminal_is_too_small() {
        let backend = TestBackend::new(10, 4);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut game = ArkanoidGame::new();
        terminal.draw(|frame| game.render(frame)).unwrap();
    }

    #[test]
    fn render_shows_score_and_lives_at_min_playable_size() {
        let (w, h) = ArkanoidGame::new().min_size();
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut game = ArkanoidGame::new();
        game.score = 150;
        game.lives = 2;
        terminal.draw(|frame| game.render(frame)).unwrap();

        let buf = terminal.backend().buffer();
        let mut text = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                text.push_str(buf[(x, y)].symbol());
            }
        }
        assert!(text.contains("150"), "{text}");
        assert!(text.contains('2'), "{text}");
        assert!(text.contains("Arkanoid"), "{text}");
        assert!(
            text.contains('●') || text.contains('█') || text.contains('▄') || text.contains('▀'),
            "{text}"
        );
    }

    #[test]
    fn serve_keeps_the_ball_on_the_paddle() {
        let mut game = parked();
        game.paddle_x = 4.0;
        game.update(Duration::from_millis(50));
        assert!((game.ball_x - (4.0 + PADDLE_LEN / 2.0 - 0.5)).abs() < 0.001);
        assert!((game.ball_y - (ArkanoidGame::paddle_y() - 1.0)).abs() < 0.001);
    }
}
