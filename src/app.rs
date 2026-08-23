use std::io;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{DefaultTerminal, Frame, prelude::*, widgets::*};

use crate::game::Game;
use crate::menu;
use crate::registry;

pub struct App {
    exit: bool,
    state: AppState,
    term_width: u16,
    term_height: u16,
}

enum AppState {
    MainMenu { selected: usize },
    Playing {
        game: Box<dyn Game>,
        substate: PlaySubstate,
    },
}

enum Transition {
    LaunchGame(usize),
    ReturnToMenu,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PlaySubstate {
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
            AppState::Playing { game, substate } => match substate {
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
        let entries = registry::entries();
        if let Some(entry) = entries.get(index) {
            let mut game = (entry.factory)();
            game.reset();
            self.state = AppState::Playing {
                game,
                substate: PlaySubstate::Running,
            };
        }
    }

    fn return_to_menu(&mut self) {
        self.state = AppState::MainMenu { selected: 0 };
    }

    fn handle_event(&mut self, event: Event) -> io::Result<()> {
        if let Event::Resize(width, height) = event {
            self.term_width = width;
            self.term_height = height;
            return Ok(());
        }

        let Event::Key(key) = event else { return Ok(()) };
        if key.kind != KeyEventKind::Press {
            return Ok(());
        }

        if self.is_too_small() {
            if matches!(key.code, KeyCode::Char('q') | KeyCode::Char('Q')) {
                self.exit = true;
            }
            return Ok(());
        }

        let mut transition = None;

        match &mut self.state {
            AppState::MainMenu { selected } => {
                let max = registry::entries().len().saturating_sub(1);
                match key.code {
                    KeyCode::Char('q') | KeyCode::Char('Q') => self.exit = true,
                    KeyCode::Up => *selected = selected.saturating_sub(1),
                    KeyCode::Down => *selected = (*selected + 1).min(max),
                    KeyCode::Enter => transition = Some(Transition::LaunchGame(*selected)),
                    _ => {}
                }
            }
            AppState::Playing { game, substate } => {
                match key.code {
                    KeyCode::Esc if *substate == PlaySubstate::Running => {
                        *substate = PlaySubstate::Paused;
                    }
                    KeyCode::Esc if *substate == PlaySubstate::Paused => {
                        *substate = PlaySubstate::Running;
                    }
                    _ => match substate {
                        PlaySubstate::Running => game.handle_event(event),
                        PlaySubstate::Paused => match key.code {
                            KeyCode::Char('r') | KeyCode::Char('R') => {
                                *substate = PlaySubstate::Running;
                            }
                            KeyCode::Char('m') | KeyCode::Char('M') => {
                                transition = Some(Transition::ReturnToMenu);
                            }
                            _ => {}
                        },
                        PlaySubstate::GameOver => match key.code {
                            KeyCode::Char('p') | KeyCode::Char('P') => {
                                game.reset();
                                *substate = PlaySubstate::Running;
                            }
                            KeyCode::Char('m') | KeyCode::Char('M') => {
                                transition = Some(Transition::ReturnToMenu);
                            }
                            _ => {}
                        },
                    },
                }
            }
        }

        match transition {
            Some(Transition::LaunchGame(idx)) => self.launch_game(idx),
            Some(Transition::ReturnToMenu) => self.return_to_menu(),
            None => {}
        }

        Ok(())
    }

    fn update(&mut self, delta: Duration) {
        if let AppState::Playing { game, substate } = &mut self.state {
            if *substate == PlaySubstate::Running {
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
            AppState::Playing { game, substate } => {
                game.render(frame);
                match *substate {
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
        Paragraph::new(text).alignment(Alignment::Center).block(block),
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
        Paragraph::new(text).alignment(Alignment::Center).block(block),
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
