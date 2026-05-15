use std::time::Duration;

use crossterm::event::{Event, KeyCode, KeyEventKind};
use ratatui::{Frame, prelude::*, symbols::border, widgets::{Block, Paragraph}};

use super::Game;

#[derive(Debug)]
pub struct DemoGame {
    head: Square,
    score: u32,
    game_over: bool,
    inner: Rect,
}

#[derive(Debug, Default)]
struct Square {
    x: u16,
    y: u16,
}

impl DemoGame {
    pub fn new() -> Self {
        Self {
            head: Square::default(),
            score: 0,
            game_over: false,
            inner: Rect::default(),
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

    fn reset(&mut self) {
        self.head = Square::default();
        self.score = 0;
        self.game_over = false;
        self.inner = Rect::default();
    }

    fn handle_event(&mut self, event: Event) {
        let Event::Key(key) = event else { return };
        if key.kind != KeyEventKind::Press {
            return;
        }

        let max_x = self.inner.width.saturating_sub(1);
        let max_y = self.inner.height.saturating_sub(1);

        match key.code {
            KeyCode::Left if self.head.x > 0 => self.head.x -= 1,
            KeyCode::Right if self.inner.width > 0 && self.head.x < max_x => self.head.x += 1,
            KeyCode::Up if self.head.y > 0 => self.head.y -= 1,
            KeyCode::Down if self.inner.height > 0 && self.head.y < max_y => self.head.y += 1,
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
        self.inner = block.inner(area);

        let span = Span::default().content("□");
        frame.buffer_mut().set_span(
            self.inner.x + self.head.x,
            self.inner.y + self.head.y,
            &span,
            span.width() as u16,
        );

        let score_text = Text::from(format!("Score: {}", self.score));
        let score_widget = Paragraph::new(score_text);
        let score_area = Rect {
            x: self.inner.x,
            y: self.inner.y + self.inner.height.saturating_sub(1),
            width: self.inner.width,
            height: 1,
        };
        frame.render_widget(score_widget, score_area);
    }

    fn is_game_over(&self) -> bool {
        self.game_over
    }

    fn score(&self) -> u32 {
        self.score
    }
}
