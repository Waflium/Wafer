use std::collections::{BTreeMap, HashMap};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, RwLock};

use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;

use super::structure_template::StructureTemplate;
use crate::server::packs::Resource;
use crate::server::packs::repository::{PackRepository, VANILLA_ID};
use crate::server::packs::resources::MultiPackResourceManager;
use crate::util::file_util;
use crate::world::level::chunk::storage::{decode_nbt, encode_nbt};

const STRUCTURE_DIRECTORY: &str = "structure";
const STRUCTURE_FILE_EXTENSION: &str = ".nbt";

static RESOURCES: RwLock<Option<Arc<BTreeMap<String, Resource>>>> = RwLock::new(None);
static VANILLA_RESOURCES: LazyLock<Arc<BTreeMap<String, Resource>>> = LazyLock::new(|| {
    let mut repository = PackRepository::create_vanilla_trusted_repository();
    repository.reload();
    repository.set_selected(&[VANILLA_ID]);
    Arc::new(MultiPackResourceManager::new(repository.open_all_selected()).list_resources(STRUCTURE_DIRECTORY, STRUCTURE_FILE_EXTENSION))
});
static GENERATED_DIR: RwLock<Option<PathBuf>> = RwLock::new(None);
static STRUCTURE_REPOSITORY: LazyLock<RwLock<HashMap<String, Option<Arc<StructureTemplate>>>>> = LazyLock::new(|| RwLock::new(HashMap::new()));

pub fn set_generated_dir(generated_dir: Option<PathBuf>) {
    *GENERATED_DIR.write().expect("generated dir") = generated_dir;
    STRUCTURE_REPOSITORY.write().expect("structure repository").clear();
}

pub fn on_resource_manager_reload(resource_manager: &MultiPackResourceManager) {
    *RESOURCES.write().expect("structure resources") = Some(Arc::new(resource_manager.list_resources(STRUCTURE_DIRECTORY, STRUCTURE_FILE_EXTENSION)));
    STRUCTURE_REPOSITORY.write().expect("structure repository").clear();
}

fn resources() -> Arc<BTreeMap<String, Resource>> {
    RESOURCES.read().expect("structure resources").clone().unwrap_or_else(|| VANILLA_RESOURCES.clone())
}

fn generated_dir() -> Option<PathBuf> {
    GENERATED_DIR.read().expect("generated dir").clone()
}

pub fn list_templates() -> Vec<String> {
    let mut templates = Vec::new();
    if let Some(generated_dir) = generated_dir() {
        list_directory(&generated_dir, &mut templates);
    }
    for location in resources().keys() {
        let (namespace, path) = location.split_once(':').unwrap();
        let id = format!("{namespace}:{}", &path[STRUCTURE_DIRECTORY.len() + 1..path.len() - STRUCTURE_FILE_EXTENSION.len()]);
        if !templates.contains(&id) {
            templates.push(id);
        }
    }
    templates
}

fn list_directory(source_dir: &Path, templates: &mut Vec<String>) {
    let Ok(namespaces) = std::fs::read_dir(source_dir) else { return };
    for namespace in namespaces.flatten().filter(|entry| entry.path().is_dir()) {
        let namespace = namespace.file_name().to_string_lossy().into_owned();
        if !crate::resources::identifier::is_valid_namespace(&namespace) {
            continue;
        }
        let top = source_dir.join(&namespace);
        let mut files = Vec::new();
        list_files(&top.join(STRUCTURE_DIRECTORY), &mut files);
        for file in files {
            let path: Vec<String> = file.strip_prefix(&top).unwrap().iter().map(|p| p.to_string_lossy().into_owned()).collect();
            let path = path.join("/");
            if let Some(path) = path.strip_prefix(&format!("{STRUCTURE_DIRECTORY}/")).and_then(|path| path.strip_suffix(STRUCTURE_FILE_EXTENSION))
                && crate::resources::identifier::is_valid_path(path)
            {
                let id = format!("{namespace}:{path}");
                if !templates.contains(&id) {
                    templates.push(id);
                }
            }
        }
    }
}

fn list_files(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            list_files(&path, files);
        } else {
            files.push(path);
        }
    }
}

