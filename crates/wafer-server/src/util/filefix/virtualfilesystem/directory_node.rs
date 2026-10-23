use std::io;

use wafer_core::random::java_string_hash;

use crate::util::java_collections::JavaHashMap;

use super::copy_on_write_fs_path::CopyOnWriteFSPath;
use super::exception::{CowFSNoSuchFileException, CowFSNotDirectoryException};
use super::node::{Node, NodeId, NodeKind};

pub(crate) struct DirectoryNode {
    child_nodes: JavaHashMap<NodeId>,
}

impl DirectoryNode {
    pub(crate) fn new() -> DirectoryNode {
        DirectoryNode { child_nodes: JavaHashMap::new(java_string_hash) }
    }
}

pub(crate) struct FileTree {
    nodes: Vec<Node>,
    root: NodeId,
}

impl FileTree {
    pub(crate) fn empty() -> FileTree {
        FileTree { nodes: Vec::new(), root: 0 }
    }

    pub(crate) fn new(root_path: &CopyOnWriteFSPath) -> FileTree {
        FileTree { nodes: vec![Node::new(root_path, NodeKind::Directory(DirectoryNode::new()))], root: 0 }
    }

    pub(crate) fn root(&self) -> NodeId {
        self.root
    }

    pub(crate) fn alloc(&mut self, node: Node) -> NodeId {
        self.nodes.push(node);
        self.nodes.len() - 1
    }

    pub(crate) fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id]
    }

    pub(crate) fn node_mut(&mut self, id: NodeId) -> &mut Node {
        &mut self.nodes[id]
    }

    fn directory(&self, id: NodeId) -> &DirectoryNode {
        match &self.nodes[id].kind {
            NodeKind::Directory(directory) => directory,
            NodeKind::File(_) => panic!("Not a directory node"),
        }
    }

    fn directory_mut(&mut self, id: NodeId) -> &mut DirectoryNode {
        match &mut self.nodes[id].kind {
            NodeKind::Directory(directory) => directory,
            NodeKind::File(_) => panic!("Not a directory node"),
        }
    }

    pub(crate) fn is_directory(&self, id: NodeId) -> bool {
        matches!(self.nodes[id].kind, NodeKind::Directory(_))
    }

    pub(crate) fn children(&self, directory: NodeId) -> Vec<NodeId> {
        self.directory(directory).child_nodes.iter().map(|(_, id)| *id).collect()
    }

    pub(crate) fn has_children(&self, directory: NodeId) -> bool {
        !self.directory(directory).child_nodes.is_empty()
    }

    pub(crate) fn add_child(&mut self, directory: NodeId, child: NodeId) {
        let name = self.nodes[child].name().expect("child name").to_string();
        self.directory_mut(directory).child_nodes.insert(name, child);
        self.nodes[child].parent = Some(directory);
    }

    pub(crate) fn remove_child(&mut self, directory: NodeId, name: &str) {
        self.directory_mut(directory).child_nodes.remove(name);
    }

    pub(crate) fn get_child(&self, directory: NodeId, name: &str) -> Option<NodeId> {
        self.directory(directory).child_nodes.get(name).copied()
    }

    pub(crate) fn directory_by_path(&self, path: &CopyOnWriteFSPath) -> io::Result<NodeId> {
        let node = self.by_path(path)?;
        if self.is_directory(node) {
            Ok(node)
        } else {
            Err(CowFSNotDirectoryException::new(format!("{path} was a file, expected directory")))
        }
    }

    pub(crate) fn by_path(&self, path: &CopyOnWriteFSPath) -> io::Result<NodeId> {
        self.by_path_or_null(path).ok_or_else(|| CowFSNoSuchFileException::new(path.to_string()))
    }

    pub(crate) fn by_path_or_null(&self, path: &CopyOnWriteFSPath) -> Option<NodeId> {
        let name_count = path.get_name_count();
        let mut directory = self.root;
        for i in 0..name_count {
            let name = path.get_name(i).to_string();
            if name != "." {
                if name == ".." {
                    if let Some(parent) = self.nodes[directory].parent {
                        directory = parent;
                    }
                } else {
                    let next_node = self.get_child(directory, &name);
                    match next_node {
                        Some(next) if self.is_directory(next) => directory = next,
                        other => return if i == name_count - 1 { other } else { None },
                    }
                }
            }
        }
        Some(directory)
    }
}
