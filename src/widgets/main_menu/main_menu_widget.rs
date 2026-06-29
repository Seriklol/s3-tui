use crate::events::S3Update;
use crate::s3::objects::{
    create_bucket, create_folder, delete_multipart_upload, delete_object, download_object,
    get_file_download_path, get_file_name, get_object_info, list_buckets, list_objects,
    upload_object, S3ObjectInfo, S3ObjectsList,
};
use crate::s3::s3_profile::S3Profile;
use crate::utils::{centered_area, DEFAULT_STYLE};
use crate::widgets::main_menu::buckets_widget::BucketsWidget;
use crate::widgets::main_menu::file_tree::tree_node::{Key, NodeType};
use crate::widgets::main_menu::file_tree::tree_widget::TreeWidget;
use crate::widgets::main_menu::files_progress_widget::{
    FilesProgressWidget, KeyWithBucket, Progress, SelectedProgressTab, UnfinishedUploadsListState,
};
pub(crate) use crate::widgets::main_menu::input_widget::InputWidget;
use crate::widgets::main_menu::input_widget::InputWidgetType;
use anyhow::{anyhow, Result};
use aws_config::{BehaviorVersion, Region};
use aws_sdk_s3::config::{Credentials, SharedCredentialsProvider};
use aws_sdk_s3::primitives::DateTimeFormat;
use aws_sdk_s3::Client;
use aws_smithy_types_convert::date_time::DateTimeExt;
use bytesize::ByteSize;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::prelude::{Line, Widget};
use ratatui::widgets::{Block, Clear, Paragraph};
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;

pub struct MainMenuWidget<'a> {
    pub s3_client: Arc<Client>,
    pub active_window: ActiveWindow,

    pub buckets: BucketsWidget,
    pub tree: TreeWidget,
    pub files_progress: FilesProgressWidget,
    pub object_info: Option<Arc<S3ObjectInfo>>,
    pub input_popup: Option<InputWidget<'a>>,
}

#[derive(Default, PartialEq, Copy, Clone)]
pub enum ActiveWindow {
    #[default]
    Buckets,
    FileTree,
    FilesProgress,
}

