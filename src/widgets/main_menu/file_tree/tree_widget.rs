use crate::utils::{BLOCK_ACTIVE_STYLE, DEFAULT_STYLE, LIST_HIGHLIGHT_STYLE};
use crate::widgets::main_menu::file_tree::tree::Tree;
use crate::widgets::main_menu::file_tree::tree_node::NodeType::Dir;
use crate::widgets::main_menu::file_tree::tree_node::{NodeType, Open};
use aws_sdk_s3::types::Object;
use indextree::NodeId;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::{Line, StatefulWidget, Widget};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};
use std::ops::Index;

const OPEN_PREFIX: &str = "\u{25bc} ";
const CLOSED_PREFIX: &str = "\u{25b6} ";

#[derive(Default, Clone)]
pub struct TreeWidget {
    pub tree: Option<Tree>,
    is_active: bool,
}

impl TreeWidget {
    pub fn set_tree(&mut self, bucket: String, objects: Vec<Object>) {
        self.tree = Some(Tree::new(bucket, objects));
    }

    pub fn set_active(&mut self, active: bool) {
        self.is_active = active;
    }

    pub fn select(&mut self, index: NodeId) -> bool {
        if let Some(tree) = self.tree.as_mut() {
            let changed = tree.selected.is_some_and(|sel| sel != index) || tree.selected.is_none();
            tree.selected = Some(index);
            changed
        } else {
            false
        }
    }

    pub fn try_open(&mut self, identifier: NodeId) {
        if let Some(tree) = self.tree.as_mut() {
            if let Some(node) = &mut tree.arena.get_mut(identifier)
                && node.get_mut().try_open()
            {
                tree.update_flattened();
            }
        }
    }

    pub fn try_close(&mut self, identifier: NodeId) {
        if let Some(tree) = self.tree.as_mut() {
            if let Some(node) = &mut tree.arena.get_mut(identifier)
                && node.get_mut().try_close()
            {
                tree.update_flattened();
            }
        }
    }

    pub fn toggle(&mut self, identifier: NodeId) {
        if let Some(tree) = self.tree.as_mut() {
            if let Some(node) = &mut tree.arena.get_mut(identifier)
                && node.get_mut().try_toggle()
            {
                tree.update_flattened();
            }
        }
    }

    pub fn toggle_selected(&mut self) {
        if let Some(tree) = self.tree.as_mut() {
            if let Some(index) = tree.selected {
                self.toggle(index)
            }
        }
    }

    pub fn close_all(&mut self) {
        if let Some(tree) = self.tree.as_mut() {
            for node in &mut tree.arena.iter_mut() {
                if let Dir(_) = node.get().node_type {
                    node.get_mut().try_close();
                }
            }
            tree.update_flattened();
        }
    }

    pub fn key_up(&mut self) {
        if let Some(tree) = self.tree.as_mut() {
            tree.selected = match tree.selected {
                None => tree.flattened.get_index(0).copied(),
                Some(node_id) => match tree.flattened.get_index_of(&node_id) {
                    None => tree.flattened.get_index(0).copied(),
                    Some(ind) => Some(tree.flattened[ind.saturating_sub(1)]),
                }
            };
        }
    }

    pub fn key_down(&mut self) {
        if let Some(tree) = self.tree.as_mut() {
            tree.selected = match tree.selected {
                None => tree.flattened.get_index(0).copied(),
                Some(index) => match tree.flattened.len() {
                    0 => None,
                    _ => match tree.flattened.get_index_of(&index) {
                        None => tree.flattened.get_index(0).copied(),
                        Some(ind) => Some(tree.flattened[(ind + 1).clamp(0, tree.flattened.len() - 1)]),
                    }
                },
            };
        }
    }

    pub fn try_select_to_parent(&mut self) {
        if let Some(tree) = self.tree.as_mut() {
            tree.selected = match tree.selected {
                None => None,
                Some(node_id) => match node_id.ancestors(&tree.arena).next() {
                    None => Some(node_id),
                    Some(ancestor) => Some(ancestor),
                },
            }
        }
    }
}

impl Widget for &mut TreeWidget {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let curr_style = if self.is_active {
            BLOCK_ACTIVE_STYLE
        } else {
            DEFAULT_STYLE
        };

        let mut block = Block::default()
            .borders(Borders::ALL)
            .title_top("[2] Tree")
            .style(curr_style);

        match self.tree.as_mut() {
            None => {
                block.render(area, buf);
            }
            Some(tree) => {
                if tree.flattened.is_empty() {
                    block.render(area, buf);
                    return;
                }

                if let Some(node_id) = tree.selected
                    && let Some(node) = tree.arena.get(node_id)
                {
                    if let NodeType::File(_) = node.get().node_type {
                        block = block
                            .title_bottom(Line::from("| Download: d | Delete: Del | Info: i |"))
                    } else {
                        block =
                            block.title_bottom(Line::from("| Open/Close: Enter | Upload file: u | New folder: f |"))
                    }
                }

                let inner = block.inner(area);
                block.render(area, buf);

                let list = List::new(tree.flattened.iter().map(|node_id| {
                    let offset = node_id.ancestors(&tree.arena).count();
                    let Some(node_data) = tree.arena.get(*node_id) else { return ListItem::from(""); };
                    let node = node_data.get();
                    let prefix = match node.node_type {
                        Dir(Open(open)) => {
                            let mut pref = " ".repeat(offset - 1);
                            pref.push_str(if open { OPEN_PREFIX } else { CLOSED_PREFIX });
                            pref
                        }
                        NodeType::File(_) => " ".repeat(offset + 1),
                    }
                    .to_owned();
                    ListItem::from(prefix + &node.name)
                }))
                .style(DEFAULT_STYLE)
                .highlight_style(LIST_HIGHLIGHT_STYLE);

                let mut list_state = ListState::default().with_offset(tree.offset);
                if let Some(sel) = tree.selected {
                    list_state.select(tree.flattened.get_index_of(&sel));
                }
                StatefulWidget::render(list, inner, buf, &mut list_state);
                tree.selected = list_state.selected().map(|sel| *tree.flattened.index(sel));
                tree.offset = list_state.offset();
            }
        }
    }
}
