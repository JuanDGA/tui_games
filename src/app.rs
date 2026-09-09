use std::io;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{DefaultTerminal, Frame, prelude::*, widgets::*};

use crate::game::{Game, KeyMap, MatchConfig, PlayerId};
use crate::menu;
use crate::registry;

pub struct App {
    exit: bool,
    state: AppState,
    term_width: u16,
    term_height: u16,
}

enum AppState {
    MainMenu {
        selected: usize,
    },
    Playing {
        game: Box<dyn Game>,
        substate: PlaySubstate,
        menu_index: usize,
    },
}

enum Transition {
    StartGame(usize),
    BeginMatch { vs_cpu: bool },
    ReturnToMenu,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PlaySubstate {
    Ready,
    Running,
    Paused,
    GameOver,
}

impl App {
    pub fn new() -> Self {
        Self {
            exit: false,
            state: AppState::MainMenu { selected: 0 },
            term_width: 0,
            term_height: 0,
        }
    }

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        let mut last_instant = Instant::now();

        while !self.exit {
            if let Ok((width, height)) = crossterm::terminal::size() {
                self.term_width = width;
                self.term_height = height;
            }

            if event::poll(self.poll_timeout())? {
                loop {
                    self.handle_event(event::read()?)?;
                    if !event::poll(Duration::ZERO)? {
                        break;
                    }
                }
            }

            let now = Instant::now();
            let delta = now.duration_since(last_instant);
            last_instant = now;

            if !self.is_too_small() {
                self.update(delta);
            }
            terminal.draw(|frame| self.render(frame))?;
        }

        Ok(())
    }

    fn poll_timeout(&self) -> Duration {
        if self.is_too_small() {
            return Duration::from_millis(50);
        }
        match &self.state {
            AppState::MainMenu { .. } => Duration::from_millis(50),
            AppState::Playing { game, substate, .. } => match substate {
                PlaySubstate::Running => game.poll_timeout(),
                _ => Duration::from_millis(50),
            },
        }
    }

    fn min_size(&self) -> (u16, u16) {
        match &self.state {
            AppState::MainMenu { .. } => menu::MIN_SIZE,
            AppState::Playing { game, .. } => game.min_size(),
        }
    }

    fn is_too_small(&self) -> bool {
        let (min_w, min_h) = self.min_size();
        self.term_width < min_w || self.term_height < min_h
    }

    fn launch_game(&mut self, index: usize) {
        self.launch_configured(index, true);
    }

    fn start_game(&mut self, index: usize) {
        let entries = registry::entries();
        let Some(entry) = entries.get(index) else {
            return;
        };
        let mut game = (entry.factory)();
        game.reset();
        self.state = AppState::Playing {
            game,
            substate: PlaySubstate::Ready,
            menu_index: index,
        };
    }

    fn begin_from_ready(&mut self, vs_cpu: bool) {
        let AppState::Playing { game, substate, .. } = &mut self.state else {
            return;
        };
        if *substate != PlaySubstate::Ready {
            return;
        }
        if let Some(mp) = game.as_multiplayer() {
            let spec = mp.spec();
            let config = if vs_cpu {
                MatchConfig::vs_cpu(spec)
            } else {
                MatchConfig::vs_player(spec)
            };
            let _ = mp.apply_match(&config);
        }
        game.reset();
        *substate = PlaySubstate::Running;
    }

    fn launch_configured(&mut self, index: usize, vs_cpu: bool) {
        let entries = registry::entries();
        let Some(entry) = entries.get(index) else {
            return;
        };
        let game = (entry.factory)();
        self.begin_match(game, vs_cpu, index);
    }

    fn begin_match(&mut self, mut game: Box<dyn Game>, vs_cpu: bool, menu_index: usize) {
        if let Some(mp) = game.as_multiplayer() {
            let spec = mp.spec();
            let config = if vs_cpu {
                MatchConfig::vs_cpu(spec)
            } else {
                MatchConfig::vs_player(spec)
            };
            let _ = mp.apply_match(&config);
        }
        game.reset();
        self.state = AppState::Playing {
            game,
            substate: PlaySubstate::Running,
            menu_index,
        };
    }

