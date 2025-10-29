use crate::s3::s3_profile::S3Profile;
use crate::utils::BLOCK_ACTIVE_STYLE;
use crate::widgets::single_line_input::SingleLineInput;
use crossterm::event::KeyEvent;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::prelude::{Direction, Line, Stylize, Widget};

#[derive(Clone)]
pub struct ProfileInfo<'a> {
    name_input: SingleLineInput<'a>,
    endpoint_input: SingleLineInput<'a>,
    region_input: SingleLineInput<'a>,
    access_key_id_input: SingleLineInput<'a>,
    secret_access_key_input: SingleLineInput<'a>,
    selected: SelectedField,
}

impl ProfileInfo<'_> {
    pub fn from_existing(profile: S3Profile) -> Self {
        Self {
            name_input: SingleLineInput::new(profile.name, false, true),
            endpoint_input: SingleLineInput::new(profile.endpoint, false, false),
            region_input: SingleLineInput::new(profile.region, false, false),
            access_key_id_input: SingleLineInput::new(profile.access_key_id, false, false),
            secret_access_key_input: SingleLineInput::new(profile.secret_access_key, false, false),
            selected: SelectedField::default(),
        }
    }

    pub fn edit_current_field(&mut self, event: KeyEvent) -> bool {
        let curr_input = match self.selected {
            SelectedField::Name => &mut self.name_input,
            SelectedField::Endpoint => &mut self.endpoint_input,
            SelectedField::Region => &mut self.region_input,
            SelectedField::AccessKeyId => &mut self.access_key_id_input,
            SelectedField::SecretAccessKey => &mut self.secret_access_key_input,
        };

        curr_input.handle_text_input(event)
    }

    pub fn to_profile(&self) -> S3Profile {
        S3Profile::new(
            self.name_input.text().into(),
            self.endpoint_input.text().into(),
            self.region_input.text().into(),
            self.access_key_id_input.text().into(),
            self.secret_access_key_input.text().into(),
        )
    }

    pub fn select_next(&mut self) {
        self.selected = self.selected.next_field();
        self.update_styles();
    }

    pub fn select_previous(&mut self) {
        self.selected = self.selected.prev_field();
        self.update_styles();
    }

    fn update_styles(&mut self) {
        self.name_input.deactivate_input();
        self.endpoint_input.deactivate_input();
        self.region_input.deactivate_input();
        self.access_key_id_input.deactivate_input();
        self.secret_access_key_input.deactivate_input();

        match self.selected {
            SelectedField::Name => {
                self.name_input.activate_input();
            }
            SelectedField::Endpoint => {
                self.endpoint_input.activate_input();
            }
            SelectedField::Region => {
                self.region_input.activate_input();
            }
            SelectedField::AccessKeyId => {
                self.access_key_id_input.activate_input();
            }
            SelectedField::SecretAccessKey => {
                self.secret_access_key_input.activate_input();
            }
        };
    }
}

impl Default for ProfileInfo<'_> {
    fn default() -> Self {
        Self {
            name_input:SingleLineInput::new("".into(), false, true),
            endpoint_input:SingleLineInput::new("".into(), false, false),
            region_input:SingleLineInput::new("".into(), false, false),
            access_key_id_input:SingleLineInput::new("".into(), false, false),
            secret_access_key_input: SingleLineInput::new("".into(), false, false),
            selected: SelectedField::default(),
        }
    }
}

impl Widget for &mut ProfileInfo<'_> {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let rects = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1); 10])
            .split(area);
        let mut name_title = Line::raw("Name").bold();
        let mut endpoint_title = Line::raw("Endpoint").bold();
        let mut region_title = Line::raw("Region").bold();
        let mut access_key_title = Line::raw("Access Key ID").bold();
        let mut secret_access_key_title = Line::raw("Secret Access Key").bold();

        match self.selected {
            SelectedField::Name => {
                name_title = name_title.style(BLOCK_ACTIVE_STYLE);
            }
            SelectedField::Endpoint => {
                endpoint_title = endpoint_title.style(BLOCK_ACTIVE_STYLE);
            }
            SelectedField::Region => {
                region_title = region_title.style(BLOCK_ACTIVE_STYLE);
            }
            SelectedField::AccessKeyId => {
                access_key_title = access_key_title.style(BLOCK_ACTIVE_STYLE);
            }
            SelectedField::SecretAccessKey => {
                secret_access_key_title = secret_access_key_title.style(BLOCK_ACTIVE_STYLE);
            }
        };

        name_title.render(rects[0], buf);
        self.name_input.render(rects[1], buf);
        endpoint_title.render(rects[2], buf);
        self.endpoint_input.render(rects[3], buf);
        region_title.render(rects[4], buf);
        self.region_input.render(rects[5], buf);
        access_key_title.render(rects[6], buf);
        self.access_key_id_input.render(rects[7], buf);
        secret_access_key_title.render(rects[8], buf);
        self.secret_access_key_input.render(rects[9], buf);
    }
}

#[derive(Default, Clone, Copy, PartialEq)]
enum SelectedField {
    #[default]
    Name,
    Endpoint,
    Region,
    AccessKeyId,
    SecretAccessKey,
}

impl SelectedField {
    const FIELDS: [Self; 5] = [
        SelectedField::Name,
        SelectedField::Endpoint,
        SelectedField::Region,
        SelectedField::AccessKeyId,
        SelectedField::SecretAccessKey,
    ];

    fn next_field(self) -> SelectedField {
        let current_index = self as usize;
        Self::FIELDS[(current_index + 1) % Self::FIELDS.len()]
    }

    fn prev_field(self) -> SelectedField {
        let current_index = self as usize;
        Self::FIELDS[(current_index + Self::FIELDS.len() - 1) % Self::FIELDS.len()]
    }
}
