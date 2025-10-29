use crate::app_state::{AppState, CurrentWindow};
use crate::s3::objects::{create_bucket, create_folder, delete_multipart_upload, delete_object, get_file_download_path, get_file_name, get_object_info, list_buckets, list_objects, upload_object, S3ObjectInfo};
use crate::s3::objects::{download_object, S3ObjectsList};
use crate::utils::render_error;
use crate::widgets::main_menu::file_tree::tree_node::{Key, NodeType};
use crate::widgets::main_menu::files_progress_widget::{
    KeyWithBucket, Progress, SelectedProgressTab,
};
use crate::widgets::main_menu::input_widget::InputWidgetType;
use crate::widgets::main_menu::main_menu_widget::{ActiveWindow, InputWidget, MainMenuWidget};
use crate::widgets::profiles::edit_profile_widget::EditProfileWidget;
use crate::widgets::profiles::new_profile_widget::NewProfileWidget;
use crate::widgets::profiles::profiles_widget::CurrentProfilesWindow::{
    DeleteProfileConfirmation, EditProfile, NewProfile, ProfileSelection,
};
use anyhow::anyhow;
use aws_sdk_s3::Client;
use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyModifiers};
use futures::StreamExt;
use ratatui::widgets::ListState;
use ratatui::DefaultTerminal;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::mpsc::UnboundedSender;
use crate::widgets::main_menu::buckets_widget::BucketsWidget;

#[derive(Clone)]
pub struct Popup {
    pub message: String,
}

impl Popup {
    pub fn new(message: String) -> Self {
        Self { message }
    }
}

pub enum S3Update {
    // TODO: wrap S3ObjectsList in Arc/Box?
    ListObjects(S3ObjectsList),
    ObjectInfo(Arc<S3ObjectInfo>),
    DownloadProgress(FileUpdate),
    UploadProgress(FileUpdate),
    Error(anyhow::Error),
}

pub struct FileUpdate {
    pub bucket: String,
    pub key: String,
    pub result: Result<FileProgress, anyhow::Error>,
}

impl FileUpdate {
    pub fn new(bucket: String, key: String, result: Result<FileProgress, anyhow::Error>) -> Self {
        Self {
            bucket,
            key,
            result,
        }
    }
}

pub struct FileProgress {
    pub done: u64,
    pub size: u64,
}

