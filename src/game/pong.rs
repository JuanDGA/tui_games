use std::time::Duration;

use crossterm::event::{Event, KeyCode, KeyEventKind};
use rand::Rng;
use ratatui::{Frame, prelude::*, symbols::border, widgets::Block};

use super::controls::letter;
use super::{
    Action, ActionPhase, Controller, Game, KeyMap, MatchConfig, MatchError, Multiplayer,
    MultiplayerSpec, PlayerId, Seat,
};
use crate::grid::GridLayout;

const COURT_COLS: u16 = 39;
const COURT_ROWS: u16 = 17;
const PADDLE_LEN: f32 = 4.0;
const PADDLE_SPEED: f32 = 22.0;
const AI_SPEED: f32 = 14.0;
const BALL_SPEED: f32 = 16.0;
const BALL_SPEED_MAX: f32 = 32.0;
const HIT_SPEEDUP: f32 = 1.06;
const WIN_SCORE: u32 = 11;
const SERVE_PAUSE: Duration = Duration::from_millis(700);
const PHYSICS_STEP: f32 = 1.0 / 120.0;

const LEFT: usize = 0;
const RIGHT: usize = 1;
const LEFT_ID: PlayerId = PlayerId::A;
const RIGHT_ID: PlayerId = PlayerId::B;

fn pong_key_map() -> KeyMap {
    KeyMap::new()
        .player_a(Action::UP, letter('w'))
        .player_a(Action::DOWN, letter('s'))
        .player_b(Action::UP, [KeyCode::Up])
        .player_b(Action::DOWN, [KeyCode::Down])
}

const PLAYER_COLOR: Color = Color::LightCyan;
const AI_COLOR: Color = Color::LightYellow;
const BALL_COLOR: Color = Color::White;
const NET_COLOR: Color = Color::DarkGray;

#[derive(Debug)]
struct Paddle {
    y: f32,
    up_held: bool,
    down_held: bool,
    controller: Controller,
}

#[derive(Debug)]
pub struct PongGame {
    paddles: [Paddle; 2],
    ball_x: f32,
    ball_y: f32,
    ball_vx: f32,
    ball_vy: f32,
    scores: [u32; 2],
    serve_delay: Duration,
    serve_toward_left: bool,
    match_config: MatchConfig,
    game_over: bool,
}

impl PongGame {
    pub fn new() -> Self {
        let spec = MultiplayerSpec::TWO_PLAYER;
        let match_config = MatchConfig::vs_cpu(spec);
        let mut game = Self {
            paddles: [
                Paddle {
                    y: 0.0,
                    up_held: false,
                    down_held: false,
                    controller: Controller::Human,
                },
                Paddle {
                    y: 0.0,
                    up_held: false,
                    down_held: false,
                    controller: Controller::Cpu,
                },
            ],
            ball_x: 0.0,
            ball_y: 0.0,
            ball_vx: 0.0,
            ball_vy: 0.0,
            scores: [0, 0],
            serve_delay: Duration::ZERO,
            serve_toward_left: false,
            match_config,
            game_over: false,
        };
        game.sync_controllers();
        game.reset();
        game
    }

    fn max_paddle_y() -> f32 {
        COURT_ROWS as f32 - PADDLE_LEN
    }

    fn center_paddles(&mut self) {
        let y = (COURT_ROWS as f32 - PADDLE_LEN) / 2.0;
        self.paddles[LEFT].y = y;
        self.paddles[RIGHT].y = y;
    }

    fn serve(&mut self) {
        self.ball_x = COURT_COLS as f32 / 2.0;
        self.ball_y = COURT_ROWS as f32 / 2.0 - 0.5;
        let mut rng = rand::thread_rng();
        let angle = rng.gen_range(-0.55f32..0.55);
        let dir_x = if self.serve_toward_left { -1.0 } else { 1.0 };
        self.ball_vx = dir_x * BALL_SPEED * angle.cos();
        self.ball_vy = BALL_SPEED * angle.sin();
        self.serve_delay = SERVE_PAUSE;
    }

