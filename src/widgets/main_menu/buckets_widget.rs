use crate::utils::{BLOCK_ACTIVE_STYLE, DEFAULT_STYLE, LIST_HIGHLIGHT_STYLE};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::Widget;
use ratatui::widgets::{Block, Borders, List, ListState};

#[derive(Clone)]
pub struct BucketsWidget {
    pub list: Vec<String>,
    pub state: ListState,
    pub is_active: bool,
}

impl BucketsWidget {
    pub fn new(list: Vec<String>) -> Self {
        Self {
            list,
            state: ListState::default(),
            is_active: true,
        }
    }

    pub fn set_active(&mut self, is_active: bool) {
        self.is_active = is_active;
    }
}

impl Widget for &mut BucketsWidget {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let mut buckets_block = Block::default()
            .borders(Borders::ALL)
            .title_top("[1] Buckets")
            .style(if self.is_active {
                BLOCK_ACTIVE_STYLE
            } else {
                DEFAULT_STYLE
            });

        if self.state.selected().is_some() {
            buckets_block = buckets_block.title_bottom("| Open bucket: Enter | New bucket: b | New folder: f |");
        }

        let ref_list = self.list.iter().map(|x| x.as_str()).collect::<Vec<_>>();

        let buckets_list = List::new(ref_list)
            .style(DEFAULT_STYLE)
            .highlight_style(LIST_HIGHLIGHT_STYLE)
            .block(buckets_block);

        ratatui::prelude::StatefulWidget::render(buckets_list, area, buf, &mut self.state)
    }
}
