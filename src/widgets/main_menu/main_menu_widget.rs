use crate::s3::objects::{get_file_name, list_buckets, S3ObjectInfo, S3ObjectsList};
use crate::s3::s3_profile::S3Profile;
use crate::utils::{centered_area, DEFAULT_STYLE};
use crate::widgets::main_menu::buckets_widget::BucketsWidget;
use crate::widgets::main_menu::file_tree::tree_widget::TreeWidget;
use crate::widgets::main_menu::files_progress_widget::{
    FilesProgressWidget, UnfinishedUploadsListState,
};
pub(crate) use crate::widgets::main_menu::input_widget::InputWidget;
use anyhow::Result;
use aws_config::{BehaviorVersion, Region};
use aws_sdk_s3::config::{Credentials, SharedCredentialsProvider};
use aws_sdk_s3::primitives::DateTimeFormat;
use aws_sdk_s3::Client;
use aws_smithy_types_convert::date_time::DateTimeExt;
use bytesize::ByteSize;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::prelude::{Line, Widget};
use ratatui::widgets::{Block, Clear, Paragraph};
use std::sync::Arc;

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
        self.files_progress.unfinished_uploads = UnfinishedUploadsListState::new(list.bucket, list.uploads);
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

        let client = Client::new(&conf);

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