pub fn get(id: &str) -> Option<Arc<StructureTemplate>> {
    if let Some(template) = STRUCTURE_REPOSITORY.read().expect("structure repository").get(id) {
        return template.clone();
    }
    let template = try_load(id).map(Arc::new);
    STRUCTURE_REPOSITORY.write().expect("structure repository").entry(id.to_string()).or_insert(template).clone()
}

pub fn get_or_create(id: &str) -> Arc<StructureTemplate> {
    if let Some(template) = get(id) {
        return template;
    }
    let template = Arc::new(StructureTemplate::empty());
    STRUCTURE_REPOSITORY.write().expect("structure repository").insert(id.to_string(), Some(template.clone()));
    template
}

pub fn put(id: &str, template: StructureTemplate) {
    STRUCTURE_REPOSITORY.write().expect("structure repository").insert(id.to_string(), Some(Arc::new(template)));
}

pub fn remove(id: &str) {
    STRUCTURE_REPOSITORY.write().expect("structure repository").remove(id);
}

pub fn save(id: &str) -> bool {
    let Some(Some(template)) = STRUCTURE_REPOSITORY.read().expect("structure repository").get(id).cloned() else { return false };
    let Some(generated_dir) = generated_dir() else { return false };
    let Some(file) = create_and_validate_path_to_structure(&generated_dir, id) else { return false };
    match save_to(&file, &template) {
        Ok(saved) => saved,
        Err(e) => {
            warn!("Failed to save structure file {id} to {}\n{e}", file.display());
            false
        }
    }
}

fn create_and_validate_path_to_structure(source_dir: &Path, id: &str) -> Option<PathBuf> {
    let (namespace, path) = id.split_once(':').unwrap_or(("minecraft", id));
    let file = format!("{STRUCTURE_DIRECTORY}/{path}{STRUCTURE_FILE_EXTENSION}");
    let decomposed_path = file_util::decompose_path(&file)?;
    if !decomposed_path.iter().all(|part| file_util::is_path_part_portable(part)) {
        return None;
    }
    Some(decomposed_path.iter().fold(source_dir.join(namespace), |path, part| path.join(part)))
}

fn save_to(file: &Path, template: &StructureTemplate) -> std::io::Result<bool> {
    let Some(parent) = file.parent() else { return Ok(false) };
    file_util::create_directories_safe(parent)?;
    let mut output = GzEncoder::new(std::fs::File::create(file)?, Compression::default());
    output.write_all(&encode_nbt(&template.save()))?;
    output.finish()?;
    Ok(true)
}

fn try_load(id: &str) -> Option<StructureTemplate> {
    let (namespace, path) = id.split_once(':').unwrap_or(("minecraft", id));
    let relative_file = format!("{STRUCTURE_DIRECTORY}/{path}{STRUCTURE_FILE_EXTENSION}");
    if let Some(generated_dir) = generated_dir()
        && generated_dir.is_dir()
        && let Some(file) = file_util::decompose_path(&relative_file)
            .map(|parts| parts.iter().fold(generated_dir.join(namespace), |path, part| path.join(part)))
            .filter(|file| file.is_file())
    {
        match std::fs::read(&file).map_err(|e| e.to_string()).and_then(|bytes| read_template(&bytes)) {
            Ok(template) => return Some(template),
            Err(e) => error!("Couldn't load structure from {}:{id}\n{e}", generated_dir.display()),
        }
    }
    let location = format!("{namespace}:{STRUCTURE_DIRECTORY}/{path}{STRUCTURE_FILE_EXTENSION}");
    let resource = resources().get(&location)?.clone();
    match resource.supplier.read().map_err(|e| e.to_string()).and_then(|bytes| read_template(&bytes)) {
        Ok(template) => Some(template),
        Err(e) => {
            error!("Couldn't load structure {id}\n{e}");
            None
        }
    }
}

fn read_template(bytes: &[u8]) -> Result<StructureTemplate, String> {
    let mut decompressed = Vec::new();
    GzDecoder::new(bytes).read_to_end(&mut decompressed).map_err(|e| e.to_string())?;
    let tag = decode_nbt(&decompressed).map_err(|e| e.to_string())?;
    let version = crate::nbt::get_data_version(&tag, 500);
    let tag = crate::util::datafix::data_fix_types::DataFixTypes::Structure.update_tag_to_current_version(tag, version);
    Ok(StructureTemplate::load(&tag))
}
