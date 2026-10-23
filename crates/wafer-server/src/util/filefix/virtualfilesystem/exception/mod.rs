use std::io;

use crate::util::java_nio::java_io_error;

macro_rules! cow_fs_exception {
    ($name:ident, $kind:ident, $class:literal) => {
        pub struct $name;

        impl $name {
            pub fn new(message: impl Into<String>) -> io::Error {
                java_io_error(io::ErrorKind::$kind, concat!("net.minecraft.util.filefix.virtualfilesystem.exception.", $class), message.into())
            }
        }
    };
}

cow_fs_exception!(CowFSCreationException, Other, "CowFSCreationException");
cow_fs_exception!(CowFSDirectoryNotEmptyException, DirectoryNotEmpty, "CowFSDirectoryNotEmptyException");
cow_fs_exception!(CowFSFileAlreadyExistsException, AlreadyExists, "CowFSFileAlreadyExistsException");
cow_fs_exception!(CowFSFileSystemException, Other, "CowFSFileSystemException");
cow_fs_exception!(CowFSNoSuchFileException, NotFound, "CowFSNoSuchFileException");
cow_fs_exception!(CowFSNotDirectoryException, NotADirectory, "CowFSNotDirectoryException");
cow_fs_exception!(CowFSSymlinkException, Other, "CowFSSymlinkException");