impl FileProgress {
    pub fn new(downloaded: u64, size: u64) -> Self {
        Self {
            done: downloaded,
            size,
        }
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
        let (tx_input, mut rx_process_input) = mpsc::channel::<KeyEvent>(8);
        let (tx_s3, mut rx_process_s3) = mpsc::unbounded_channel::<S3Update>();

        let input_task: tokio::task::JoinHandle<color_eyre::Result<()>> =
            tokio::spawn(async move {
                while let Some(Ok(opt_event)) = reader.next().await {
                    if let Event::Key(key) = opt_event
                        && (key.is_press() || key.is_release())
                    {
                        tx_input.send(key).await?;
                    }
                }
                Ok(())
            });

        let process_task: tokio::task::JoinHandle<anyhow::Result<()>> = tokio::spawn(async move {
            loop {
                tokio::select! {
                    () = tokio::time::sleep(std::time::Duration::from_millis(100)) => {
                            App::draw(&mut terminal, &mut self.state).unwrap()
                    }
                    Some(key_event) = rx_process_input.recv() => {
                        if let Err(err) = self.handle_input(&tx_s3, key_event).await{
                            self.state.popup = Some(Popup::new(err.to_string()))
                        }
                        App::draw(&mut terminal, &mut self.state).unwrap();
                        if !self.running {
                            break;
                        }
                    }
                    Some(s3_update) = rx_process_s3.recv() => {
                        self.handle_s3_events(s3_update);
                        App::draw(&mut terminal, &mut self.state).unwrap();
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
        match s3_update {
            S3Update::ListObjects(list) => {
                if let CurrentWindow::Main(ref mut main_state) = self.state.window {
                    main_state.set_bucket_objects(list);
                }
            }
            S3Update::DownloadProgress(update) => {
                if let CurrentWindow::Main(ref mut main_state) = self.state.window {
                    match update.result {
                        Ok(progress) => {
                            if progress.done >= progress.size {
                                main_state.files_progress.downloads.list.shift_remove(
                                    &KeyWithBucket {
                                        key: update.key,
                                        bucket: update.bucket,
                                    },
                                );
                            } else if let Some(download) = main_state
                                .files_progress
                                .downloads
                                .list
                                .get_mut(&KeyWithBucket {
                                    key: update.key,
                                    bucket: update.bucket,
                                })
                            {
                                download.done = progress.done;
                                download.size = progress.size;
                            }
                        }
                        Err(err) => {
                            self.state.popup = Some(Popup::new(err.to_string()));
                            main_state
                                .files_progress
                                .downloads
                                .list
                                .shift_remove(&KeyWithBucket {
                                    key: update.key,
                                    bucket: update.bucket,
                                });
                        }
                    }
                }
            }
            S3Update::UploadProgress(update) => {
                if let CurrentWindow::Main(ref mut main_state) = self.state.window {
                    match update.result {
                        Ok(progress) => {
                            if progress.done == progress.size {
                                main_state.files_progress.uploads.list.shift_remove(
                                    &KeyWithBucket {
                                        key: update.key,
                                        bucket: update.bucket,
                                    },
                                );
                            } else if let Some(upload) = main_state
                                .files_progress
                                .uploads
                                .list
                                .get_mut(&KeyWithBucket {
                                    key: update.key,
                                    bucket: update.bucket,
                                })
                            {
                                upload.done = progress.done;
                                upload.size = progress.size;
                            }
                        }
                        Err(err) => {
                            self.state.popup = Some(Popup::new(err.to_string()));
                            main_state
                                .files_progress
                                .uploads
                                .list
                                .shift_remove(&KeyWithBucket {
                                    key: update.key,
                                    bucket: update.bucket,
                                });
                        }
                    }
                }
            }
            S3Update::ObjectInfo(info) => {
                if let CurrentWindow::Main(ref mut main_state) = self.state.window {
                    main_state.object_info = Some(info);
                }
            }
            S3Update::Error(error) => {
                self.state.popup = Some(Popup::new(error.to_string()));
            }
        }
    }

    async fn handle_input(
        &mut self,
        tx_s3: &mpsc::UnboundedSender<S3Update>,
        key: KeyEvent,
    ) -> anyhow::Result<()> {
        // Quit app
        if let (_, KeyCode::Char('q'))
        | (KeyModifiers::CONTROL, KeyCode::Char('c') | KeyCode::Char('C')) =
            (key.modifiers, key.code)
        {
            self.quit();
            return Ok(());
        }

        // Error popup
        if self.state.popup.is_some() {
            if key.code == KeyCode::Enter {
                self.state.popup = None;
            }
            return Ok(());
        }

        match &mut self.state.window {
            CurrentWindow::Profiles(profiles) => match &mut profiles.window {
                ProfileSelection(selection) => match key.code {
                    KeyCode::Char('n') => {
                        profiles.window = NewProfile(NewProfileWidget::default());
                    }
                    KeyCode::Char('e') => {
                        if let Some(ind) = selection.selected() {
                            let profile = profiles
                                .secret
                                .profiles
                                .get(ind)
                                .ok_or(anyhow!("Could not get profile"))?
                                .clone();
                            profiles.window = EditProfile(EditProfileWidget::new(ind, profile));
                        }
                    }
                    KeyCode::Delete => {
                        if let Some(ind) = selection.selected() {
                            profiles.window = DeleteProfileConfirmation(ind);
                        }
                    }
                    KeyCode::Down => {
                        selection.select_next();
                    }
                    KeyCode::Up => {
                        selection.select_previous();
                    }
                    KeyCode::Enter => {
                        if let Some(ind) = selection.selected() {
                            MainMenuWidget::new(&profiles.secret.profiles[ind])
                                .await
                                .map(|widget| {
                                    self.state =
                                        Box::new(AppState::new(CurrentWindow::Main(widget), None));
                                })?;
                        }
                    }
                    _ => {}
                },
                NewProfile(new_profile) => match key.code {
                    KeyCode::Esc => {
                        profiles.window = ProfileSelection(ListState::default());
                    }
                    KeyCode::Down => {
                        new_profile.info.select_next();
                    }
                    KeyCode::Up => {
                        new_profile.info.select_previous();
                    }
                    KeyCode::Enter => {
                        profiles.try_save_profile()?;
                        profiles.window = ProfileSelection(ListState::default());
                    }
                    _ => {
                        new_profile.info.edit_current_field(key);
                    }
                },
                EditProfile(edit_profile) => match key.code {
                    KeyCode::Esc => {
                        profiles.window = ProfileSelection(ListState::default());
                    }
                    KeyCode::Down => {
                        edit_profile.info.select_next();
                    }
                    KeyCode::Up => {
                        edit_profile.info.select_previous();
                    }
                    KeyCode::Enter => {
                        profiles.try_save_profile()?;
                        profiles.window = ProfileSelection(ListState::default());
                    }
                    _ => {
                        edit_profile.info.edit_current_field(key);
                    }
                },
                DeleteProfileConfirmation(delete_ind) => match key.code {
                    KeyCode::Char('y') => {
                        profiles.secret.profiles.remove(*delete_ind);
                        profiles.secret.save()?;
                        profiles.window = ProfileSelection(ListState::default());
                    }
                    KeyCode::Char('n') => {
                        profiles.window = ProfileSelection(ListState::default());
                    }
                    _ => {}
                },
            },
            CurrentWindow::Main(main) => {
                if let Some(popup) = &mut main.input_popup {
                    match key.code {
                        KeyCode::Enter => {
                            match popup.widget_type {
                                InputWidgetType::Upload => {
                                    let client = main.s3_client.clone();
                                    let dir = popup.directory.clone();
                                    let path = popup.input.text().clone();
                                    let file_name = get_file_name(&path)?;
                                    let file_key = if dir.is_empty() {
                                        file_name.to_string()
                                    } else {
                                        format!("{}/{}", dir, file_name)
                                    };
                                    let file_key2 = file_key.clone();
                                    let tx_s3 = tx_s3.clone();
                                    let bucket = popup.bucket.clone();
                                    let handle = Arc::new(tokio::spawn(async move {
                                        upload_object(&client, &bucket, &file_key, &path, &tx_s3)
                                            .await
                                    }));
                                    main.files_progress.uploads.list.insert(
                                        KeyWithBucket {
                                            key: file_key2,
                                            bucket: popup.bucket.clone(),
                                        },
                                        Progress {
                                            done: 0,
                                            size: 0,
                                            handle,
                                        },
                                    );
                                }
                                InputWidgetType::NewFolder => {
                                    let dir = &popup.directory;
                                    let name = popup.input.text().clone();
                                    let key = if dir.is_empty() {
                                        name
                                    } else {
                                        format!("{}/{}", dir, name)
                                    };
                                    create_folder(&main.s3_client, &popup.bucket, &key).await?;
                                    Self::list_objects_or_error_task(
                                        &main.s3_client,
                                        &popup.bucket,
                                        tx_s3,
                                    );
                                }
                                InputWidgetType::NewBucket => {
                                    create_bucket(&main.s3_client, popup.input.text()).await?;
                                    let buckets = list_buckets(&main.s3_client).await?;
                                    main.buckets = BucketsWidget::new(buckets);
                                }
                            };

                            main.input_popup = None;
                        }
                        KeyCode::Esc => {
                            main.input_popup = None;
                        }
                        _ => {
                            popup.input.handle_text_input(key);
                        }
                    }
                    return Ok(());
                }

                match key.code {
                    KeyCode::Char('1') => {
                        main.active_window = ActiveWindow::Buckets;
                        return Ok(());
                    }
                    KeyCode::Char('2') => {
                        main.active_window = ActiveWindow::FileTree;
                        return Ok(());
                    }
                    KeyCode::Char('3') => {
                        main.active_window = ActiveWindow::FilesProgress;
                        main.files_progress.selected = SelectedProgressTab::Downloads;
                        return Ok(());
                    }
                    KeyCode::Char('4') => {
                        main.active_window = ActiveWindow::FilesProgress;
                        main.files_progress.selected = SelectedProgressTab::Uploads;
                        return Ok(());
                    }
                    KeyCode::Char('5') => {
                        main.active_window = ActiveWindow::FilesProgress;
                        main.files_progress.selected = SelectedProgressTab::UnfinishedUploads;
                        return Ok(());
                    }
                    _ => {}
                }

                match main.active_window {
                    ActiveWindow::Buckets => match key.code {
                        KeyCode::Up => main.buckets.state.select_previous(),
                        KeyCode::Down => main.buckets.state.select_next(),
                        KeyCode::Enter => {
                            if let Some(ind) = main.buckets.state.selected()
                                && let Some(bucket) = main.buckets.list.get(ind)
                            {
                                Self::list_objects_or_error_task(&main.s3_client, bucket, tx_s3);
                            }
                        }
                        KeyCode::Char('u') => {
                            if let Some(ind) = main.buckets.state.selected() {
                                let bucket = main.buckets.list[ind].clone();
                                main.input_popup = Some(InputWidget::new(
                                    "".to_string(),
                                    bucket,
                                    InputWidgetType::Upload,
                                ))
                            }
                        }
                        KeyCode::Char('b') => {
                            main.input_popup = Some(InputWidget::new(
                                "".to_string(),
                                "".to_string(),
                                InputWidgetType::NewBucket,
                            ));
                        }
                        KeyCode::Char('f') => {
                            if let Some(int) = main.buckets.state.selected()
                                && let Some(bucket) = main.buckets.list.get(int)
                            {
                                main.input_popup = Some(InputWidget::new(
                                    "".to_string(),
                                    bucket.clone(),
                                    InputWidgetType::NewFolder,
                                ));
                            }
                        }
                        _ => return Ok(()),
                    },
                    ActiveWindow::FileTree => match key.code {
                        KeyCode::Up => {
                            main.tree.key_up();
                        }
                        KeyCode::Down => {
                            main.tree.key_down();
                        }
                        KeyCode::Enter => {
                            main.tree.toggle_selected();
                        }
                        KeyCode::Char('d') => {
                            if let Some(tree) = &main.tree.tree
                                && let Some(sel_id) = tree.selected
                            {
                                let node = tree.arena.get(sel_id).unwrap().get();
                                if let NodeType::File(Key(file_key)) = &node.node_type
                                    && !main.files_progress.downloads.list.contains_key(
                                        &KeyWithBucket {
                                            key: file_key.clone(),
                                            bucket: tree.bucket.clone(),
                                        },
                                    )
                                {
                                    let client = main.s3_client.clone();
                                    let tx_s3 = tx_s3.clone();
                                    let file_key = file_key.clone();
                                    let file_key2 = file_key.clone();
                                    let bucket = tree.bucket.clone();
                                    let handle = Arc::new(tokio::spawn(async move {
                                        download_object(&client, &file_key.clone(), &bucket, &tx_s3)
                                            .await
                                    }));
                                    main.files_progress.downloads.list.insert(
                                        KeyWithBucket {
                                            key: file_key2,
                                            bucket: tree.bucket.clone(),
                                        },
                                        Progress {
                                            done: 0,
                                            size: 0,
                                            handle,
                                        },
                                    );
                                }
                            }
                        }
                        KeyCode::Char('u') => {
                            if let Some(tree) = &main.tree.tree
                                && let Some(node_id) = tree.selected
                                && let NodeType::Dir(_) =
                                    &tree.arena.get(node_id).unwrap().get().node_type
                            {
                                let dir = tree.get_path(&node_id);
                                main.input_popup = Some(InputWidget::new(
                                    dir,
                                    tree.bucket.clone(),
                                    InputWidgetType::Upload,
                                ))
                            }
                        }
                        KeyCode::Char('f') => {
                            if let Some(tree) = &main.tree.tree
                                && let Some(node_id) = tree.selected
                                && let NodeType::Dir(_) =
                                    &tree.arena.get(node_id).unwrap().get().node_type
                            {
                                let dir = tree.get_path(&node_id);
                                main.input_popup = Some(InputWidget::new(
                                    dir,
                                    tree.bucket.clone(),
                                    InputWidgetType::NewFolder,
                                ))
                            }
                        }
                        KeyCode::Char('i') => {
                            if let Some(tree) = &main.tree.tree
                                && let Some(sel_id) = tree.selected
                                && let NodeType::File(Key(key)) =
                                    &tree.arena.get(sel_id).unwrap().get().node_type
                            {
                                let key = key.clone();
                                let client = main.s3_client.clone();
                                let bucket = tree.bucket.clone();
                                let tx_s3 = tx_s3.clone();
                                tokio::spawn(async move {
                                    let info =
                                        get_object_info(&client, &bucket, &key).await.unwrap();
                                    tx_s3
                                        .send(S3Update::ObjectInfo(Arc::new(S3ObjectInfo {
                                            key,
                                            info,
                                        })))
                                        .unwrap();
                                });
                            }
                        }
                        KeyCode::Delete => {
                            if let Some(tree) = &main.tree.tree
                                && let Some(sel_id) = tree.selected
                                && let NodeType::File(Key(key)) =
                                    &tree.arena.get(sel_id).unwrap().get().node_type
                            {
                                let key = key.clone();
                                let bucket = tree.bucket.clone();
                                let client = main.s3_client.clone();
                                let tx_s3 = tx_s3.clone();
                                tokio::spawn(async move {
                                    let resp = delete_object(&client, &key, &bucket).await;
                                    match resp {
                                        Ok(_) => {
                                            Self::list_objects_or_error(&client, &bucket, &tx_s3)
                                                .await
                                        }
                                        Err(err) => tx_s3.send(S3Update::Error(err)).unwrap(),
                                    }
                                });
                            }
                        }
                        _ => {}
                    },
                    ActiveWindow::FilesProgress => match main.files_progress.selected {
                        SelectedProgressTab::Downloads => match key.code {
                            KeyCode::Up => main.files_progress.downloads.state.select_previous(),
                            KeyCode::Down => main.files_progress.downloads.state.select_next(),
                            KeyCode::Delete => {
                                if let Some(ind) = main.files_progress.downloads.state.selected()
                                    && let Some(key_bucket) = main
                                        .files_progress
                                        .downloads
                                        .list
                                        .get_index(ind)
                                        .map(|t| t.0.clone())
                                    && let Some(download) =
                                        main.files_progress.downloads.list.shift_remove(&key_bucket)
                                {
                                    download.handle.abort();
                                    let path = get_file_download_path(&key_bucket.key)?;
                                    tokio::fs::remove_file(path).await?;
                                }
                            }
                            _ => {}
                        },
                        SelectedProgressTab::Uploads => match key.code {
                            KeyCode::Up => main.files_progress.uploads.state.select_previous(),
                            KeyCode::Down => main.files_progress.uploads.state.select_next(),
                            KeyCode::Delete => {
                                if let Some(ind) = main.files_progress.uploads.state.selected()
                                    && let Some(key_bucket) = main
                                        .files_progress
                                        .uploads
                                        .list
                                        .get_index(ind)
                                        .map(|t| t.0.clone())
                                    && let Some(upload) =
                                        main.files_progress.uploads.list.shift_remove(&key_bucket)
                                {
                                    upload.handle.abort();
                                }
                            }
                            _ => {}
                        },
                        SelectedProgressTab::UnfinishedUploads => match key.code {
                            KeyCode::Up => main
                                .files_progress
                                .unfinished_uploads
                                .state
                                .select_previous(),
                            KeyCode::Down => {
                                main.files_progress.unfinished_uploads.state.select_next()
                            }
                            KeyCode::Delete => {
                                if let Some(ind) =
                                    main.files_progress.unfinished_uploads.state.selected()
                                    && let Some(upload) =
                                        main.files_progress.unfinished_uploads.list.get_index(ind)
                                {
                                    let key = upload.0.clone();
                                    let bucket =
                                        main.files_progress.unfinished_uploads.bucket.clone();
                                    let client = main.s3_client.clone();
                                    let upload_id = upload
                                        .1
                                        .upload_id
                                        .clone()
                                        .ok_or(anyhow!("Could not get upload ID"))?;
                                    delete_multipart_upload(&client, bucket, key, upload_id)
                                        .await?;
                                }
                            }
                            _ => {}
                        },
                    },
                }
                return Ok(());
            }
        }

        Ok(())
    }

    fn list_objects_or_error_task(
        client: &Client,
        bucket: &str,
        tx_s3: &UnboundedSender<S3Update>,
    ) {
        let client = client.clone();
        let bucket = bucket.to_string();
        let tx_s3 = tx_s3.clone();
        tokio::spawn(async move {
            Self::list_objects_or_error(&client, &bucket, &tx_s3).await;
        });
    }

    async fn list_objects_or_error(
        client: &Client,
        bucket: &str,
        tx_s3: &UnboundedSender<S3Update>,
    ) {
        let resp = list_objects(client, bucket).await;
        match resp {
            Ok(objects) => tx_s3.send(S3Update::ListObjects(objects)).unwrap(),
            Err(err) => tx_s3.send(S3Update::Error(err)).unwrap(),
        }
    }

    fn quit(&mut self) {
        self.running = false;
    }
}
