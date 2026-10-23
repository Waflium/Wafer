use super::copy_on_write_fs_path::CopyOnWriteFSPath;
use super::directory_node::DirectoryNode;
use super::file_node::FileNode;

pub(crate) type NodeId = usize;

pub(crate) enum NodeKind {
    File(FileNode),
    Directory(DirectoryNode),
}

pub(crate) struct Node {
    pub(crate) parent: Option<NodeId>,
    pub(crate) path: Vec<String>,
    pub(crate) kind: NodeKind,
}

impl Node {
    pub(crate) fn new(cow_path: &CopyOnWriteFSPath, kind: NodeKind) -> Node {
        let mut node = Node { parent: None, path: Vec::new(), kind };
        node.set_path(cow_path);
        node
    }

    pub(crate) fn name(&self) -> Option<&str> {
        self.path.last().map(String::as_str)
    }

    pub(crate) fn set_path(&mut self, path: &CopyOnWriteFSPath) {
        self.path = path.normalize().to_absolute_path().names().to_vec();
    }
}