    fn return_to_menu(&mut self) {
        let selected = match &self.state {
            AppState::Playing { menu_index, .. } => *menu_index,
            AppState::MainMenu { selected } => *selected,
        };
        self.state = AppState::MainMenu { selected };
    }

    fn handle_event(&mut self, event: Event) -> io::Result<()> {
        if let Event::Resize(width, height) = event {
            self.term_width = width;
            self.term_height = height;
            return Ok(());
        }

        let Event::Key(key) = event else {
            return Ok(());
        };
        let action = crate::kitty::is_action(key.kind);

        if self.is_too_small() {
            if action && matches!(key.code, KeyCode::Char('q') | KeyCode::Char('Q')) {
                self.exit = true;
            }
            return Ok(());
        }

        let mut transition = None;

        match &mut self.state {
            AppState::MainMenu { selected } => {
                if !action {
                    return Ok(());
                }
                let max = registry::entries().len().saturating_sub(1);
                match key.code {
                    KeyCode::Char('q') | KeyCode::Char('Q') => self.exit = true,
                    KeyCode::Up => *selected = selected.saturating_sub(1),
                    KeyCode::Down => *selected = (*selected + 1).min(max),
                    KeyCode::Enter => transition = Some(Transition::StartGame(*selected)),
                    _ => {}
                }
            }
            AppState::Playing { game, substate, .. } => match *substate {
                PlaySubstate::Ready => {
                    if !action {
                        return Ok(());
                    }
                    match key.code {
                        KeyCode::Esc | KeyCode::Char('m') | KeyCode::Char('M') => {
                            transition = Some(Transition::ReturnToMenu);
                        }
                        KeyCode::Enter | KeyCode::Char('s') | KeyCode::Char('S')
                            if !game.is_multiplayer() =>
                        {
                            transition = Some(Transition::BeginMatch { vs_cpu: true });
                        }
                        KeyCode::Char('c') | KeyCode::Char('C') | KeyCode::Enter
                            if game.is_multiplayer() =>
                        {
                            transition = Some(Transition::BeginMatch { vs_cpu: true });
                        }
                        KeyCode::Char('f') | KeyCode::Char('F') if game.is_multiplayer() => {
                            transition = Some(Transition::BeginMatch { vs_cpu: false });
                        }
                        _ => {}
                    }
                }
                PlaySubstate::Running => {
                    if action && key.code == KeyCode::Esc {
                        *substate = PlaySubstate::Paused;
                    } else {
                        crate::game::controls::dispatch(game.as_mut(), event);
                    }
                }
                PlaySubstate::Paused => {
                    // Forward releases so hold-to-move games can stop paddles
                    // even if the key is lifted while the pause overlay is up.
                    if key.kind == KeyEventKind::Release {
                        crate::game::controls::dispatch(game.as_mut(), event);
                    }
                    if !action {
                        return Ok(());
                    }
                    match key.code {
                        KeyCode::Esc | KeyCode::Char('r') | KeyCode::Char('R') => {
                            *substate = PlaySubstate::Running;
                        }
                        KeyCode::Char('m') | KeyCode::Char('M') => {
                            transition = Some(Transition::ReturnToMenu);
                        }
                        _ => {}
                    }
                }
                PlaySubstate::GameOver => {
                    if !action {
                        return Ok(());
                    }
                    match key.code {
                        KeyCode::Char('p') | KeyCode::Char('P') => {
                            game.reset();
                            *substate = PlaySubstate::Ready;
                        }
                        KeyCode::Char('m') | KeyCode::Char('M') => {
                            transition = Some(Transition::ReturnToMenu);
                        }
                        _ => {}
                    }
                }
            },
        }

        match transition {
            Some(Transition::StartGame(idx)) => self.start_game(idx),
            Some(Transition::BeginMatch { vs_cpu }) => self.begin_from_ready(vs_cpu),
            Some(Transition::ReturnToMenu) => self.return_to_menu(),
            None => {}
        }

        Ok(())
    }

