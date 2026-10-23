pub mod chunk_nbt;
pub mod compressed_nbt;
pub mod file_access;
pub mod file_access_provider;
pub mod file_relation;
pub mod file_resource_type;
pub mod file_resource_types;
pub mod level_dat;
pub mod player_data;
pub mod saved_data_nbt;

use std::io;

pub trait AutoCloseable: Send + Sync {
    fn close(&self) -> io::Result<()>;
}
