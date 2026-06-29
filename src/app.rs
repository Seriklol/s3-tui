use crate::app_state::{AppState, CurrentWindow};
use crate::events::S3Update;
use crate::utils::render_error;
use crate::widgets::main_menu::main_menu_widget::MainMenuWidget;
use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyModifiers};
use futures::StreamExt;
use ratatui::DefaultTerminal;
use tokio::sync::mpsc;

#[derive(Clone)]
pub struct Popup {
    pub message: String,
}

impl Popup {
    pub fn new(message: String) -> Self {
        Self { message }
    }
}

#[derive(Default)]
pub struct App {
    pub running: bool,
    pub state: Box<AppState<'static>>,
}

impl App {
    pub async fn run(mut self) -> color_eyre::Result<()> {
        self.running = true;
        let mut terminal = ratatui::init();
        let mut reader = EventStream::new();
        let (tx_input, mut rx_process_input) = mpsc::channel::<Event>(8);
        let (tx_s3, mut rx_process_s3) = mpsc::unbounded_channel::<S3Update>();

        let input_task: tokio::task::JoinHandle<color_eyre::Result<()>> =
            tokio::spawn(async move {
                while let Some(Ok(event)) = reader.next().await {
                    match event {
                        Event::Key(key) if key.is_press() || key.is_release() => {
                            tx_input.send(Event::Key(key)).await?;
                        }
                        Event::Resize(_, _) => tx_input.send(event).await?,
                        _ => {}
                    }
                }
                Ok(())
            });

        let process_task: tokio::task::JoinHandle<anyhow::Result<()>> = tokio::spawn(async move {
            App::draw(&mut terminal, &mut self.state)?;
            loop {
                tokio::select! {
                    Some(event) = rx_process_input.recv() => {
                        match event {
                            Event::Key(key) => {
                                if let Err(err) = self.handle_input(&tx_s3, key).await {
                                    self.state.popup = Some(Popup::new(err.to_string()));
                                }
                                App::draw(&mut terminal, &mut self.state)?;
                                if !self.running {
                                    break;
                                }
                            }
                            Event::Resize(_, _) => App::draw(&mut terminal, &mut self.state)?,
                            _ => {}
                        }
                    }
                    Some(s3_update) = rx_process_s3.recv() => {
                        self.handle_s3_events(s3_update);
                        App::draw(&mut terminal, &mut self.state)?;
                    }
                }
            }
            Ok(())
        });

        tokio::select!(
            _ = input_task => {}
            _ = process_task => {});

        Ok(())
    }

    fn draw(terminal: &mut DefaultTerminal, state: &mut AppState) -> anyhow::Result<()> {
        terminal.draw(|frame| {
            match &mut state.window {
                CurrentWindow::Profiles(profiles_widget) => {
                    frame.render_widget(profiles_widget, frame.area());
                }
                CurrentWindow::Main(main_state) => {
                    frame.render_widget(main_state, frame.area());
                }
            };

            if let Some(popup) = &state.popup {
                render_error(&popup.message, frame);
            }
        })?;

        Ok(())
    }

    fn handle_s3_events(&mut self, s3_update: S3Update) {
        use crate::widgets::main_menu::files_progress_widget::KeyWithBucket;
        if let CurrentWindow::Main(ref mut main) = self.state.window {
            match s3_update {
                S3Update::ListObjects(list) => main.set_bucket_objects(list),
                S3Update::ObjectInfo(info) => main.object_info = Some(info),
                S3Update::Error(err) => self.state.popup = Some(Popup::new(err.to_string())),
                S3Update::DownloadProgress(update) => {
                    let kb = KeyWithBucket { key: update.key, bucket: update.bucket };
                    match update.result {
                        Ok(p) if p.done >= p.size => { main.files_progress.downloads.list.shift_remove(&kb); }
                        Ok(p) => { if let Some(d) = main.files_progress.downloads.list.get_mut(&kb) { d.done = p.done; d.size = p.size; } }
                        Err(err) => { self.state.popup = Some(Popup::new(err.to_string())); main.files_progress.downloads.list.shift_remove(&kb); }
                    }
                }
                S3Update::UploadProgress(update) => {
                    let kb = KeyWithBucket { key: update.key, bucket: update.bucket };
                    match update.result {
                        Ok(p) if p.done >= p.size => { main.files_progress.uploads.list.shift_remove(&kb); }
                        Ok(p) => { if let Some(u) = main.files_progress.uploads.list.get_mut(&kb) { u.done = p.done; u.size = p.size; } }
                        Err(err) => { self.state.popup = Some(Popup::new(err.to_string())); main.files_progress.uploads.list.shift_remove(&kb); }
                    }
                }
            }
        } else if let S3Update::Error(err) = s3_update {
            self.state.popup = Some(Popup::new(err.to_string()));
        }
    }

    async fn handle_input(
        &mut self,
        tx_s3: &mpsc::UnboundedSender<S3Update>,
        key: KeyEvent,
    ) -> anyhow::Result<()> {
        if let (_, KeyCode::Char('q'))
        | (KeyModifiers::CONTROL, KeyCode::Char('c') | KeyCode::Char('C')) =
            (key.modifiers, key.code)
        {
            self.quit();
            return Ok(());
        }

        if self.state.popup.is_some() {
            if key.code == KeyCode::Enter {
                self.state.popup = None;
            }
            return Ok(());
        }

        match &mut self.state.window {
            CurrentWindow::Profiles(profiles) => {
                if let Some(profile) = profiles.handle_key(key)? {
                    let widget = MainMenuWidget::new(&profile).await?;
                    self.state = Box::new(AppState::new(CurrentWindow::Main(widget), None));
                }
            }
            CurrentWindow::Main(main) => {
                main.handle_key(key, tx_s3).await?;
            }
        }

        Ok(())
    }

    fn quit(&mut self) {
        self.running = false;
    }
}