    fn update(&mut self, delta: Duration) {
        if let AppState::Playing { game, substate, .. } = &mut self.state {
            if *substate == PlaySubstate::Running {
                if let Some(mp) = game.as_multiplayer() {
                    mp.tick_cpus(delta);
                }
                game.update(delta);
                if game.is_game_over() {
                    *substate = PlaySubstate::GameOver;
                }
            }
        }
    }

    fn render(&mut self, frame: &mut Frame) {
        let area = frame.area();
        let (min_w, min_h) = self.min_size();
        if area.width < min_w || area.height < min_h {
            render_too_small(frame, min_w, min_h);
            return;
        }

        match &mut self.state {
            AppState::MainMenu { selected } => {
                menu::render(frame, registry::entries(), *selected);
            }
            AppState::Playing { game, substate, .. } => {
                game.render(frame);
                match *substate {
                    PlaySubstate::Ready => {
                        let controls = game.controls();
                        render_ready_overlay(frame, game.name(), &controls, game.is_multiplayer());
                    }
                    PlaySubstate::Paused => render_pause_overlay(frame),
                    PlaySubstate::GameOver => {
                        let score = game.score();
                        render_game_over_overlay(frame, score);
                    }
                    PlaySubstate::Running => {}
                }
            }
        }
    }
}

fn render_too_small(frame: &mut Frame, min_width: u16, min_height: u16) {
    let area = frame.area();
    let width = area.width;
    let height = area.height;
    let width_style = if width < min_width {
        Style::default().fg(Color::Red)
    } else {
        Style::default().fg(Color::Green)
    };
    let height_style = if height < min_height {
        Style::default().fg(Color::Red)
    } else {
        Style::default().fg(Color::Green)
    };

    let text = Text::from(vec![
        Line::from("Terminal size too small:"),
        Line::from(vec![
            Span::raw(" Width = "),
            Span::styled(format!("{width} "), width_style),
            Span::raw("Height = "),
            Span::styled(format!("{height}"), height_style),
        ]),
        Line::from("Needed for current config:"),
        Line::from(format!("Width = {min_width} Height = {min_height}")),
    ]);

    let y = area.y + area.height.saturating_sub(4) / 2;
    let text_area = Rect {
        x: area.x,
        y,
        width: area.width,
        height: area.height.saturating_sub(y.saturating_sub(area.y)).min(4),
    };

    frame.render_widget(Clear, area);
    frame.render_widget(Paragraph::new(text).alignment(Alignment::Center), text_area);
}

fn render_ready_overlay(frame: &mut Frame, name: &str, controls: &KeyMap, multiplayer: bool) {
    let mut lines: Vec<Line> = Vec::new();
    if multiplayer {
        let a = controls.summarize(PlayerId::A);
        let b = controls.summarize(PlayerId::B);
        lines.push(Line::from(format!("A  {a}")));
        lines.push(Line::from(format!("B  {b}")));
        lines.push(Line::from(""));
        lines.push(Line::from("[C] vs CPU"));
        lines.push(Line::from("[F] vs Player"));
    } else {
        for (keys, action) in controls.describe(PlayerId::A) {
            lines.push(Line::from(format!("{keys}  {action}")));
        }
        lines.push(Line::from(""));
        lines.push(Line::from("[Enter] Start"));
    }
    lines.push(Line::from(""));
    lines.push(Line::from("[M] Main Menu"));

    let width = lines
        .iter()
        .map(|line| line.width() as u16)
        .max()
        .unwrap_or(16)
        .saturating_add(4)
        .max(24);
    let height = (lines.len() as u16).saturating_add(2).max(7);
    let area = overlay_rect(frame.area(), width, height);
    let title = Line::from(format!(" {name} ")).bold();
    let block = Block::bordered().title(title.centered());
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .alignment(Alignment::Center)
            .block(block),
        area,
    );
}

