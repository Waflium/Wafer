use std::path::{Component, Path, PathBuf};

use serde_json::{Map, Value};

use crate::util::gson::{JsonElement, to_json_string};

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

    pub fn decode(value: &JsonElement, from_directory: &Path, to_directory: &Path) -> Result<FileMove, String> {
        let JsonElement::Object(_) = value else { return Err(format!("Not a JSON object: {}", to_json_string(value))) };
        let field = |name: &str, directory: &Path| match value.get(name) {
            Some(JsonElement::String(text)) => guarded_path_decode(directory, text),
            Some(JsonElement::Null) | None => Err(format!("No key {name} in MapLike[{}]", to_json_string(value))),
            Some(other) => Err(format!("Not a string: {}", to_json_string(other))),
        };
        match (field("from", from_directory), field("to", to_directory)) {
            (Ok(from), Ok(to)) => Ok(FileMove::new(from, to)),
            (Err(from), Err(to)) => Err(format!("{to}; {from}")),
            (Err(message), Ok(_)) | (Ok(_), Err(message)) => Err(message),
        }
    }

    pub fn decode_moves(value: Option<&JsonElement>, from_directory: &Path, to_directory: &Path) -> Result<Vec<FileMove>, String> {
        let Some(root @ JsonElement::Object(_)) = value else { return Err(format!("Not a JSON object: {}", value.map_or_else(|| "null".to_string(), to_json_string))) };
        let moves = match root.get("moves") {
            Some(JsonElement::Array(moves)) => moves,
            Some(JsonElement::Null) | None => return Err(format!("No key moves in MapLike[{}]", to_json_string(root))),
            Some(other) => return Err(format!("Not a json array: {}", to_json_string(other))),
        };
        let mut decoded = Vec::with_capacity(moves.len());
        let mut errors: Vec<String> = Vec::new();
        for file_move in moves {
            match FileMove::decode(file_move, from_directory, to_directory) {
                Ok(file_move) => decoded.push(file_move),
                Err(message) => errors.insert(0, message),
            }
        }
        if errors.is_empty() { Ok(decoded) } else { Err(errors.join("; ")) }
    }
}

fn guarded_path_encode(base_folder: &Path, path: &Path) -> Result<String, String> {
    let relative = path.strip_prefix(base_folder).map_err(|_| format!("Path {} does not start with base folder {}", path.display(), base_folder.display()))?;
    relative_normalized_sub_path_encode(relative)
}

fn relative_normalized_sub_path_encode(path: &Path) -> Result<String, String> {
    let normalized = normalize(path);
    validate_relative_sub_path(&normalized)?;
    Ok(normalized.display().to_string().replace('\\', "/"))
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
    if path.is_absolute() {
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
