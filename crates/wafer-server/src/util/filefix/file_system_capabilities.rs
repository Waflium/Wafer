use std::fmt;

use crate::util::java_nio::CopyOption;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FileSystemCapabilities {
    pub atomic_move: bool,
    pub hard_links: bool,
}

impl FileSystemCapabilities {
    pub fn new(atomic_move: bool, hard_links: bool) -> FileSystemCapabilities {
        FileSystemCapabilities { atomic_move, hard_links }
    }

    pub fn atomic_move(&self) -> bool {
        self.atomic_move
    }

    pub fn hard_links(&self) -> bool {
        self.hard_links
    }

    pub fn get_move_options(&self) -> Vec<CopyOption> {
        if self.atomic_move { vec![CopyOption::AtomicMove, CopyOption::NoFollowLinks] } else { vec![CopyOption::NoFollowLinks] }
    }
}

impl fmt::Display for FileSystemCapabilities {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FileSystemCapabilities[atomicMove={}, hardLinks={}]", self.atomic_move, self.hard_links)
    }
}
