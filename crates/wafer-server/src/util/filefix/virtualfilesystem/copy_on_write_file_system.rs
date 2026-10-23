use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use crate::util::basic_file_attributes::BasicFileAttributes;

use super::copy_on_write_file_store::CopyOnWriteFileStore;
use super::copy_on_write_fs_path::CopyOnWriteFSPath;
use crate::util::java_nio::CopyOption;

use super::copy_on_write_fs_provider::CopyOnWriteFSProvider;
use super::directory_node::{DirectoryNode, FileTree};
use super::exception::{CowFSCreationException, CowFSSymlinkException};
use super::file_move::FileMove;
use super::file_node::FileNode;
use super::node::{Node, NodeKind};

const FILE_ATTRIBUTE_VIEWS: [&str; 1] = ["basic"];

struct Shared {
    name: String,
    base_directory: PathBuf,
    skipped_paths: Box<dyn Fn(&Path) -> bool + Send + Sync>,
    tmp_directory: PathBuf,
    tmp_file_index: AtomicI32,
    file_tree: Mutex<FileTree>,
}

#[derive(Clone)]
pub struct CopyOnWriteFileSystem {
    shared: Arc<Shared>,
}

impl PartialEq for CopyOnWriteFileSystem {
    fn eq(&self, other: &CopyOnWriteFileSystem) -> bool {
        Arc::ptr_eq(&self.shared, &other.shared)
    }
}

impl Eq for CopyOnWriteFileSystem {}

pub struct Moves {
    pub directories: Vec<PathBuf>,
    pub copied_files: Vec<FileMove>,
    pub preexisting_files: Vec<FileMove>,
}

impl CopyOnWriteFileSystem {
    pub fn create(
        name: &str,
        base_directory: &Path,
        tmp_directory: &Path,
        skipped_paths: impl Fn(&Path) -> bool + Send + Sync + 'static,
    ) -> io::Result<CopyOnWriteFileSystem> {
        if tmp_directory.exists() {
            return Err(CowFSCreationException::new(format!("Temporary directory already exists: {}", tmp_directory.display())));
        }
        let file_system = CopyOnWriteFileSystem::new(name, base_directory, tmp_directory, Box::new(skipped_paths))?;
        std::fs::create_dir(tmp_directory)?;
        Ok(file_system)
    }

    fn new(name: &str, base_directory: &Path, tmp_directory: &Path, skipped_paths: Box<dyn Fn(&Path) -> bool + Send + Sync>) -> io::Result<CopyOnWriteFileSystem> {
        let placeholder = FileTree::empty();
        let shared = Arc::new(Shared {
            name: name.to_string(),
            base_directory: base_directory.to_path_buf(),
            skipped_paths,
            tmp_directory: tmp_directory.to_path_buf(),
            tmp_file_index: AtomicI32::new(0),
            file_tree: Mutex::new(placeholder),
        });
        let file_system = CopyOnWriteFileSystem { shared };
        let file_tree = file_system.build_file_tree_from(base_directory)?;
        *file_system.lock_tree() = file_tree;
        Ok(file_system)
    }

    fn build_file_tree_from(&self, base_directory: &Path) -> io::Result<FileTree> {
        let mut file_tree = FileTree::new(&self.root_path());
        self.walk_directory(&mut file_tree, base_directory, base_directory)?;
        Ok(file_tree)
    }

    fn walk_directory(&self, file_tree: &mut FileTree, real_path: &Path, base_directory: &Path) -> io::Result<()> {
        check_attributes(real_path, &BasicFileAttributes::read(real_path)?)?;
        if (self.shared.skipped_paths)(real_path) {
            return Ok(());
        }
        if real_path != base_directory {
            let cow_path = self.to_cow_path(real_path, base_directory);
            let parent_node = file_tree.directory_by_path(&cow_path.get_parent().expect("parent"))?;
            let child = file_tree.alloc(Node::new(&cow_path, NodeKind::Directory(DirectoryNode::new())));
            file_tree.add_child(parent_node, child);
        }
        for entry in std::fs::read_dir(real_path)? {
            let child_path = entry?.path();
            let attributes = BasicFileAttributes::read(&child_path)?;
            if attributes.directory {
                self.walk_directory(file_tree, &child_path, base_directory)?;
            } else {
                self.visit_file(file_tree, &child_path, &attributes, base_directory)?;
            }
        }
        Ok(())
    }

    fn visit_file(&self, file_tree: &mut FileTree, real_path: &Path, attributes: &BasicFileAttributes, base_directory: &Path) -> io::Result<()> {
        check_attributes(real_path, attributes)?;
        if (self.shared.skipped_paths)(real_path) {
            return Ok(());
        }
        let cow_path = self.to_cow_path(real_path, base_directory);
        let parent_node = file_tree.directory_by_path(&cow_path.get_parent().expect("parent"))?;
        let child = file_tree.alloc(Node::new(&cow_path, NodeKind::File(FileNode::new(real_path.to_path_buf(), false))));
        file_tree.add_child(parent_node, child);
        Ok(())
    }

    fn to_cow_path(&self, real_path: &Path, base_directory: &Path) -> CopyOnWriteFSPath {
        let relative = real_path.strip_prefix(base_directory).unwrap_or(real_path);
        self.root_path().resolve_str(&relative.display().to_string())
    }

    pub fn provider(&self) -> CopyOnWriteFSProvider {
        CopyOnWriteFSProvider::new(self.clone())
    }

    pub fn close(&self) -> io::Result<()> {
        if self.shared.tmp_directory.exists() {
            std::fs::remove_dir_all(&self.shared.tmp_directory)?;
        }
        Ok(())
    }

    pub fn is_open(&self) -> bool {
        true
    }

