use std::fs::File;
use std::io;

use crate::util::basic_file_attributes::BasicFileAttributes;
use crate::util::java_nio::{CopyOption, java_io_error};

use super::copy_on_write_file_system::CopyOnWriteFileSystem;
use super::copy_on_write_fs_path::CopyOnWriteFSPath;
use super::directory_node::DirectoryNode;
use super::exception::{
    CowFSDirectoryNotEmptyException, CowFSFileAlreadyExistsException, CowFSFileSystemException, CowFSNoSuchFileException,
};
use super::file_node::FileNode;
use super::node::{Node, NodeKind};

pub const SCHEME: &str = "x-mc-copy-on-write";

#[derive(Clone, Copy, Default, Debug)]
pub struct OpenOptions {
    pub read: bool,
    pub write: bool,
    pub append: bool,
    pub create: bool,
    pub create_new: bool,
    pub truncate_existing: bool,
    pub delete_on_close: bool,
}

impl OpenOptions {
    pub fn read() -> OpenOptions {
        OpenOptions { read: true, ..OpenOptions::default() }
    }

    pub fn sync_output() -> OpenOptions {
        OpenOptions { write: true, create: true, truncate_existing: true, ..OpenOptions::default() }
    }

    fn wants_write(&self) -> bool {
        self.write || (!self.read && self.append)
    }

    fn open(&self, path: &std::path::Path) -> io::Result<File> {
        let writing = self.wants_write();
        let mut options = std::fs::OpenOptions::new();
        options.read(self.read || !writing);
        options.write(self.write);
        options.append(self.append);
        if writing {
            options.truncate(self.truncate_existing && self.write);
            if self.create_new {
                options.create_new(true);
            } else if self.create {
                options.create(true);
            }
        }
        options.open(path).map_err(|e| crate::util::java_nio::for_path(e, path))
    }
}

#[derive(Clone)]
pub struct CopyOnWriteFSProvider {
    fs: CopyOnWriteFileSystem,
}

impl CopyOnWriteFSProvider {
    pub fn new(fs: CopyOnWriteFileSystem) -> CopyOnWriteFSProvider {
        CopyOnWriteFSProvider { fs }
    }

