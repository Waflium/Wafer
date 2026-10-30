use std::fs;
use std::path::PathBuf;

use wafer_protocol::nbt::Nbt;

use super::{WorldDir, create_temp_file, format_uuid, read_compressed, write_compressed};
use crate::util::datafix::data_fix_types::DataFixTypes;
use crate::world::level::chunk::storage::WRITE_COMPRESSED_PATH;

const SAVE_CALLERS: [&str; 4] = [
    WRITE_COMPRESSED_PATH[0],
    WRITE_COMPRESSED_PATH[1],
    "net.minecraft.world.level.storage.PlayerDataStorage.save(PlayerDataStorage.java:41)",
    "net.minecraft.server.players.PlayerList.save(PlayerList.java:310)",
];

pub struct PlayerDataStorage {
    player_dir: PathBuf,
}

impl PlayerDataStorage {
    pub fn new(level_access: &WorldDir) -> PlayerDataStorage {
        PlayerDataStorage { player_dir: level_access.root.join("players/data") }
    }

    pub fn save(&self, data_to_store: &Nbt, uuid: u128, plain_text_name: &str) {
        let saved = (|| {
            let string_uuid = format_uuid(uuid);
            let tmp_file = create_temp_file(&self.player_dir, &format!("{string_uuid}-"), ".dat")?;
            write_compressed(data_to_store, &tmp_file, &SAVE_CALLERS)?;
            let real_file = self.player_dir.join(format!("{string_uuid}.dat"));
            let old_file = self.player_dir.join(format!("{string_uuid}.dat_old"));
            crate::util::safe_replace_file(&real_file, &tmp_file, &old_file);
            std::io::Result::Ok(())
        })();
        if saved.is_err() {
            warn!("Failed to save player data for {plain_text_name}");
        }
    }

    fn backup(&self, name: &str, uuid: u128, suffix: &str) {
        let id_string = format_uuid(uuid);
        let real_path = self.player_dir.join(format!("{id_string}{suffix}"));
        let backup_path = self.player_dir.join(format!("{id_string}_corrupted_{}{suffix}", chrono::Local::now().format("%Y-%m-%d_%H-%M-%S")));
        if real_path.is_file() {
            let copied = fs::copy(&real_path, &backup_path).and_then(|_| {
                let modified = fs::metadata(&real_path)?.modified()?;
                fs::File::options().write(true).open(&backup_path)?.set_modified(modified)
            });
            if let Err(e) = copied {
                warn!("Failed to copy the player.dat file for {name}\n{e}");
            }
        }
    }

    fn load_with_suffix(&self, name: &str, uuid: u128, suffix: &str) -> Option<Nbt> {
        let real_file = self.player_dir.join(format!("{}{suffix}", format_uuid(uuid)));
        if real_file.is_file() {
            match read_compressed(&real_file) {
                Ok(tag) => return Some(tag),
                Err(_) => warn!("Failed to load player data for {name}"),
            }
        }
        None
    }

    pub fn load(&self, name: &str, uuid: u128) -> Option<Nbt> {
        let tag = self.load_with_suffix(name, uuid, ".dat");
        if tag.is_none() {
            self.backup(name, uuid, ".dat");
        }
        tag.or_else(|| self.load_with_suffix(name, uuid, ".dat_old")).map(|tag| {
            let version = crate::nbt::get_data_version(&tag, -1);
            DataFixTypes::Player.update_tag_to_current_version(tag, version)
        })
    }
}
