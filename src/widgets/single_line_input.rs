use crate::utils::DEFAULT_STYLE;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::{Modifier, Widget};
use tui_textarea::{CursorMove, TextArea};

#[derive(Clone)]
pub struct SingleLineInput<'a> {
    textarea: TextArea<'a>,
}

impl SingleLineInput<'_> {
    pub fn new(default_text: String, hidden: bool, active: bool) -> Self {
        let mut input = Self {
            textarea: TextArea::new(vec![default_text]),
        };

        input.textarea.set_cursor_line_style(DEFAULT_STYLE);
        input.textarea.move_cursor(CursorMove::End);

        if active {
            input.activate_input()
        } else {
            input.deactivate_input()
        }

        if hidden {
            input.textarea.set_mask_char('*');
        }

        input
    }

    pub fn deactivate_input(&mut self) {
        self.textarea.set_cursor_style(DEFAULT_STYLE);
    }

    pub fn activate_input(&mut self) {
        self.textarea
            .set_cursor_style(DEFAULT_STYLE.add_modifier(Modifier::REVERSED));
    }

    pub fn text(&self) -> &String {
        &self.textarea.lines()[0]
    }

    pub fn handle_text_input(&mut self, input: KeyEvent) -> bool {
        match (input.modifiers, input.code) {
            (KeyModifiers::CONTROL, KeyCode::Left) => {
                self.textarea.move_cursor(CursorMove::WordBack);
                true
            }
            (KeyModifiers::CONTROL, KeyCode::Right) => {
                self.textarea.move_cursor(CursorMove::WordForward);
                true
            }
            (_, char) => match char {
                KeyCode::Backspace => self.textarea.delete_char(),
                KeyCode::Left => {
                    self.textarea.move_cursor(CursorMove::Back);
                    true
                }
                KeyCode::Right => {
                    self.textarea.move_cursor(CursorMove::Forward);
                    true
                }
                KeyCode::Delete => self.textarea.delete_next_char(),
                KeyCode::Char(char) => {
                    self.textarea.insert_char(char);
                    true
                }
                _ => false,
            },
        }
    }
}

impl Default for SingleLineInput<'_> {
    fn default() -> Self {
        let mut input = Self {
            textarea: Default::default(),
        };

        input.textarea.set_cursor_style(DEFAULT_STYLE);
        input.textarea.set_cursor_line_style(DEFAULT_STYLE);
        input.textarea.move_cursor(CursorMove::End);

        input
    }
}

impl Widget for &SingleLineInput<'_> {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        // if self.hidden {
        //     let mut hidden = TextArea::new(vec!["*".repeat(self.textarea.lines()[0].len())]);
        //     let pos = self.textarea.cursor();
        //
        //     hidden.set_cursor_style(DEFAULT_STYLE);
        //     hidden.set_cursor_line_style(DEFAULT_STYLE);
        //     hidden.move_cursor(CursorMove::Jump(pos.0 as u16, pos.1 as u16));
        //     hidden.render(area, buf);
        // } else {
        //     let tt = self.textarea.hidden();
        //     tt.0.render(area, buf);
        self.textarea.render(area, buf);
        // }
    }
}