    pub fn get_scheme(&self) -> &'static str {
        SCHEME
    }

    pub fn new_byte_channel(&self, path: &CopyOnWriteFSPath, options: &OpenOptions) -> io::Result<File> {
        if options.delete_on_close {
            panic!("java.lang.UnsupportedOperationException: DELETE_ON_CLOSE is not supported by CowFS");
        }
        let mut tree = self.fs.lock_tree();
        match tree.by_path_or_null(path) {
            None => {
                if !options.create && !options.create_new {
                    return Err(CowFSNoSuchFileException::new(path.to_string()));
                }
                let directory_node = tree.directory_by_path(&path.get_parent().expect("parent"))?;
                let temp_file = self.fs.create_temporary_file_path();
                let result = options.open(&temp_file)?;
                let child = tree.alloc(Node::new(path, NodeKind::File(FileNode::new(temp_file, true))));
                tree.add_child(directory_node, child);
                Ok(result)
            }
            Some(node) => {
                if tree.is_directory(node) {
                    return Err(CowFSFileSystemException::new(format!("{path}: not a regular file")));
                }
                let NodeKind::File(file_node) = &mut tree.node_mut(node).kind else { unreachable!() };
                if options.wants_write() {
                    file_node.ensure_copy(|| self.fs.create_temporary_file_path())?;
                }
                options.open(&file_node.storage_path)
            }
        }
    }

    pub fn new_input_stream(&self, path: &CopyOnWriteFSPath) -> io::Result<File> {
        self.new_byte_channel(path, &OpenOptions::read())
    }

    pub fn new_output_stream(&self, path: &CopyOnWriteFSPath, options: &OpenOptions) -> io::Result<File> {
        self.new_byte_channel(path, options)
    }

    pub fn new_directory_stream(&self, dir: &CopyOnWriteFSPath, filter: impl Fn(&CopyOnWriteFSPath) -> bool) -> io::Result<Vec<CopyOnWriteFSPath>> {
        let tree = self.fs.lock_tree();
        let directory_node = tree.directory_by_path(dir)?;
        let mut result = Vec::new();
        for child_node in tree.children(directory_node) {
            let path = CopyOnWriteFSPath::from_names(self.fs.clone(), tree.node(child_node).path.clone(), true);
            if filter(&path) {
                result.push(path);
            }
        }
        Ok(result)
    }

    pub fn create_directory(&self, dir: &CopyOnWriteFSPath) -> io::Result<()> {
        let Some(parent) = dir.get_parent() else {
            return Err(CowFSFileAlreadyExistsException::new(dir.to_string()));
        };
        let mut tree = self.fs.lock_tree();
        let parent_folder = tree.directory_by_path(&parent)?;
        let folder_name = dir.get_file_name().expect("file name").to_string();
        if tree.get_child(parent_folder, &folder_name).is_some() {
            return Err(CowFSFileAlreadyExistsException::new(dir.to_string()));
        }
        let child = tree.alloc(Node::new(dir, NodeKind::Directory(DirectoryNode::new())));
        tree.add_child(parent_folder, child);
        Ok(())
    }

    pub fn delete(&self, path: &CopyOnWriteFSPath) -> io::Result<()> {
        let mut tree = self.fs.lock_tree();
        let node = tree.by_path(path)?;
        let Some(parent) = tree.node(node).parent else {
            return Err(CowFSFileSystemException::new("Can't remove root"));
        };
        let name = tree.node(node).name().expect("name").to_string();
        match &tree.node(node).kind {
            NodeKind::Directory(_) => {
                if tree.has_children(node) {
                    return Err(CowFSDirectoryNotEmptyException::new(path.to_string()));
                }
            }
            NodeKind::File(file_node) => file_node.delete_copy().map_err(|e| crate::util::java_nio::for_path(e, &file_node.storage_path))?,
        }
        tree.remove_child(parent, &name);
        Ok(())
    }

    pub fn move_(&self, source: &CopyOnWriteFSPath, target: &CopyOnWriteFSPath, options: &[CopyOption]) -> io::Result<()> {
        if source.is_root() {
            return Err(CowFSFileSystemException::new(format!("{source}: can't move root directory")));
        }
        let mut replace_existing = false;
        for option in options {
            if *option == CopyOption::AtomicMove {
                return Err(java_io_error(
                    io::ErrorKind::Unsupported,
                    "java.nio.file.AtomicMoveNotSupportedException",
                    format!("{source} -> {target}: CowFs does not support atomic move"),
                ));
            }
            if *option == CopyOption::ReplaceExisting {
                replace_existing = true;
            }
        }
        let mut tree = self.fs.lock_tree();
        let Some(source_node) = tree.by_path_or_null(source) else {
            return Err(CowFSNoSuchFileException::new(source.to_string()));
        };
        let Some(parent) = target.to_absolute_path().get_parent() else {
            return Err(CowFSFileAlreadyExistsException::new(target.to_string()));
        };
        match tree.by_path_or_null(&parent) {
            Some(folder_target) if tree.is_directory(folder_target) => {
                let new_name = target.get_file_name().expect("file name").to_string();
                if let Some(old_child) = tree.get_child(folder_target, &new_name) {
                    if old_child == source_node {
                        return Ok(());
                    }
                    if !replace_existing {
                        return Err(CowFSFileAlreadyExistsException::new(target.to_string()));
                    }
                    tree.remove_child(folder_target, &new_name);
                }
                let source_parent = tree.node(source_node).parent.expect("parent");
                let source_name = tree.node(source_node).name().expect("name").to_string();
                tree.remove_child(source_parent, &source_name);
                tree.node_mut(source_node).set_path(target);
                tree.add_child(folder_target, source_node);
                Ok(())
            }
            _ => Err(CowFSNoSuchFileException::new(target.to_string())),
        }
    }

    pub fn check_access(&self, path: &CopyOnWriteFSPath) -> io::Result<()> {
        let tree = self.fs.lock_tree();
        let node = tree.by_path(path)?;
        let check_path = match &tree.node(node).kind {
            NodeKind::Directory(_) => self.fs.tmp_directory().to_path_buf(),
            NodeKind::File(file_node) => file_node.storage_path.clone(),
        };
        std::fs::metadata(&check_path).map(|_| ()).map_err(|e| crate::util::java_nio::for_path(e, &check_path))
    }

    pub fn read_attributes(&self, path: &CopyOnWriteFSPath) -> io::Result<BasicFileAttributes> {
        let tree = self.fs.lock_tree();
        let node = tree.by_path(path)?;
        match &tree.node(node).kind {
            NodeKind::Directory(_) => Ok(BasicFileAttributes { symbolic_link: false, directory: true, regular_file: false }),
            NodeKind::File(file_node) => BasicFileAttributes::read(&file_node.storage_path).map_err(|e| crate::util::java_nio::for_path(e, &file_node.storage_path)),
        }
    }

    pub fn get_real_path(&self, path: &CopyOnWriteFSPath) -> io::Result<CopyOnWriteFSPath> {
        let tree = self.fs.lock_tree();
        let node = tree.by_path(&path.to_absolute_path())?;
        Ok(CopyOnWriteFSPath::from_names(self.fs.clone(), tree.node(node).path.clone(), true))
    }
}
