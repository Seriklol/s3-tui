use crate::s3::s3_profile::S3Profile;
use crate::s3::s3_secret::S3Secret;
use crate::utils::{centered_area, DEFAULT_STYLE, LIST_HIGHLIGHT_STYLE};
use crate::widgets::profiles::edit_profile_widget::EditProfileWidget;
use crate::widgets::profiles::new_profile_widget::NewProfileWidget;
use anyhow::anyhow;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Rect};
use ratatui::prelude::{Line, StatefulWidget, Text, Widget};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};

#[derive(Clone)]
pub struct ProfilesWidget<'a> {
    pub window: CurrentProfilesWindow<'a>,
    pub secret: S3Secret,
}

impl Default for ProfilesWidget<'_> {
    fn default() -> Self {
        Self {
            window: Default::default(),
            secret: S3Secret::from_keyring().unwrap_or(S3Secret { profiles: vec![] }),
        }
    }
}

impl ProfilesWidget<'_> {
    pub fn try_save_profile(&mut self) -> anyhow::Result<()> {
        match &self.window {
            CurrentProfilesWindow::NewProfile(widget) => {
                let new_profile = widget.info.to_profile();
                self.secret.profiles.push(new_profile);
                self.secret.save()?
            }
            CurrentProfilesWindow::EditProfile(widget) => {
                let new_profile = widget.info.to_profile();
                let old_profile = self
                    .secret
                    .profiles
                    .get_mut(widget.index)
                    .ok_or(anyhow!("Could not edit profile"))?;
                *old_profile = new_profile;
                self.secret.save()?
            }
            _ => {}
        };

        Ok(())
    }
}

impl ProfilesWidget<'_> {
    pub fn handle_key(&mut self, key: KeyEvent) -> anyhow::Result<Option<S3Profile>> {
        use CurrentProfilesWindow::*;
        match &mut self.window {
            ProfileSelection(selection) => match key.code {
                KeyCode::Char('n') => {
                    self.window = NewProfile(NewProfileWidget::default());
                }
                KeyCode::Char('e') => {
                    if let Some(ind) = selection.selected() {
                        let profile = self.secret.profiles.get(ind)
                            .ok_or(anyhow!("Could not get profile"))?.clone();
                        self.window = EditProfile(EditProfileWidget::new(ind, profile));
                    }
                }
                KeyCode::Delete => {
                    if let Some(ind) = selection.selected() {
                        self.window = DeleteProfileConfirmation(ind);
                    }
                }
                KeyCode::Down => selection.select_next(),
                KeyCode::Up => selection.select_previous(),
                KeyCode::Enter => {
                    if let Some(ind) = selection.selected() {
                        return Ok(Some(self.secret.profiles[ind].clone()));
                    }
                }
                _ => {}
            },
            NewProfile(new_profile) => match key.code {
                KeyCode::Esc => self.window = ProfileSelection(ListState::default()),
                KeyCode::Down => new_profile.info.select_next(),
                KeyCode::Up => new_profile.info.select_previous(),
                KeyCode::Enter => {
                    self.try_save_profile()?;
                    self.window = ProfileSelection(ListState::default());
                }
                _ => { new_profile.info.edit_current_field(key); }
            },
            EditProfile(edit_profile) => match key.code {
                KeyCode::Esc => self.window = ProfileSelection(ListState::default()),
                KeyCode::Down => edit_profile.info.select_next(),
                KeyCode::Up => edit_profile.info.select_previous(),
                KeyCode::Enter => {
                    self.try_save_profile()?;
                    self.window = ProfileSelection(ListState::default());
                }
                _ => { edit_profile.info.edit_current_field(key); }
            },
            DeleteProfileConfirmation(delete_ind) => match key.code {
                KeyCode::Char('y') => {
                    self.secret.profiles.remove(*delete_ind);
                    self.secret.save()?;
                    self.window = ProfileSelection(ListState::default());
                }
                KeyCode::Char('n') => self.window = ProfileSelection(ListState::default()),
                _ => {}
            },
        }
        Ok(None)
    }
}

impl Widget for &mut ProfilesWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let app_block = Block::default()
            .borders(Borders::ALL)
            .title("s3-tui")
            .title_alignment(Alignment::Center)
            .style(DEFAULT_STYLE);

        match &mut self.window {
            CurrentProfilesWindow::ProfileSelection(state) => {
                let inner = centered_area(
                    app_block.inner(area),
                    Constraint::Percentage(50),
                    Constraint::Percentage(50),
                );
                app_block.render(area, buf);
                render_profile_selection(&self.secret.profiles, inner, buf, state)
            }
            CurrentProfilesWindow::NewProfile(new_profile_widget) => {
                let inner = centered_area(
                    app_block.inner(area),
                    Constraint::Percentage(50),
                    Constraint::Length(12),
                );
                app_block.render(area, buf);
                new_profile_widget.render(inner, buf)
            }
            CurrentProfilesWindow::EditProfile(edit_profile_widget) => {
                let inner = centered_area(
                    app_block.inner(area),
                    Constraint::Percentage(50),
                    Constraint::Length(12),
                );
                app_block.render(area, buf);
                edit_profile_widget.render(inner, buf)
            }
            CurrentProfilesWindow::DeleteProfileConfirmation(ind) => {
                let profile_name = &self.secret.profiles[*ind].name;
                let message = format!(
                    "Are you sure you want to delete profile \"{}\"?",
                    profile_name
                );

                let block =
                    Block::bordered().title_bottom(Line::from("| Yes: y | No: n |").left_aligned());
                let line = Line::from(message);
                let popup_width = line.width() + 6;
                let paragraph = Paragraph::new(line)
                    .centered()
                    .block(block)
                    .style(DEFAULT_STYLE);

                let inner = centered_area(
                    app_block.inner(area),
                    Constraint::Length(popup_width as u16),
                    Constraint::Length(3),
                );
                app_block.render(area, buf);
                paragraph.render(inner, buf);
            }
        }
    }
}

#[derive(Clone)]
pub enum CurrentProfilesWindow<'a> {
    ProfileSelection(ListState),
    NewProfile(NewProfileWidget<'a>),
    EditProfile(EditProfileWidget<'a>),
    DeleteProfileConfirmation(usize),
}

impl Default for CurrentProfilesWindow<'_> {
    fn default() -> Self {
        CurrentProfilesWindow::ProfileSelection(ListState::default())
    }
}

fn render_profile_selection(
    profiles: &Vec<S3Profile>,
    area: Rect,
    buf: &mut Buffer,
    state: &mut ListState,
) {
    let mut vec_profiles = Vec::<ListItem>::new();
    for prf in profiles {
        vec_profiles.push(ListItem::new(
            Text::from(prf.name.clone()).alignment(Alignment::Center),
        ));
    }

    let controls = if profiles.is_empty() {
        "| New profile: n | Quit: Esc |"
    } else if state.selected().is_none() {
        "| Connect: Enter | New profile: n | Quit: q |"
    } else {
        "| Connect: Enter | New profile: n | Edit profile: e | Delete profile: Del | Quit: q |"
    };

    let list = List::new(vec_profiles)
        .block(
            Block::bordered()
                .title(Line::from("Select profile").centered())
                .title_bottom(Line::from(controls).left_aligned()),
        )
        .style(DEFAULT_STYLE)
        .highlight_style(LIST_HIGHLIGHT_STYLE);

    StatefulWidget::render(list, area, buf, state);
}
