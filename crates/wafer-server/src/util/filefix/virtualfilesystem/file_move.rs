use std::path::{Component, Path, PathBuf};

use serde_json::{Map, Value};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileMove {
    from: PathBuf,
    to: PathBuf,
}

impl FileMove {
    pub fn new(from: PathBuf, to: PathBuf) -> FileMove {
        FileMove { from, to }
    }

    pub fn from(&self) -> &Path {
        &self.from
    }

    pub fn to(&self) -> &Path {
        &self.to
    }

    pub fn encode(&self, from_directory: &Path, to_directory: &Path) -> Result<Value, String> {
        let mut object = Map::new();
        object.insert("from".into(), Value::String(guarded_path_encode(from_directory, &self.from)?));
        object.insert("to".into(), Value::String(guarded_path_encode(to_directory, &self.to)?));
        Ok(Value::Object(object))
    }

    pub fn decode(value: &Value, from_directory: &Path, to_directory: &Path) -> Result<FileMove, String> {
        let Value::Object(object) = value else { return Err(format!("Not a map: {value}")) };
        let field = |name: &str, directory: &Path| match object.get(name) {
            Some(Value::String(text)) => guarded_path_decode(directory, text),
            Some(other) => Err(format!("Not a string: {other}")),
            None => Err(format!("No key {name} in MapLike[{}]", Value::Object(object.clone()))),
        };
        Ok(FileMove::new(field("from", from_directory)?, field("to", to_directory)?))
    }
}

fn guarded_path_encode(base_folder: &Path, path: &Path) -> Result<String, String> {
    let relative = path.strip_prefix(base_folder).map_err(|_| format!("Path {} does not start with base folder {}", path.display(), base_folder.display()))?;
    relative_normalized_sub_path_encode(relative)
}

fn relative_normalized_sub_path_encode(path: &Path) -> Result<String, String> {
    let normalized = normalize(path);
    validate_relative_sub_path(&normalized)?;
    Ok(normalized.display().to_string())
}

fn guarded_path_decode(base_folder: &Path, text: &str) -> Result<PathBuf, String> {
    let normalized = normalize(Path::new(text));
    validate_relative_sub_path(&normalized)?;
    let path = base_folder.join(normalized);
    if path.starts_with(base_folder) {
        Ok(path)
    } else {
        Err(format!("Path {} does not start with base folder {}", path.display(), base_folder.display()))
    }
}

fn normalize(path: &Path) -> PathBuf {
    let mut normalized: Vec<Component> = Vec::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir if normalized.last().is_some_and(|last| matches!(last, Component::Normal(_))) => {
                normalized.pop();
            }
            other => normalized.push(other),
        }
    }
    normalized.iter().collect()
}

fn validate_relative_sub_path(path: &Path) -> Result<(), String> {
    if path.has_root() || path.is_absolute() {
        return Err(format!("Illegal absolute path: {}", path.display()));
    }
    for component in path.components() {
        if matches!(component, Component::ParentDir | Component::CurDir) {
            return Err(format!("Illegal path traversal: {}", path.display()));
        }
    }
    if path.as_os_str().is_empty() {
        return Err(format!("Path is empty: {}", path.display()));
    }
    Ok(())
}