    fn sync_controllers(&mut self) {
        let seats: Vec<Seat> = self.match_config.seats().to_vec();
        for seat in seats {
            if let Some(paddle) = self.paddle_mut(seat.id) {
                paddle.controller = seat.controller;
                paddle.up_held = false;
                paddle.down_held = false;
            }
        }
    }

    fn paddle_index(id: PlayerId) -> Option<usize> {
        match id {
            LEFT_ID => Some(LEFT),
            RIGHT_ID => Some(RIGHT),
            _ => None,
        }
    }

    fn paddle_mut(&mut self, id: PlayerId) -> Option<&mut Paddle> {
        Self::paddle_index(id).map(|i| &mut self.paddles[i])
    }

    fn human_count(&self) -> usize {
        self.paddles
            .iter()
            .filter(|paddle| paddle.controller == Controller::Human)
            .count()
    }

    fn set_hold(&mut self, side: usize, up: bool, held: bool) {
        let paddle = &mut self.paddles[side];
        if up {
            paddle.up_held = held;
        } else {
            paddle.down_held = held;
        }
    }

    fn move_humans(&mut self, dt: f32) {
        for paddle in &mut self.paddles {
            if paddle.controller != Controller::Human {
                continue;
            }
            Self::nudge_paddle(paddle, PADDLE_SPEED, dt);
        }
    }

    fn nudge_paddle(paddle: &mut Paddle, speed: f32, dt: f32) {
        let mut dir = 0.0;
        if paddle.up_held {
            dir -= 1.0;
        }
        if paddle.down_held {
            dir += 1.0;
        }
        paddle.y = (paddle.y + dir * speed * dt).clamp(0.0, Self::max_paddle_y());
    }

    fn steer_cpu(&mut self, side: usize, dt: f32) {
        let going_toward = if side == LEFT {
            self.ball_vx < 0.0
        } else {
            self.ball_vx > 0.0
        };
        let target = if going_toward {
            self.ball_y + 0.5
        } else {
            COURT_ROWS as f32 / 2.0
        };
        let paddle = &mut self.paddles[side];
        let paddle_center = paddle.y + PADDLE_LEN / 2.0;
        let max = AI_SPEED * dt;
        let delta = (target - paddle_center).clamp(-max, max);
        paddle.y = (paddle.y + delta).clamp(0.0, Self::max_paddle_y());
    }

    fn step_physics(&mut self, dt: f32) {
        self.ball_x += self.ball_vx * dt;
        self.ball_y += self.ball_vy * dt;

        if self.ball_y < 0.0 {
            self.ball_y = 0.0;
            self.ball_vy = self.ball_vy.abs();
        } else if self.ball_y + 1.0 > COURT_ROWS as f32 {
            self.ball_y = COURT_ROWS as f32 - 1.0;
            self.ball_vy = -self.ball_vy.abs();
        }

        if self.ball_vx < 0.0 && self.overlaps_paddle(self.paddles[LEFT].y, 0.0) {
            self.ball_x = 1.0;
            self.bounce_off_paddle(self.paddles[LEFT].y);
        } else if self.ball_vx > 0.0
            && self.overlaps_paddle(self.paddles[RIGHT].y, (COURT_COLS - 1) as f32)
        {
            self.ball_x = (COURT_COLS - 2) as f32;
            self.bounce_off_paddle(self.paddles[RIGHT].y);
        }

        if self.ball_x + 1.0 < 0.0 {
            self.scores[RIGHT] += 1;
            self.after_point(true);
        } else if self.ball_x > COURT_COLS as f32 {
            self.scores[LEFT] += 1;
            self.after_point(false);
        }
    }

    fn overlaps_paddle(&self, paddle_y: f32, paddle_x: f32) -> bool {
        let ball_right = self.ball_x + 1.0;
        let ball_bottom = self.ball_y + 1.0;
        ball_right > paddle_x
            && self.ball_x < paddle_x + 1.0
            && ball_bottom > paddle_y
            && self.ball_y < paddle_y + PADDLE_LEN
    }

