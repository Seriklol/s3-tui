use crate::widgets::profiles::profile_info::ProfileInfo;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::{Line, Widget};
use ratatui::widgets::Block;

#[derive(Default, Clone)]
pub struct NewProfileWidget<'a> {
    pub info: ProfileInfo<'a>,
}

impl Widget for &mut NewProfileWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let controls = Line::from(vec![
            "| Save profile: Enter ".into(),
            "| Back: Esc |".into(),
        ])
        .left_aligned();

        let block = Block::bordered()
            .title_top(Line::from("New profile").centered())
            .title_bottom(controls);
        
        let inner = block.inner(area);
        block.render(area, buf);
        self.info.render(inner, buf);
    }
}