impl MainMenuWidget<'_> {
    pub fn set_bucket_objects(&mut self, list: S3ObjectsList) {
        self.tree.set_tree(list.bucket.clone(), list.objects);
        self.files_progress.unfinished_uploads =
            UnfinishedUploadsListState::new(list.bucket, list.uploads);
    }

    pub async fn handle_key(
        &mut self,
        key: KeyEvent,
        tx_s3: &UnboundedSender<S3Update>,
    ) -> anyhow::Result<()> {
        if let Some(popup) = &mut self.input_popup {
            match key.code {
                KeyCode::Enter => {
                    match popup.widget_type {
                        InputWidgetType::Upload => {
                            let client = self.s3_client.clone();
                            let dir = popup.directory.clone();
                            let path = popup.input.text().clone();
                            let file_name = get_file_name(&path)?;
                            let file_key = if dir.is_empty() {
                                file_name.to_string()
                            } else {
                                format!("{}/{}", dir, file_name)
                            };
                            let bucket = popup.bucket.clone();
                            let tx = tx_s3.clone();
                            let key_clone = file_key.clone();
                            let handle = Arc::new(tokio::spawn(async move {
                                upload_object(&client, &bucket, &key_clone, &path, &tx).await
                            }));
                            self.files_progress.uploads.list.insert(
                                KeyWithBucket { key: file_key, bucket: popup.bucket.clone() },
                                Progress { done: 0, size: 0, handle },
                            );
                        }
                        InputWidgetType::NewFolder => {
                            let key = if popup.directory.is_empty() {
                                popup.input.text().clone()
                            } else {
                                format!("{}/{}", popup.directory, popup.input.text())
                            };
                            create_folder(&self.s3_client, &popup.bucket, &key).await?;
                            Self::spawn_list_objects(&self.s3_client, &popup.bucket, tx_s3);
                        }
                        InputWidgetType::NewBucket => {
                            create_bucket(&self.s3_client, popup.input.text()).await?;
                            let buckets = list_buckets(&self.s3_client).await?;
                            self.buckets = BucketsWidget::new(buckets);
                        }
                    }
                    self.input_popup = None;
                }
                KeyCode::Esc => self.input_popup = None,
                _ => { popup.input.handle_text_input(key); }
            }
            return Ok(());
        }

        match key.code {
            KeyCode::Char('1') => { self.active_window = ActiveWindow::Buckets; return Ok(()); }
            KeyCode::Char('2') => { self.active_window = ActiveWindow::FileTree; return Ok(()); }
            KeyCode::Char('3') => {
                self.active_window = ActiveWindow::FilesProgress;
                self.files_progress.selected = SelectedProgressTab::Downloads;
                return Ok(());
            }
            KeyCode::Char('4') => {
                self.active_window = ActiveWindow::FilesProgress;
                self.files_progress.selected = SelectedProgressTab::Uploads;
                return Ok(());
            }
            KeyCode::Char('5') => {
                self.active_window = ActiveWindow::FilesProgress;
                self.files_progress.selected = SelectedProgressTab::UnfinishedUploads;
                return Ok(());
            }
            _ => {}
        }

        match self.active_window {
            ActiveWindow::Buckets => match key.code {
                KeyCode::Up => self.buckets.state.select_previous(),
                KeyCode::Down => self.buckets.state.select_next(),
                KeyCode::Enter => {
                    if let Some(ind) = self.buckets.state.selected()
                        && let Some(bucket) = self.buckets.list.get(ind)
                    {
                        Self::spawn_list_objects(&self.s3_client, bucket, tx_s3);
                    }
                }
                KeyCode::Char('u') => {
                    if let Some(ind) = self.buckets.state.selected() {
                        let bucket = self.buckets.list[ind].clone();
                        self.input_popup =
                            Some(InputWidget::new("".to_string(), bucket, InputWidgetType::Upload));
                    }
                }
                KeyCode::Char('b') => {
                    self.input_popup = Some(InputWidget::new(
                        "".to_string(),
                        "".to_string(),
                        InputWidgetType::NewBucket,
                    ));
                }
                KeyCode::Char('f') => {
                    if let Some(ind) = self.buckets.state.selected()
                        && let Some(bucket) = self.buckets.list.get(ind)
                    {
                        self.input_popup = Some(InputWidget::new(
                            "".to_string(),
                            bucket.clone(),
                            InputWidgetType::NewFolder,
                        ));
                    }
                }
                _ => {}
            },
            ActiveWindow::FileTree => match key.code {
                KeyCode::Up => self.tree.key_up(),
                KeyCode::Down => self.tree.key_down(),
                KeyCode::Enter => self.tree.toggle_selected(),
                KeyCode::Char('d') => {
                    if let Some(tree) = &self.tree.tree
                        && let Some(sel_id) = tree.selected
                        && let Some(sel_node) = tree.arena.get(sel_id)
                        && let NodeType::File(Key(file_key)) =
                            &sel_node.get().node_type
                        && !self.files_progress.downloads.list.contains_key(&KeyWithBucket {
                            key: file_key.clone(),
                            bucket: tree.bucket.clone(),
                        })
                    {
                        let client = self.s3_client.clone();
                        let tx = tx_s3.clone();
                        let fk = file_key.clone();
                        let bucket = tree.bucket.clone();
                        let handle = Arc::new(tokio::spawn(async move {
                            download_object(&client, &fk, &bucket, &tx).await
                        }));
                        self.files_progress.downloads.list.insert(
                            KeyWithBucket { key: file_key.clone(), bucket: tree.bucket.clone() },
                            Progress { done: 0, size: 0, handle },
                        );
                    }
                }
                KeyCode::Char('u') => {
                    if let Some(tree) = &self.tree.tree
                        && let Some(node_id) = tree.selected
                        && let Some(node) = tree.arena.get(node_id)
                        && let NodeType::Dir(_) =
                            &node.get().node_type
                    {
                        let dir = tree.get_path(&node_id);
                        self.input_popup = Some(InputWidget::new(
                            dir,
                            tree.bucket.clone(),
                            InputWidgetType::Upload,
                        ));
                    }
                }
                KeyCode::Char('f') => {
                    if let Some(tree) = &self.tree.tree
                        && let Some(node_id) = tree.selected
                        && let Some(node) = tree.arena.get(node_id)
                        && let NodeType::Dir(_) =
                            &node.get().node_type
                    {
                        let dir = tree.get_path(&node_id);
                        self.input_popup = Some(InputWidget::new(
                            dir,
                            tree.bucket.clone(),
                            InputWidgetType::NewFolder,
                        ));
                    }
                }
                KeyCode::Char('i') => {
                    if let Some(tree) = &self.tree.tree
                        && let Some(sel_id) = tree.selected
                        && let Some(sel_node) = tree.arena.get(sel_id)
                        && let NodeType::File(Key(k)) =
                            &sel_node.get().node_type
                    {
                        let k = k.clone();
                        let client = self.s3_client.clone();
                        let bucket = tree.bucket.clone();
                        let tx = tx_s3.clone();
                        tokio::spawn(async move {
                            match get_object_info(&client, &bucket, &k).await {
                                Ok(info) => {
                                    let _ = tx.send(S3Update::ObjectInfo(Arc::new(
                                        S3ObjectInfo { key: k, info },
                                    )));
                                }
                                Err(err) => { let _ = tx.send(S3Update::Error(err)); }
                            }
                        });
                    }
                }
                KeyCode::Delete => {
                    if let Some(tree) = &self.tree.tree
                        && let Some(sel_id) = tree.selected
                        && let Some(sel_node) = tree.arena.get(sel_id)
                        && let NodeType::File(Key(k)) =
                            &sel_node.get().node_type
                    {
                        let k = k.clone();
                        let bucket = tree.bucket.clone();
                        let client = self.s3_client.clone();
                        let tx = tx_s3.clone();
                        tokio::spawn(async move {
                            match delete_object(&client, &k, &bucket).await {
                                Ok(_) => Self::list_objects_result(&client, &bucket, &tx).await,
                                Err(err) => { let _ = tx.send(S3Update::Error(err)); }
                            }
                        });
                    }
                }
                _ => {}
            },
            ActiveWindow::FilesProgress => match self.files_progress.selected {
                SelectedProgressTab::Downloads => match key.code {
                    KeyCode::Up => self.files_progress.downloads.state.select_previous(),
                    KeyCode::Down => self.files_progress.downloads.state.select_next(),
                    KeyCode::Delete => {
                        if let Some(ind) = self.files_progress.downloads.state.selected()
                            && let Some(kb) = self
                                .files_progress
                                .downloads
                                .list
                                .get_index(ind)
                                .map(|t| t.0.clone())
                            && let Some(dl) =
                                self.files_progress.downloads.list.shift_remove(&kb)
                        {
                            dl.handle.abort();
                            let path = get_file_download_path(&kb.key)?;
                            tokio::fs::remove_file(path).await?;
                        }
                    }
                    _ => {}
                },
                SelectedProgressTab::Uploads => match key.code {
                    KeyCode::Up => self.files_progress.uploads.state.select_previous(),
                    KeyCode::Down => self.files_progress.uploads.state.select_next(),
                    KeyCode::Delete => {
                        if let Some(ind) = self.files_progress.uploads.state.selected()
                            && let Some(kb) = self
                                .files_progress
                                .uploads
                                .list
                                .get_index(ind)
                                .map(|t| t.0.clone())
                            && let Some(ul) =
                                self.files_progress.uploads.list.shift_remove(&kb)
                        {
                            ul.handle.abort();
                        }
                    }
                    _ => {}
                },
                SelectedProgressTab::UnfinishedUploads => match key.code {
                    KeyCode::Up => self.files_progress.unfinished_uploads.state.select_previous(),
                    KeyCode::Down => self.files_progress.unfinished_uploads.state.select_next(),
                    KeyCode::Delete => {
                        if let Some(ind) =
                            self.files_progress.unfinished_uploads.state.selected()
                            && let Some(upload) =
                                self.files_progress.unfinished_uploads.list.get_index(ind)
                        {
                            let k = upload.0.clone();
                            let bucket = self.files_progress.unfinished_uploads.bucket.clone();
                            let client = self.s3_client.clone();
                            let upload_id = upload
                                .1
                                .upload_id
                                .clone()
                                .ok_or(anyhow!("Could not get upload ID"))?;
                            delete_multipart_upload(&client, bucket, k, upload_id).await?;
                        }
                    }
                    _ => {}
                },
            },
        }

        Ok(())
    }

    fn spawn_list_objects(client: &Client, bucket: &str, tx_s3: &UnboundedSender<S3Update>) {
        let client = client.clone();
        let bucket = bucket.to_string();
        let tx = tx_s3.clone();
        tokio::spawn(async move { Self::list_objects_result(&client, &bucket, &tx).await });
    }

    async fn list_objects_result(
        client: &Client,
        bucket: &str,
        tx_s3: &UnboundedSender<S3Update>,
    ) {
        match list_objects(client, bucket).await {
            Ok(objects) => { let _ = tx_s3.send(S3Update::ListObjects(objects)); }
            Err(err) => { let _ = tx_s3.send(S3Update::Error(err)); }
        }
    }

    pub async fn new(profile: &S3Profile) -> Result<Self> {
        let creds = SharedCredentialsProvider::new(Credentials::new(
            profile.access_key_id.clone(),
            profile.secret_access_key.clone(),
            None,
            None,
            "",
        ));

        let conf = aws_config::SdkConfig::builder()
            .endpoint_url(&profile.endpoint)
            .region(Region::new(profile.region.clone()))
            .behavior_version(BehaviorVersion::latest())
            .credentials_provider(creds)
            .build();

        let s3_conf = aws_sdk_s3::config::Builder::from(&conf)
            .force_path_style(true)
            .build();
        let client = Client::from_conf(s3_conf);

        let buckets = list_buckets(&client).await?;

        let state = MainMenuWidget {
            active_window: Default::default(),
            s3_client: Arc::new(client),
            buckets: BucketsWidget::new(buckets),
            tree: Default::default(),
            files_progress: Default::default(),
            object_info: None,
            input_popup: None,
        };

        Ok(state)
    }
}

