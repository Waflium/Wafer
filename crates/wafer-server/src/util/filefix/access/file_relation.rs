use std::sync::Arc;

use crate::util::filefix::virtualfilesystem::copy_on_write_fs_path::CopyOnWriteFSPath;
use crate::util::filefix::virtualfilesystem::files;

type GetPaths = dyn Fn(&CopyOnWriteFSPath) -> Vec<CopyOnWriteFSPath> + Send + Sync;

#[derive(Clone)]
pub struct FileRelation(Arc<GetPaths>);

impl FileRelation {
    pub fn new(get_paths: impl Fn(&CopyOnWriteFSPath) -> Vec<CopyOnWriteFSPath> + Send + Sync + 'static) -> FileRelation {
        FileRelation(Arc::new(get_paths))
    }

    pub fn get_paths(&self, base_path: &CopyOnWriteFSPath) -> Vec<CopyOnWriteFSPath> {
        (self.0)(base_path)
    }

    pub fn origin() -> FileRelation {
        FileRelation::new(|base_path| vec![base_path.clone()])
    }

    pub fn region() -> FileRelation {
        FileRelation::new(|base_path| vec![base_path.resolve_str("region")])
    }

    pub fn data() -> FileRelation {
        FileRelation::new(|base_path| vec![base_path.resolve_str("data")])
    }

    pub fn player_data() -> FileRelation {
        FileRelation::new(|base_path| vec![base_path.resolve_str("players/data")])
    }

    pub fn dimensions() -> FileRelation {
        FileRelation::new(discover_dimensions)
    }

    pub fn dimensions_data() -> FileRelation {
        FileRelation::dimensions().resolve(&FileRelation::data())
    }

    pub fn generated_namespaces() -> FileRelation {
        FileRelation::origin().resolve(&FileRelation::new(|base_path| directories_in_path(&base_path.resolve_str("generated"))))
    }

    pub fn old_overworld() -> FileRelation {
        FileRelation::origin()
    }

    pub fn old_nether() -> FileRelation {
        FileRelation::origin().resolve(&FileRelation::new(|base_path| vec![base_path.resolve_str("DIM-1")]))
    }

    pub fn old_end() -> FileRelation {
        FileRelation::origin().resolve(&FileRelation::new(|base_path| vec![base_path.resolve_str("DIM1")]))
    }

    pub fn for_data_file_in_dimension(dimension: &str, file_name: &str) -> FileRelation {
        let path = format!("dimensions/minecraft/{dimension}/data/{file_name}");
        FileRelation::new(move |base_path| vec![base_path.resolve_str(&path)])
    }

    pub fn for_file(&self, file_name: &str) -> FileRelation {
        let file_name = file_name.to_string();
        self.resolve(&FileRelation::new(move |base_path| vec![base_path.resolve_str(&file_name)]))
    }

    pub fn resolve(&self, other: &FileRelation) -> FileRelation {
        let this = self.clone();
        let other = other.clone();
        FileRelation::new(move |base_path| this.get_paths(base_path).iter().flat_map(|path| other.get_paths(path)).collect())
    }

    pub fn join(&self, relations: &[FileRelation]) -> FileRelation {
        let relations = relations.to_vec();
        FileRelation::new(move |base_path| {
            let mut paths: Vec<CopyOnWriteFSPath> = Vec::new();
            for relation in &relations {
                for path in relation.get_paths(base_path) {
                    if !paths.contains(&path) {
                        paths.push(path);
                    }
                }
            }
            paths
        })
    }
}

fn discover_dimensions(base_path: &CopyOnWriteFSPath) -> Vec<CopyOnWriteFSPath> {
    let dimensions_root = base_path.resolve_str("dimensions");
    if !files::exists(&dimensions_root) {
        return get_default_dimensions(base_path);
    }
    match files::list(&dimensions_root) {
        Ok(namespace_paths) => {
            let discovered_dimensions: Vec<CopyOnWriteFSPath> =
                namespace_paths.iter().filter(|path| files::is_directory(path)).flat_map(directories_in_path).collect();
            if discovered_dimensions.is_empty() { get_default_dimensions(base_path) } else { discovered_dimensions }
        }
        Err(e) => {
            warn!("Failed to discover dimensions, assuming default: {e}");
            get_default_dimensions(base_path)
        }
    }
}

pub fn directories_in_path(path: &CopyOnWriteFSPath) -> Vec<CopyOnWriteFSPath> {
    match files::list(path) {
        Ok(dimension_paths) => dimension_paths.into_iter().filter(|path| files::is_directory(path)).collect(),
        Err(_) => Vec::new(),
    }
}

fn get_default_dimensions(base_path: &CopyOnWriteFSPath) -> Vec<CopyOnWriteFSPath> {
    vec![
        base_path.resolve_str("dimensions/minecraft/overworld"),
        base_path.resolve_str("dimensions/minecraft/the_nether"),
        base_path.resolve_str("dimensions/minecraft/the_end"),
    ]
}
