use std::io;
use std::path::PathBuf;

use wafer_core::random::java_string_hash;
use wafer_data::WORLD_VERSION;
use wafer_protocol::nbt::{Nbt, TagMap};

use super::{read_saved_data_dat, write_dat};
use crate::util::datafix::data_fix_types::DataFixTypes;
use crate::util::java_collections::JavaHashMap;

pub struct CommandStorage {
    dir: Option<PathBuf>,
    namespaces: JavaHashMap<Container>,
    missing: Vec<String>,
}

struct Container {
    storage: JavaHashMap<TagMap>,
    dirty: bool,
}

impl Default for Container {
    fn default() -> Container {
        Container { storage: JavaHashMap::new(java_string_hash), dirty: false }
    }
}

impl CommandStorage {
    pub fn new(dir: Option<PathBuf>) -> CommandStorage {
        CommandStorage { dir, namespaces: JavaHashMap::new(java_string_hash), missing: Vec::new() }
    }

    fn file(&self, namespace: &str) -> Option<PathBuf> {
        Some(self.dir.as_ref()?.join(namespace).join("command_storage.dat"))
    }

    fn container(&mut self, namespace: &str) -> Option<&mut Container> {
        if !self.namespaces.contains_key(namespace) && !self.missing.iter().any(|missing| missing == namespace) {
            let loaded = self.file(namespace).and_then(|file| match read_saved_data_dat(&file, DataFixTypes::SavedDataCommandStorage) {
                Ok(tag) => tag.and_then(|t| Container::from_nbt(t.get("data")?)),
                Err(_) => {
                    error!("Error loading saved data: SavedDataType[{namespace}:command_storage]");
                    None
                }
            });
            match loaded {
                Some(container) => drop(self.namespaces.insert(namespace.to_string(), container)),
                None => self.missing.push(namespace.to_string()),
            }
        }
        self.namespaces.get_mut(namespace)
    }

    pub fn get(&mut self, id: &str) -> TagMap {
        let (namespace, path) = id.split_once(':').unwrap();
        self.container(namespace).and_then(|c| c.storage.get(path)).cloned().unwrap_or_default()
    }

    pub fn write_back(&mut self, id: &str, contents: TagMap) {
        let (namespace, path) = id.split_once(':').unwrap();
        if let Some(entry) = self.container(namespace).and_then(|c| c.storage.get_mut(path)) {
            *entry = contents;
        }
    }

    pub fn set(&mut self, id: &str, contents: TagMap) {
        let (namespace, path) = id.split_once(':').unwrap();
        if self.container(namespace).is_none() {
            self.missing.retain(|missing| missing != namespace);
            self.namespaces.insert(namespace.to_string(), Container::default());
        }
        let container = self.container(namespace).unwrap();
        if contents.is_empty() {
            container.storage.remove(path);
        } else {
            container.storage.insert(path.to_string(), contents);
        }
        container.dirty = true;
    }

    pub fn keys(&self) -> Vec<String> {
        self.namespaces.iter().flat_map(|(ns, c)| c.storage.iter().map(move |(p, _)| format!("{ns}:{p}"))).collect()
    }

    pub fn save(&mut self) -> io::Result<()> {
        let Some(dir) = self.dir.clone() else { return Ok(()) };
        let dirty: Vec<String> = self.namespaces.iter().filter(|(_, container)| container.dirty).map(|(namespace, _)| namespace.to_string()).collect();
        for namespace in dirty {
            let container = self.namespaces.get_mut(&namespace).expect("a dirty container");
            let file = Nbt::Compound(vec![("data".into(), container.to_nbt()), ("DataVersion".into(), Nbt::Int(WORLD_VERSION))].into());
            write_dat(&dir.join(namespace.as_str()).join("command_storage.dat"), &file)?;
            container.dirty = false;
        }
        Ok(())
    }
}

impl Container {
    fn from_nbt(data: &Nbt) -> Option<Container> {
        let Some(Nbt::Compound(contents)) = data.get("contents") else { return None };
        let storage = contents.iteration().filter_map(|(path, tag)| match tag {
            Nbt::Compound(fields) => Some((path.clone(), fields.copy())),
            _ => None,
        });
        Some(Container { storage: JavaHashMap::copy_of(java_string_hash, storage), dirty: false })
    }

    fn to_nbt(&self) -> Nbt {
        let contents = self.storage.iter().map(|(path, fields)| (path.to_string(), Nbt::Compound(fields.copy()))).collect();
        Nbt::Compound(vec![("contents".into(), Nbt::Compound(contents))].into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_per_namespace() {
        let dir = std::env::temp_dir().join(format!("wafer-command-storage-{}", std::process::id()));
        let mut storage = CommandStorage::new(Some(dir.clone()));
        storage.set("a:x", vec![("v".into(), Nbt::Int(1))].into());
        storage.set("b:y/z", vec![("w".into(), Nbt::String("s".into()))].into());
        storage.save().unwrap();
        assert!(dir.join("a/command_storage.dat").exists() && dir.join("b/command_storage.dat").exists());
        let mut loaded = CommandStorage::new(Some(dir.clone()));
        assert_eq!(Nbt::Compound(loaded.get("a:x")), Nbt::Compound(vec![("v".to_string(), Nbt::Int(1))].into()));
        assert_eq!(Nbt::Compound(loaded.get("b:y/z")), Nbt::Compound(vec![("w".to_string(), Nbt::String("s".into()))].into()));
        assert_eq!(Nbt::Compound(loaded.get("c:none")), Nbt::Compound(TagMap::new()));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn keys_follow_hash_map_order() {
        let mut storage = CommandStorage::new(None);
        for path in ["x", "y", "z", "w"] {
            storage.set(&format!("a:{path}"), vec![("v".into(), Nbt::Int(1))].into());
        }
        assert_eq!(storage.keys(), ["a:w", "a:x", "a:y", "a:z"]);
    }
}