    fn bounce_off_paddle(&mut self, paddle_y: f32) {
        let paddle_center = paddle_y + PADDLE_LEN / 2.0;
        let hit = (self.ball_y + 0.5) - paddle_center;
        let spin = hit / (PADDLE_LEN / 2.0);
        self.ball_vx = -self.ball_vx;
        self.ball_vy += spin * BALL_SPEED * 0.45;
        let speed = (self.ball_vx * self.ball_vx + self.ball_vy * self.ball_vy)
            .sqrt()
            .max(0.001)
            * HIT_SPEEDUP;
        let speed = speed.min(BALL_SPEED_MAX);
        let inv = speed / (self.ball_vx * self.ball_vx + self.ball_vy * self.ball_vy).sqrt();
        self.ball_vx *= inv;
        self.ball_vy *= inv;
    }

    fn after_point(&mut self, toward_left: bool) {
        if self.scores[LEFT] >= WIN_SCORE || self.scores[RIGHT] >= WIN_SCORE {
            self.game_over = true;
            return;
        }
        self.serve_toward_left = toward_left;
        self.center_paddles();
        self.serve();
    }

    fn cell(pos: f32, max: u16) -> u16 {
        (pos.floor() as i32).clamp(0, max as i32 - 1) as u16
    }

    fn help_text(&self) -> &'static str {
        match self.human_count() {
            0 => " CPU vs CPU ",
            1 => " W/S or ↑/↓ hold to move ",
            _ => " W/S A   ↑/↓ B ",
        }
    }
}

impl Game for PongGame {
    fn name(&self) -> &'static str {
        "Pong"
    }

    fn poll_timeout(&self) -> Duration {
        Duration::from_millis(16)
    }

    fn min_size(&self) -> (u16, u16) {
        (COURT_COLS + 2, COURT_ROWS + 2)
    }

    fn reset(&mut self) {
        self.scores = [0, 0];
        for paddle in &mut self.paddles {
            paddle.up_held = false;
            paddle.down_held = false;
        }
        self.game_over = false;
        self.serve_toward_left = false;
        self.center_paddles();
        self.serve();
    }

    fn handle_event(&mut self, event: Event) {
        if self.game_over {
            return;
        }
        let Event::Key(key) = event else { return };
        let phase = ActionPhase::from_key_kind(key.kind);
        if let Some((player, action)) = self.controls().resolve(key.code, Some(&self.match_config))
        {
            self.handle_action(player, action, phase);
        }
    }

    fn controls(&self) -> KeyMap {
        pong_key_map()
    }

    fn handle_action(&mut self, player: PlayerId, action: Action, phase: ActionPhase) {
        if self.game_over {
            return;
        }
        if self.match_config.controller(player) != Some(Controller::Human) {
            return;
        }
        let Some(side) = Self::paddle_index(player) else {
            return;
        };
        let up = if action == Action::UP {
            true
        } else if action == Action::DOWN {
            false
        } else {
            return;
        };
        self.set_hold(side, up, phase.is_start());
    }

    fn is_multiplayer(&self) -> bool {
        true
    }

    fn as_multiplayer(&mut self) -> Option<&mut dyn Multiplayer> {
        Some(self)
    }

    fn update(&mut self, delta: Duration) {
        if self.game_over {
            return;
        }

        let dt = delta.as_secs_f32();
        self.move_humans(dt);

        if self.serve_delay > Duration::ZERO {
            self.serve_delay = self.serve_delay.saturating_sub(delta);
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

        let two_human = self.human_count() > 1;
        let title = Line::from(format!(
            " {} {:>2}   Pong   {:>2} {} ",
            side_label(LEFT, self.paddles[LEFT].controller, two_human),
            self.scores[LEFT],
            self.scores[RIGHT],
            side_label(RIGHT, self.paddles[RIGHT].controller, two_human),
        ))
        .bold();
        let help = Line::from(self.help_text());
        let block = Block::bordered()
            .title(title.centered())
            .title_bottom(help.centered())
            .border_set(border::THICK);
        frame.render_widget(&block, grid.border_rect());

        let net_col = COURT_COLS / 2;
        for row in (0..COURT_ROWS).step_by(2) {
            fill_cell(
                frame,
                grid.cell_rect(net_col, row),
                "│",
                Style::default().fg(NET_COLOR),
            );
        }

        draw_paddle(frame, &grid, 0, self.paddles[LEFT].y, PLAYER_COLOR);
        draw_paddle(
            frame,
            &grid,
            COURT_COLS - 1,
            self.paddles[RIGHT].y,
            AI_COLOR,
        );
        draw_ball(frame, &grid, self.ball_x, self.ball_y);
    }

    fn is_game_over(&self) -> bool {
        self.game_over
    }

    fn score(&self) -> u32 {
        self.scores[LEFT]
    }
}

