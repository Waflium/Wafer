use crate::util::filefix::access::file_relation::FileRelation;

use super::apply_in_folders::ApplyInFolders;
use super::delete_file_or_empty_directory::DeleteFileOrEmptyDirectory;
use super::file_fix_operation::FileFixOperation;
use super::group_move::GroupMove;
use super::move_::Move;
use super::regex_move::RegexMove;

pub struct FileFixOperations;

impl FileFixOperations {
    pub fn move_simple(file: &str) -> Move {
        Move::new(file, file)
    }

    pub fn move_(from: &str, to: &str) -> Move {
        Move::new(from, to)
    }

    pub fn move_regex(file_pattern: &str, replace_pattern: &str) -> RegexMove {
        RegexMove::new(file_pattern, replace_pattern)
    }

    pub fn delete(target: &str) -> DeleteFileOrEmptyDirectory {
        DeleteFileOrEmptyDirectory::new(target)
    }

    pub fn apply_in_folders(applicable_folders: FileRelation, operations: Vec<Box<dyn FileFixOperation>>) -> ApplyInFolders {
        ApplyInFolders::new(applicable_folders, operations)
    }

    pub fn group_move(data: &[(&str, &str)], move_: Vec<Move>) -> GroupMove {
        GroupMove::new(data.iter().map(|(from, to)| (from.to_string(), to.to_string())).collect(), move_)
    }
}