fn render_pause_overlay(frame: &mut Frame) {
    let area = overlay_rect(frame.area(), 24, 7);
    let title = Line::from(" Paused ").bold();
    let block = Block::bordered().title(title.centered());
    let text = Text::from(vec![
        Line::from("[R] Resume"),
        Line::from(""),
        Line::from("[M] Main Menu"),
    ]);
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(text)
            .alignment(Alignment::Center)
            .block(block),
        area,
    );
}

fn render_game_over_overlay(frame: &mut Frame, score: u32) {
    let area = overlay_rect(frame.area(), 24, 9);
    let title = Line::from(" Game Over ").bold();
    let block = Block::bordered().title(title.centered());
    let text = Text::from(vec![
        Line::from(format!("Score: {score}").bold()),
        Line::from(""),
        Line::from("[P] Play Again"),
        Line::from(""),
        Line::from("[M] Main Menu"),
    ]);
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(text)
            .alignment(Alignment::Center)
            .block(block),
        area,
    );
}

fn overlay_rect(frame: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(frame.width);
    let height = height.min(frame.height);
    Rect {
        x: frame.x + frame.width.saturating_sub(width) / 2,
        y: frame.y + frame.height.saturating_sub(height) / 2,
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn too_small_menu_does_not_panic() {
        let backend = TestBackend::new(20, 4);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new();
        app.term_width = 20;
        app.term_height = 4;
        terminal.draw(|frame| app.render(frame)).unwrap();
    }

    #[test]
    fn too_small_snake_does_not_panic() {
        let backend = TestBackend::new(172, 8);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new();
        app.launch_game(0);
        app.term_width = 172;
        app.term_height = 8;
        terminal.draw(|frame| app.render(frame)).unwrap();
    }

    #[test]
    fn too_small_tetris_does_not_panic() {
        let backend = TestBackend::new(172, 8);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new();
        app.launch_game(1);
        app.term_width = 172;
        app.term_height = 8;
        terminal.draw(|frame| app.render(frame)).unwrap();
    }

    #[test]
    fn too_small_pong_does_not_panic() {
        let backend = TestBackend::new(172, 8);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new();
        app.launch_game(2);
        app.term_width = 172;
        app.term_height = 8;
        terminal.draw(|frame| app.render(frame)).unwrap();
    }

    fn buffer_text(backend: &TestBackend) -> String {
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
    fn game_over_overlay_shows_both_actions_on_min_playable_size() {
        let backend = TestBackend::new(22, 22);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render_game_over_overlay(frame, 12))
            .unwrap();
        let text = buffer_text(terminal.backend());
        assert!(text.contains("Play Again"), "{text}");
        assert!(text.contains("Main Menu"), "{text}");
        assert!(text.contains("Score: 12"), "{text}");
    }

    fn key(code: KeyCode) -> Event {
        Event::Key(crossterm::event::KeyEvent::new(
            code,
            crossterm::event::KeyModifiers::NONE,
        ))
    }

    fn select_game(app: &mut App, index: usize) {
        app.term_width = 80;
        app.term_height = 24;
        for _ in 0..index {
            app.handle_event(key(KeyCode::Down)).unwrap();
        }
        app.handle_event(key(KeyCode::Enter)).unwrap();
    }

    fn playing_substate(app: &App) -> PlaySubstate {
        match app.state {
            AppState::Playing { substate, .. } => substate,
            _ => panic!("expected playing"),
        }
    }

    #[test]
    fn selecting_snake_opens_the_ready_overlay() {
        let mut app = App::new();
        select_game(&mut app, 0);
        assert_eq!(playing_substate(&app), PlaySubstate::Ready);

        let backend = TestBackend::new(42, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| app.render(frame)).unwrap();
        let text = buffer_text(terminal.backend());
        assert!(text.contains("Snake"), "{text}");
        assert!(text.contains("Start"), "{text}");
        assert!(text.contains("Main Menu"), "{text}");
        assert!(text.contains("up"), "{text}");
    }

    #[test]
    fn enter_starts_a_single_player_game() {
        let mut app = App::new();
        select_game(&mut app, 0);
        app.handle_event(key(KeyCode::Enter)).unwrap();
        assert_eq!(playing_substate(&app), PlaySubstate::Running);
    }

    #[test]
    fn selecting_pong_opens_the_ready_overlay() {
        let mut app = App::new();
        select_game(&mut app, 2);
        assert_eq!(playing_substate(&app), PlaySubstate::Ready);

        let backend = TestBackend::new(42, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| app.render(frame)).unwrap();
        let text = buffer_text(terminal.backend());
        assert!(text.contains("Pong"), "{text}");
        assert!(text.contains("vs CPU"), "{text}");
        assert!(text.contains("vs Player"), "{text}");
        assert!(text.contains("W/S"), "{text}");
        assert!(text.contains('A'), "{text}");
        assert!(text.contains('B'), "{text}");
    }

    #[test]
    fn pong_vs_cpu_keeps_a_cpu_seat() {
        let mut app = App::new();
        select_game(&mut app, 2);
        app.handle_event(key(KeyCode::Char('c'))).unwrap();
        assert_eq!(playing_substate(&app), PlaySubstate::Running);
        let AppState::Playing { game, .. } = &mut app.state else {
            panic!("expected playing");
        };
        let mp = game.as_multiplayer().unwrap();
        assert!(mp.match_config().has_cpu());
        assert_eq!(
            mp.match_config().controller(PlayerId::B),
            Some(crate::game::Controller::Cpu)
        );
    }

    #[test]
    fn pong_vs_player_uses_two_humans() {
        let mut app = App::new();
        select_game(&mut app, 2);
        app.handle_event(key(KeyCode::Char('f'))).unwrap();
        let AppState::Playing { game, .. } = &mut app.state else {
            panic!("expected playing");
        };
        let mp = game.as_multiplayer().unwrap();
        assert!(!mp.match_config().has_cpu());
        assert_eq!(
            mp.match_config().controller(PlayerId::A),
            Some(crate::game::Controller::Human)
        );
        assert_eq!(
            mp.match_config().controller(PlayerId::B),
            Some(crate::game::Controller::Human)
        );
    }

    #[test]
    fn escape_from_ready_returns_to_menu() {
        let mut app = App::new();
        select_game(&mut app, 2);
        app.handle_event(key(KeyCode::Esc)).unwrap();
        assert!(matches!(app.state, AppState::MainMenu { selected: 2 }));
    }

    #[test]
    fn ready_overlay_shows_controls_on_min_playable_size() {
        let backend = TestBackend::new(22, 22);
        let mut terminal = Terminal::new(backend).unwrap();
        let map = KeyMap::new()
            .player_a(crate::game::Action::LEFT, [KeyCode::Left])
            .player_a(crate::game::Action::RIGHT, [KeyCode::Right]);
        terminal
            .draw(|frame| render_ready_overlay(frame, "Snake", &map, false))
            .unwrap();
        let text = buffer_text(terminal.backend());
        assert!(text.contains("Snake"), "{text}");
        assert!(text.contains("Start"), "{text}");
        assert!(text.contains("Main Menu"), "{text}");
        assert!(text.contains("left"), "{text}");
    }

    #[test]
    fn pause_overlay_shows_both_actions_on_min_playable_size() {
        let backend = TestBackend::new(22, 22);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render_pause_overlay(frame)).unwrap();
        let text = buffer_text(terminal.backend());
        assert!(text.contains("Resume"), "{text}");
        assert!(text.contains("Main Menu"), "{text}");
    }
}