impl Widget for &mut MainMenuWidget<'_> {
    fn render(self, area: ratatui::layout::Rect, buf: &mut ratatui::buffer::Buffer) {
        let hor_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![Constraint::Ratio(1, 3); 3])
            .split(area);

        let vert_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Ratio(2, 3), Constraint::Ratio(1, 3)])
            .split(hor_chunks[2]);

        let info_block = Block::bordered()
            .title_top("Object info")
            .style(DEFAULT_STYLE);

        if let Some(info) = &self.object_info {
            let info_paragraph = Paragraph::new(vec![
                Line::raw(format!(
                    "Name: {}",
                    get_file_name(&info.key).unwrap_or(&info.key)
                )),
                Line::raw(format!(
                    "Size: {}",
                    ByteSize::b(info.info.content_length.unwrap_or(0) as u64)
                        .display()
                        .si()
                )),
                Line::raw(format!(
                    "Type: {}",
                    info.info
                        .content_type
                        .clone()
                        .map(|t| t.to_string())
                        .unwrap_or(String::from("unknown"))
                )),
                Line::raw(format!(
                    "Last modified: {}",
                    info.info
                        .last_modified
                        .map(|a| a
                            .to_chrono_utc()
                            .map(|b| b.to_string())
                            .unwrap_or(a.fmt(DateTimeFormat::HttpDate).unwrap_or(a.to_string())))
                        .unwrap_or(String::from("none"))
                )),
            ])
            .block(info_block);

            info_paragraph.render(vert_chunks[1], buf);
        } else {
            info_block.render(vert_chunks[1], buf);
        }

        self.buckets.set_active(false);
        self.tree.set_active(false);
        self.files_progress.set_active(false);
        match self.active_window {
            ActiveWindow::Buckets => {
                self.buckets.set_active(true);
            }
            ActiveWindow::FileTree => {
                self.tree.set_active(true);
            }
            ActiveWindow::FilesProgress => {
                self.files_progress.set_active(true);
            }
        }

        self.buckets.render(hor_chunks[0], buf);
        self.files_progress.render(vert_chunks[0], buf);
        self.tree.render(hor_chunks[1], buf);

        if let Some(popup) = &mut self.input_popup {
            let inner = centered_area(area, Constraint::Percentage(50), Constraint::Length(3));
            Clear.render(inner, buf);
            popup.render(inner, buf);
        }
    }
}