    pub fn is_read_only(&self) -> bool {
        false
    }

    pub fn get_separator(&self) -> &'static str {
        std::path::MAIN_SEPARATOR_STR
    }

    pub fn get_root_directories(&self) -> Vec<CopyOnWriteFSPath> {
        vec![self.root_path()]
    }

    pub fn get_file_stores(&self) -> Vec<CopyOnWriteFileStore> {
        vec![self.store()]
    }

    pub fn supported_file_attribute_views(&self) -> &'static [&'static str] {
        &FILE_ATTRIBUTE_VIEWS
    }

    pub fn get_path(&self, first: &str, more: &[&str]) -> CopyOnWriteFSPath {
        CopyOnWriteFSPath::of(self.clone(), first, more)
    }

    pub fn store(&self) -> CopyOnWriteFileStore {
        CopyOnWriteFileStore::new(self.shared.name.clone(), self.clone())
    }

    pub fn root_path(&self) -> CopyOnWriteFSPath {
        CopyOnWriteFSPath::from_names(self.clone(), Vec::new(), true)
    }

    pub(crate) fn lock_tree(&self) -> MutexGuard<'_, FileTree> {
        self.shared.file_tree.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn base_directory(&self) -> &Path {
        &self.shared.base_directory
    }

    pub fn tmp_directory(&self) -> &Path {
        &self.shared.tmp_directory
    }

    pub(crate) fn create_temporary_file_path(&self) -> PathBuf {
        self.shared.tmp_directory.join(format!("tmp_{}", self.shared.tmp_file_index.fetch_add(1, Ordering::SeqCst) + 1))
    }

    pub fn collect_move_operations(&self, out_path: &Path) -> Moves {
        let mut result = Moves { directories: Vec::new(), copied_files: Vec::new(), preexisting_files: Vec::new() };
        let tree = self.lock_tree();
        collect_move_operations(&tree, out_path, tree.root(), &mut result);
        result
    }

    pub fn create_directories(directories: &[PathBuf]) -> io::Result<()> {
        for directory in directories {
            std::fs::create_dir(directory)?;
        }
        Ok(())
    }

    pub fn hard_link_files(moves: &[FileMove]) -> io::Result<()> {
        for file_move in moves {
            if !file_move.to().exists() {
                if !is_regular_file(file_move.from()) {
                    return Err(io::Error::other(format!("java.lang.IllegalStateException: Not a regular file: {}", file_move.from().display())));
                }
                std::fs::hard_link(file_move.from(), file_move.to())?;
            }
        }
        Ok(())
    }

    pub fn move_files(moves: &[FileMove]) -> io::Result<()> {
        for file_move in moves {
            crate::util::java_nio::move_file(file_move.from(), file_move.to(), &[])?;
        }
        Ok(())
    }

    pub fn move_files_with_retry(moves: &[FileMove], options: &[CopyOption]) -> io::Result<()> {
        for file_move in moves {
            if file_move.from().exists() || !file_move.to().exists() {
                if !is_regular_file(file_move.from()) {
                    return Err(io::Error::other(format!("java.io.IOException: Not a regular file: {}", file_move.from().display())));
                }
                crate::util::java_nio::move_file(file_move.from(), file_move.to(), options)?;
            }
        }
        Ok(())
    }

    pub fn try_revert_moves(moves: &[FileMove], options: &[CopyOption]) -> Vec<FileMove> {
        let mut failed_moves = Vec::new();
        for file_move in moves {
            if file_move.to().exists() || !file_move.from().exists() {
                if is_regular_file(file_move.to()) {
                    let success = crate::util::safe_move_file(file_move.to(), file_move.from(), options);
                    if success {
                        info!("Reverted move from {} to {}", file_move.from().display(), file_move.to().display());
                    } else {
                        error!("Failed to revert move from {} to {}", file_move.from().display(), file_move.to().display());
                        failed_moves.push(file_move.clone());
                    }
                } else {
                    error!("Skipping reverting move from {} to {} as it's not a file", file_move.from().display(), file_move.to().display());
                    failed_moves.push(file_move.clone());
                }
            }
        }
        if failed_moves.is_empty() {
            info!("Successfully reverted back to previous world state");
        } else {
            error!("Completed reverting with errors");
        }
        failed_moves
    }
}

fn is_regular_file(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|metadata| metadata.is_file())
}

fn is_writable(path: &Path) -> bool {
    match std::fs::metadata(path) {
        Ok(metadata) => metadata.is_dir() || !metadata.permissions().readonly(),
        Err(_) => false,
    }
}

fn check_attributes(real_path: &Path, attributes: &BasicFileAttributes) -> io::Result<()> {
    if !attributes.regular_file && !attributes.directory {
        return Err(CowFSSymlinkException::new(format!("Cannot build copy-on-write file system when symlink is present: {}", real_path.display())));
    }
    if !is_writable(real_path) {
        return Err(CowFSCreationException::new(format!("Cannot build copy-on-write file system, missing write access for file: {}", real_path.display())));
    }
    Ok(())
}

fn collect_move_operations(tree: &FileTree, out_path: &Path, folder: super::node::NodeId, result: &mut Moves) {
    for child_node in tree.children(folder) {
        let node = tree.node(child_node);
        let target = out_path.join(node.name().expect("name"));
        match &node.kind {
            NodeKind::File(file_node) => {
                let file_move = FileMove::new(file_node.storage_path.clone(), target);
                if file_node.is_copy {
                    result.copied_files.push(file_move);
                } else {
                    result.preexisting_files.push(file_move);
                }
            }
            NodeKind::Directory(_) => {
                result.directories.push(target.clone());
                collect_move_operations(tree, &target, child_node, result);
            }
        }
    }
}
