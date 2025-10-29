use ratatui::layout::{Constraint, Direction, Flex, Layout, Rect};
use ratatui::prelude::{Color, Line, Style};
use ratatui::widgets::{Block, Clear, Paragraph, Wrap};
use ratatui::Frame;

pub const DEFAULT_STYLE: Style = Style::new().fg(Color::White);
pub const BLOCK_ACTIVE_STYLE: Style = Style::new().fg(Color::Blue);
pub const LIST_HIGHLIGHT_STYLE: Style = Style::new().bg(Color::Blue).fg(Color::White);

pub(crate) fn centered_area(r: Rect, hor: Constraint, vert: Constraint) -> Rect {
    let vert = cut_center(Direction::Vertical, vert).split(r);
    cut_center(Direction::Horizontal, hor).split(vert[0])[0]
}

fn cut_center(dir: Direction, constraint: Constraint) -> Layout {
    Layout::default()
        .flex(Flex::Center)
        .direction(dir)
        .constraints([constraint])
}

pub(crate) fn render_error(message: &str, frame: &mut Frame) {
    let area = centered_area(
        frame.area(),
        Constraint::Percentage(50),
        Constraint::Length(4),
    );
    let block = Block::bordered()
        .title(Line::from("Error").centered())
        .title_bottom(Line::from("| Close: Enter |"));
    let paragraph = Paragraph::new(message)
        .wrap(Wrap { trim: true })
        .block(block)
        .style(DEFAULT_STYLE);

    frame.render_widget(Clear, area);
    frame.render_widget(paragraph, area);
}
