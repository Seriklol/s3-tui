use crate::s3::s3_profile::S3Profile;
use crate::widgets::profiles::profile_info::ProfileInfo;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::{Line, Widget};
use ratatui::widgets::Block;

#[derive(Clone)]
pub struct EditProfileWidget<'a> {
    pub index: usize,
    pub info: ProfileInfo<'a>,
}

impl EditProfileWidget<'_> {
    pub fn new(index: usize, profile: S3Profile) -> Self {
        Self {
            index,
            info: ProfileInfo::from_existing(profile),
        }
    }
}

impl Widget for &mut EditProfileWidget<'_> {
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
            .title_top(Line::from("Edit profile").centered())
            .title_bottom(controls);

        let inner = block.inner(area);
        block.render(area, buf);
        self.info.render(inner, buf);
    }
}
