use crate::app::Popup;
use crate::widgets::main_menu::main_menu_widget::MainMenuWidget;
use crate::widgets::profiles::profiles_widget::ProfilesWidget;

#[derive(Default)]
pub struct AppState<'a> {
    pub window: CurrentWindow<'a>,
    pub popup: Option<Popup>,
}

impl<'a> AppState<'a> {
    pub fn new(draw_state: CurrentWindow<'a>,  popup: Option<Popup>) -> Self {
        Self {
            window: draw_state,
            popup
        }
    }
}

pub enum CurrentWindow<'a> {
    Profiles(ProfilesWidget<'a>),
    Main(MainMenuWidget<'a>),
}

impl Default for CurrentWindow<'_> {
    fn default() -> Self {
        CurrentWindow::Profiles(ProfilesWidget::default())
    }
}
