#[derive(Debug, Clone)]
pub struct TreeNode {
    pub name: String,
    pub node_type: NodeType,
}

#[derive(Debug, PartialEq, Clone)]
pub enum NodeType {
    Dir(Open),
    File(Key),
}

#[derive(Debug, PartialEq, Clone)]
pub struct Open(pub bool);

#[derive(Debug, PartialEq, Clone)]
pub struct Key(pub String);

impl TreeNode {
    pub fn new_dir(name: String) -> Self {
        Self {
            name,
            node_type: NodeType::Dir(Open(false)),
        }
    }

    pub fn new_file(name: String, key: String) -> Self {
        Self {
            name,
            node_type: NodeType::File(Key(key)),
        }
    }

    pub fn try_open(&mut self) -> bool {
        if let NodeType::Dir(Open(open)) = &self.node_type {
            if *open {
                false
            } else {
                self.node_type = NodeType::Dir(Open(true));
                true
            }
        } else {
            false
        }
    }
    pub fn try_close(&mut self) -> bool {
        if let NodeType::Dir(Open(open)) = &self.node_type {
            if *open {
                self.node_type = NodeType::Dir(Open(false));
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    pub fn try_toggle(&mut self) -> bool {
        if let NodeType::Dir(open) = &self.node_type {
            self.node_type = NodeType::Dir(Open(!open.0));
            true
        } else {
            false
        }
    }
}
