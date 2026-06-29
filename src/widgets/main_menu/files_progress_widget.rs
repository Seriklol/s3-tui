use crate::s3::objects::get_file_name;
use crate::utils::{BLOCK_ACTIVE_STYLE, DEFAULT_STYLE, LIST_HIGHLIGHT_STYLE};
use aws_sdk_s3::types::MultipartUpload;
use bytesize::ByteSize;
use indexmap::IndexMap;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::{Line, Widget};
use ratatui::widgets::{Block, List, ListItem, ListState};
use std::sync::Arc;
use tokio::task::JoinHandle;

#[derive(Default)]
pub struct FilesProgressWidget {
    pub selected: SelectedProgressTab,
    pub downloads: ProgressListState,
    pub uploads: ProgressListState,
    pub unfinished_uploads: UnfinishedUploadsListState,
    is_active: bool,
}

impl FilesProgressWidget {
    pub fn set_active(&mut self, active: bool) {
        self.is_active = active;
    }
}

impl Widget for &mut FilesProgressWidget {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let mut block = Block::bordered().style(if self.is_active {
            BLOCK_ACTIVE_STYLE
        } else {
            DEFAULT_STYLE
        });

        match self.selected {
            SelectedProgressTab::Downloads => {
                if self.downloads.state.selected().is_some() {
                    block = block.title_bottom("| Cancel download: Del |");
                }
            }
            SelectedProgressTab::Uploads => {
                if self.uploads.state.selected().is_some() {
                    block = block.title_bottom("| Cancel upload: Del |");
                }
            }
            SelectedProgressTab::UnfinishedUploads => {
                if self.unfinished_uploads.state.selected().is_some() {
                    block = block.title_bottom("| Abort upload: Del |")
                }
            }
        }

        let mut downloads = Line::from("[3] Downloads").style(DEFAULT_STYLE);
        let mut uploads = Line::from("[4] Uploads").style(DEFAULT_STYLE);
        let mut unfinished = Line::from("[5] Unfinished").style(DEFAULT_STYLE);
        match self.selected {
            SelectedProgressTab::Downloads => downloads = downloads.style(BLOCK_ACTIVE_STYLE),
            SelectedProgressTab::Uploads => uploads = uploads.style(BLOCK_ACTIVE_STYLE),
            SelectedProgressTab::UnfinishedUploads => unfinished = unfinished.style(BLOCK_ACTIVE_STYLE),
        }

        block = block
            .title_top(downloads)
            .title_top(uploads)
            .title_top(unfinished);

        match self.selected {
            SelectedProgressTab::Downloads => {
                if self.downloads.list.is_empty() {
                    Widget::render(block, area, buf);
                } else {
                    let downloads_list = List::new(self.downloads.list.iter().map(|download| {
                        ListItem::new(format!(
                            "{0}: {1}: {2} / {3}",
                            download.0.bucket,
                            get_file_name(&download.0.key).unwrap_or(&download.0.key),
                            ByteSize::b(download.1.done).display().si(),
                            ByteSize::b(download.1.size).display().si()
                        ))
                    }))
                    .style(DEFAULT_STYLE)
                    .highlight_style(LIST_HIGHLIGHT_STYLE)
                    .block(block);

                    ratatui::prelude::StatefulWidget::render(
                        downloads_list,
                        area,
                        buf,
                        &mut self.downloads.state,
                    );
                }
            }
            SelectedProgressTab::Uploads => {
                if self.uploads.list.is_empty() {
                    Widget::render(block, area, buf);
                } else {
                    let uploads_list = List::new(self.uploads.list.iter().map(|upload| {
                        ListItem::new(format!(
                            "{0}: {1}: {2} / {3}",
                            upload.0.bucket,
                            get_file_name(&upload.0.key).unwrap_or(&upload.0.key),
                            ByteSize::b(upload.1.done).display().si(),
                            ByteSize::b(upload.1.size).display().si()
                        ))
                    }))
                    .style(DEFAULT_STYLE)
                    .highlight_style(LIST_HIGHLIGHT_STYLE)
                    .block(block);

                    ratatui::prelude::StatefulWidget::render(
                        uploads_list,
                        area,
                        buf,
                        &mut self.uploads.state,
                    );
                }
            }
            SelectedProgressTab::UnfinishedUploads => {
                if self.unfinished_uploads.list.is_empty() {
                    Widget::render(block, area, buf);
                } else {
                    let unfinished = List::new(
                        self.unfinished_uploads
                            .list
                            .iter()
                            .map(|kvp| ListItem::new(kvp.0.clone())),
                    )
                    .style(DEFAULT_STYLE)
                    .highlight_style(LIST_HIGHLIGHT_STYLE)
                    .block(block);

                    ratatui::prelude::StatefulWidget::render(
                        unfinished,
                        area,
                        buf,
                        &mut self.unfinished_uploads.state,
                    );
                }
            }
        }
    }
}

#[derive(Default, Clone, PartialEq)]
pub enum SelectedProgressTab {
    #[default]
    Downloads,
    Uploads,
    UnfinishedUploads,
}

#[derive(Default)]
pub struct ProgressListState {
    pub list: IndexMap<KeyWithBucket, Progress>,
    pub state: ListState,
}

#[derive(PartialEq, Eq, Hash, Clone)]
pub struct KeyWithBucket {
    pub key: String,
    pub bucket: String,
}

#[derive(Default, Clone)]
pub struct UnfinishedUploadsListState {
    pub bucket: String,
    pub list: IndexMap<String, MultipartUpload>,
    pub state: ListState,
}

impl UnfinishedUploadsListState {
    pub fn new(bucket: String, list: Vec<MultipartUpload>) -> Self {
        let mut map = IndexMap::with_capacity(list.len());
        list.iter().for_each(|upload| {
            if let Some(key) = upload.key.clone() {
                map.entry(key).or_insert_with(|| upload.clone());
            }
        });
        Self {
            bucket,
            list: map,
            state: Default::default(),
        }
    }
}

#[derive(Clone)]
pub struct Progress {
    pub done: u64,
    pub size: u64,
    pub handle: Arc<JoinHandle<()>>,
}

impl Progress {
    pub fn new(downloaded: u64, size: u64, handle: Arc<JoinHandle<()>>) -> Self {
        Self {
            done: downloaded,
            size,
            handle,
        }
    }
}
