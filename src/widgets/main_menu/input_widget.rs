use crate::utils::DEFAULT_STYLE;
use crate::widgets::single_line_input::SingleLineInput;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::{Alignment, Widget};
use ratatui::text::Line;
use ratatui::widgets::Block;

#[derive(Clone)]
pub struct InputWidget<'a> {
    pub input: SingleLineInput<'a>,
    pub directory: String,
    pub bucket: String,
    pub widget_type: InputWidgetType,
}

impl InputWidget<'_> {
    pub fn new(directory: String, bucket: String, widget_type: InputWidgetType) -> Self {
        Self {
            input: Default::default(),
            directory,
            bucket,
            widget_type,
        }
    }
}

impl Widget for &mut InputWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let block = Block::bordered()
            .title(match self.widget_type {
                InputWidgetType::Upload => Line::from("Enter path to the file to upload"),
                InputWidgetType::NewFolder => Line::from("Enter new folder name"),
                InputWidgetType::NewBucket => Line::from("Enter new bucket name"),
            })
            .title_alignment(Alignment::Center)
            .title_bottom(Line::from("| Upload: Enter | Cancel: Esc |").left_aligned())
            .style(DEFAULT_STYLE);

        let inner = block.inner(area);
        block.render(area, buf);
        self.input.render(inner, buf);
    }
}

#[derive(Clone)]
pub enum InputWidgetType {
    Upload,
    NewFolder,
    NewBucket,
}
