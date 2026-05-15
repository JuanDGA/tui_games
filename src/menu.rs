use ratatui::{Frame, prelude::*, widgets::*};

use crate::registry::GameEntry;

pub fn render(frame: &mut Frame, registry: &[GameEntry], selected: usize) {
    let area = frame.area();
    let items: Vec<ListItem> = registry.iter().enumerate().map(|(i, entry)| {
        let style = if i == selected {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        ListItem::new(entry.name).style(style)
    }).collect();

    let title = Line::from(" TUI Games ").bold();
    let footer = Line::from("↑/↓ Navigate  |  Enter Select  |  Q Quit");

    let list = List::new(items)
        .block(
            Block::bordered()
                .title(title.centered())
                .title_bottom(footer.centered()),
        );

    frame.render_widget(list, area);
}
