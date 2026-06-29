use crate::widgets::main_menu::file_tree::tree_node::NodeType::Dir;
use crate::widgets::main_menu::file_tree::tree_node::{Open, TreeNode};
use aws_sdk_s3::types::Object;
use indexmap::IndexSet;
use indextree::{Arena, NodeId};

#[derive(Clone)]
pub struct Tree {
    pub bucket: String,
    pub root: NodeId,
    pub arena: Arena<TreeNode>,
    pub selected: Option<NodeId>,
    pub flattened: IndexSet<NodeId>,
    pub offset: usize,
}

impl Tree {
    pub fn new(bucket: String, objects: Vec<Object>) -> Self {
        let mut arena = Arena::<TreeNode>::new();
        let root = arena.new_node(TreeNode::new_dir(String::from("root")));

        for object in objects {
            let key = object.key().unwrap_or_default();
            let path = key
                .split('/')
                .map(|s| s.to_string())
                .filter(|s| !s.is_empty())
                .collect::<Vec<String>>();
            let is_dir = key.ends_with('/');
            let mut curr_root = root;

            for (ind, path_part) in path.iter().enumerate() {
                match curr_root
                    .children(&arena)
                    .find(|p| arena.get(*p).is_some_and(|n| n.get().name == *path_part))
                {
                    None => {
                        let is_last = ind == path.len() - 1;
                        curr_root = if is_last {
                            let node = if is_dir {
                                TreeNode::new_dir(path_part.clone())
                            } else {
                                TreeNode::new_file(path_part.clone(), String::from(key))
                            };
                            curr_root.append_value(node, &mut arena)
                        } else {
                            curr_root.append_value(TreeNode::new_dir(path_part.clone()), &mut arena)
                        }
                    }
                    Some(node_id) => {
                        curr_root = node_id;
                    }
                }
            }
        }

        let flattened = Tree::create_flattened_set(&arena, &root);

        Self {
            bucket,
            root,
            arena,
            selected: None,
            flattened,
            offset: 0,
        }
    }

    pub fn get_path(&self, node_id: &NodeId) -> String {
        let mut path = node_id
            .ancestors(&self.arena)
            .filter_map(|a| self.arena.get(a).map(|n| n.get().name.clone()))
            .collect::<Vec<_>>();
        path.reverse();

        path[1..path.len()].join("/")
    }

    pub fn update_flattened(&mut self) {
        self.flattened = Tree::create_flattened_set(&self.arena, &self.root);
    }

    fn create_flattened_set(arena: &Arena<TreeNode>, root: &NodeId) -> IndexSet<NodeId> {
        let mut flat = IndexSet::new();
        for root_child in root.children(arena) {
            let nodes = Tree::get_open_dirs_and_files(arena, root_child);
            for node in nodes {
                flat.insert(node);
            }
        }
        flat
    }

    fn get_open_dirs_and_files(arena: &Arena<TreeNode>, curr_node: NodeId) -> Vec<NodeId> {
        let Some(node) = arena.get(curr_node) else { return vec![]; };
        let mut root = vec![curr_node];
        if let Dir(Open(true)) = node.get().node_type {
            let mut nodes = curr_node
                .children(arena)
                .flat_map(|child| Tree::get_open_dirs_and_files(arena, child))
                .collect::<Vec<NodeId>>();
            root.append(&mut nodes);
        }
        root
    }
}
