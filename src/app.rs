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
        }
    }

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        let mut last_instant = Instant::now();

        while !self.exit {
            let timeout = self.poll_timeout();

            if event::poll(timeout)? {
                let event = event::read()?;
                self.handle_event(event)?;
            }

            let now = Instant::now();
            let delta = now.duration_since(last_instant);
            last_instant = now;

            self.update(delta);
            terminal.draw(|frame| self.render(frame))?;
        }

        Ok(())
    }

    fn poll_timeout(&self) -> Duration {
        match &self.state {
            AppState::MainMenu { .. } => Duration::from_millis(50),
            AppState::Playing { game, .. } => game.poll_timeout(),
        }
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
        let Event::Key(key) = event else { return Ok(()) };
        if key.kind != KeyEventKind::Press {
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

fn render_pause_overlay(frame: &mut Frame) {
    let area = centered_rect(30, 20, frame.area());
    let title = Line::from(" Paused ").bold();
    let block = Block::bordered()
        .title(title.centered());

    let text = Text::from(vec![
        Line::from(""),
        Line::from("[R] Resume"),
        Line::from(""),
        Line::from("[M] Main Menu"),
        Line::from(""),
    ]);

    let paragraph = Paragraph::new(text).alignment(Alignment::Center).block(block);
    frame.render_widget(Clear, area);
    frame.render_widget(paragraph, area);
}

fn render_game_over_overlay(frame: &mut Frame, score: u32) {
    let area = centered_rect(30, 20, frame.area());
    let title = Line::from(" Game Over ").bold();
    let block = Block::bordered()
        .title(title.centered());

    let text = Text::from(vec![
        Line::from(""),
        Line::from(format!("Score: {}", score).bold()),
        Line::from(""),
        Line::from("[P] Play Again"),
        Line::from(""),
        Line::from("[M] Main Menu"),
        Line::from(""),
    ]);

    let paragraph = Paragraph::new(text).alignment(Alignment::Center).block(block);
    frame.render_widget(Clear, area);
    frame.render_widget(paragraph, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