impl Multiplayer for PongGame {
    fn spec(&self) -> MultiplayerSpec {
        MultiplayerSpec::TWO_PLAYER
    }

    fn match_config(&self) -> &MatchConfig {
        &self.match_config
    }

    fn apply_match(&mut self, config: &MatchConfig) -> Result<(), MatchError> {
        if config.player_count() != 2
            || config.controller(LEFT_ID).is_none()
            || config.controller(RIGHT_ID).is_none()
        {
            return Err(MatchError::WrongPlayers);
        }
        self.match_config = config.clone();
        self.sync_controllers();
        Ok(())
    }

    fn control_cpu(&mut self, player: PlayerId, delta: Duration) {
        let Some(side) = Self::paddle_index(player) else {
            return;
        };
        if self.paddles[side].controller != Controller::Cpu {
            return;
        }
        self.steer_cpu(side, delta.as_secs_f32());
    }
}

fn side_label(side: usize, controller: Controller, two_human: bool) -> &'static str {
    match controller {
        Controller::Cpu => "CPU",
        Controller::Human if two_human && side == LEFT => "A",
        Controller::Human if two_human => "B",
        Controller::Human => "You",
    }
}

fn draw_paddle(frame: &mut Frame, grid: &GridLayout, col: u16, y: f32, color: Color) {
    let start = PongGame::cell(y, COURT_ROWS);
    let len = PADDLE_LEN.round() as u16;
    for i in 0..len {
        let row = (start + i).min(COURT_ROWS - 1);
        fill_cell(
            frame,
            grid.cell_rect(col, row),
            "█",
            Style::default().fg(color).bg(color),
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

    fn parked() -> PongGame {
        let mut game = PongGame::new();
        game.serve_delay = Duration::from_secs(60);
        game.ball_vx = 0.0;
        game.ball_vy = 0.0;
        game.center_paddles();
        game
    }

    #[test]
    fn min_size_includes_border() {
        let game = PongGame::new();
        assert_eq!(game.min_size(), (COURT_COLS + 2, COURT_ROWS + 2));
    }

    #[test]
    fn hold_moves_the_paddle_and_release_stops_it() {
        let mut game = parked();
        let start = game.paddles[LEFT].y;

        game.handle_event(press(KeyCode::Up));
        game.update(Duration::from_millis(80));
        assert!(
            game.paddles[LEFT].y < start,
            "held up should move toward row 0"
        );

        let mid = game.paddles[LEFT].y;
        game.handle_event(release(KeyCode::Up));
        game.update(Duration::from_millis(200));
        assert!(
            (game.paddles[LEFT].y - mid).abs() < f32::EPSILON,
            "release should freeze the paddle, got {} vs {}",
            game.paddles[LEFT].y,
            mid
        );
    }

    #[test]
    fn release_of_the_other_key_does_not_stop_the_held_key() {
        let mut game = parked();
        game.handle_event(press(KeyCode::Down));
        game.handle_event(press(KeyCode::Up));
        game.handle_event(release(KeyCode::Up));
        let y = game.paddles[LEFT].y;
        game.update(Duration::from_millis(80));
        assert!(game.paddles[LEFT].y > y);
    }

    #[test]
    fn press_keeps_moving_through_the_key_repeat_delay() {
        let mut game = parked();
        game.paddles[LEFT].y = 0.0;
        game.handle_event(press(KeyCode::Down));
        game.update(Duration::from_millis(80));
        let early = game.paddles[LEFT].y;
        assert!((early - PADDLE_SPEED * 0.08).abs() < 0.001);

        // OS key-repeat often waits ~300-500ms after Press before Repeat.
        // Motion must not pause in that gap; Release is what stops the paddle.
        game.update(Duration::from_millis(400));
        assert!((game.paddles[LEFT].y - PADDLE_SPEED * 0.48).abs() < 0.001);
        assert!(game.paddles[LEFT].y > early);
    }

    #[test]
    fn wasd_and_arrows_share_the_player_paddle() {
        let mut game = parked();
        let start = game.paddles[LEFT].y;
        game.handle_event(press(KeyCode::Char('w')));
        game.update(Duration::from_millis(80));
        assert!(game.paddles[LEFT].y < start);
        game.handle_event(release(KeyCode::Char('w')));

        let mid = game.paddles[LEFT].y;
        game.handle_event(press(KeyCode::Char('s')));
        game.update(Duration::from_millis(80));
        assert!(game.paddles[LEFT].y > mid);
        game.handle_event(release(KeyCode::Char('S')));
    }

    #[test]
    fn ball_bounces_off_the_top_wall() {
        let mut game = parked();
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
    fn player_paddle_reflects_the_ball() {
        let mut game = parked();
        game.serve_delay = Duration::ZERO;
        game.paddles[LEFT].y = 6.0;
        game.ball_x = 0.6;
        game.ball_y = 7.0;
        game.ball_vx = -18.0;
        game.ball_vy = 0.0;
        game.update(Duration::from_millis(40));
        assert!(game.ball_vx > 0.0);
        assert_eq!(game.scores[RIGHT], 0);
        assert_eq!(game.scores[LEFT], 0);
    }

    #[test]
    fn missing_the_player_paddle_scores_for_cpu() {
        let mut game = parked();
        game.serve_delay = Duration::ZERO;
        game.paddles[LEFT].y = 0.0;
        game.ball_x = -0.2;
        game.ball_y = 12.0;
        game.ball_vx = -20.0;
        game.ball_vy = 0.0;
        game.update(Duration::from_millis(50));
        assert_eq!(game.scores[RIGHT], 1);
        assert!(game.serve_delay > Duration::ZERO);
        assert!(!game.game_over);
    }

    #[test]
    fn reaching_eleven_ends_the_match() {
        let mut game = parked();
        game.scores[LEFT] = 10;
        game.serve_delay = Duration::ZERO;
        game.paddles[RIGHT].y = 0.0;
        game.ball_x = COURT_COLS as f32 + 0.2;
        game.ball_y = 12.0;
        game.ball_vx = 20.0;
        game.ball_vy = 0.0;
        game.update(Duration::from_millis(50));
        assert_eq!(game.scores[LEFT], 11);
        assert!(game.game_over);
        assert_eq!(game.score(), 11);
    }

    #[test]
    fn ai_tracks_a_ball_coming_toward_it() {
        let mut game = parked();
        game.paddles[RIGHT].y = 0.0;
        game.ball_x = 20.0;
        game.ball_y = 12.0;
        game.ball_vx = 10.0;
        game.tick_cpus(Duration::from_millis(200));
        game.update(Duration::from_millis(200));
        assert!(game.paddles[RIGHT].y > 0.0);
    }

    #[test]
    fn controls_map_actions_to_player_a_and_b() {
        let game = PongGame::new();
        let map = game.controls();
        assert_eq!(
            map.lookup(KeyCode::Char('w')),
            Some((PlayerId::A, Action::UP))
        );
        assert_eq!(map.lookup(KeyCode::Down), Some((PlayerId::B, Action::DOWN)));
        assert_eq!(map.summarize(PlayerId::A), "W/S");
        assert_eq!(map.summarize(PlayerId::B), "↑/↓");
    }

    #[test]
    fn new_game_defaults_to_human_versus_cpu() {
        let game = PongGame::new();
        assert!(game.match_config().has_cpu());
        assert_eq!(
            game.match_config().controller(LEFT_ID),
            Some(Controller::Human)
        );
        assert_eq!(
            game.match_config().controller(RIGHT_ID),
            Some(Controller::Cpu)
        );
    }

    #[test]
    fn two_humans_split_wasd_and_arrows() {
        let mut game = parked();
        let config = MatchConfig::new(
            MultiplayerSpec::TWO_PLAYER,
            vec![Seat::human(0), Seat::human(1)],
        )
        .unwrap();
        game.apply_match(&config).unwrap();

        let left = game.paddles[LEFT].y;
        let right = game.paddles[RIGHT].y;
        game.handle_event(press(KeyCode::Char('w')));
        game.update(Duration::from_millis(80));
        assert!(game.paddles[LEFT].y < left);
        assert!((game.paddles[RIGHT].y - right).abs() < f32::EPSILON);

        game.handle_event(release(KeyCode::Char('w')));
        let left = game.paddles[LEFT].y;
        let right = game.paddles[RIGHT].y;
        game.handle_event(press(KeyCode::Up));
        game.update(Duration::from_millis(80));
        assert!((game.paddles[LEFT].y - left).abs() < f32::EPSILON);
        assert!(game.paddles[RIGHT].y < right);
    }

    #[test]
    fn apply_match_rejects_unknown_seat_ids() {
        let mut game = PongGame::new();
        let config = MatchConfig::new(
            MultiplayerSpec::TWO_PLAYER,
            vec![Seat::human(0), Seat::cpu(2)],
        )
        .unwrap();
        assert_eq!(game.apply_match(&config), Err(MatchError::WrongPlayers));
    }

    #[test]
    fn control_cpu_does_not_move_a_human_seat() {
        let mut game = parked();
        game.paddles[LEFT].y = 0.0;
        game.control_cpu(LEFT_ID, Duration::from_millis(200));
        assert_eq!(game.paddles[LEFT].y, 0.0);
    }

    #[test]
    fn poll_timeout_is_frame_sized() {
        assert_eq!(PongGame::new().poll_timeout(), Duration::from_millis(16));
    }

    #[test]
    fn render_does_not_panic_when_the_terminal_is_too_small() {
        let backend = TestBackend::new(10, 4);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut game = PongGame::new();
        terminal.draw(|frame| game.render(frame)).unwrap();
    }

    #[test]
    fn render_shows_both_scores_at_min_playable_size() {
        let (w, h) = PongGame::new().min_size();
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut game = PongGame::new();
        game.scores[LEFT] = 4;
        game.scores[RIGHT] = 7;
        terminal.draw(|frame| game.render(frame)).unwrap();

        let buf = terminal.backend().buffer();
        let mut text = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                text.push_str(buf[(x, y)].symbol());
            }
        }
        assert!(text.contains("4"), "{text}");
        assert!(text.contains("7"), "{text}");
        assert!(text.contains("Pong"), "{text}");
        assert!(
            text.contains('●') || text.contains('█') || text.contains('▄') || text.contains('▀'),
            "{text}"
        );
    }

    #[test]
    fn large_ball_is_a_disk_not_a_filled_rectangle() {
        let backend = TestBackend::new(8, 4);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                paint_disk(frame, frame.area(), 4.0, 2.0, 3.0, 1.5);
            })
            .unwrap();
        let buf = terminal.backend().buffer();
        let corner = buf[(0, 0)].symbol();
        let center = buf[(4, 2)].symbol();
        assert_ne!(corner, "█", "circle should leave the cell corners empty");
        assert_eq!(center, "█");
    }

    #[test]
    fn fractional_row_uses_half_blocks_instead_of_jumping() {
        let backend = TestBackend::new(6, 4);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                // Centered on the boundary between rows 1 and 2.
                paint_disk(frame, frame.area(), 3.0, 2.0, 1.2, 0.7);
            })
            .unwrap();
        let buf = terminal.backend().buffer();
        let mut glyphs = String::new();
        for y in 0..4 {
            for x in 0..6 {
                glyphs.push_str(buf[(x, y)].symbol());
            }
        }
        assert!(
            glyphs.contains('▄')
                || glyphs.contains('▀')
                || glyphs.contains('▌')
                || glyphs.contains('▐'),
            "expected a straddling half-block, got {glyphs:?}"
        );
    }
}
