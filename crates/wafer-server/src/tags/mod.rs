pub mod tag_entry;

use std::collections::HashMap;
use std::fmt;

use wafer_protocol::nbt::Nbt;

use self::tag_entry::{Lookup, TagEntry};
use crate::resources::registry_load_task::{self, TagLoadContext};
use crate::server::packs::resources::MultiPackResourceManager;
use crate::util::codec::{self, Record};
use crate::util::dependency_sorter::{DependencySorter, Entry};
use crate::util::java_throwable::JavaThrowable;
use crate::util::java_collections::{JavaHashMap, identifier_hash};

pub struct EntryWithSource {
    pub entry: TagEntry,
    pub source: String,
}

impl fmt::Display for EntryWithSource {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} (from {})", self.entry, self.source)
    }
}

struct SortingEntry<'a>(&'a [EntryWithSource]);

impl Entry for SortingEntry<'_> {
    fn visit_required_dependencies(&self, output: &mut dyn FnMut(&str)) {
        self.0.iter().for_each(|e| e.entry.visit_required_dependencies(output));
    }

    fn visit_optional_dependencies(&self, output: &mut dyn FnMut(&str)) {
        self.0.iter().for_each(|e| e.entry.visit_optional_dependencies(output));
    }
}

pub struct TagLoader {
    context: Option<TagLoadContext>,
    directory: String,
}

struct TagFile {
    entries: Vec<TagEntry>,
    replace: bool,
}

impl TagFile {
    fn decode(value: &Nbt) -> Result<TagFile, String> {
        codec::map(value)?;
        let mut record = Record::default();
        let entries = record.required(value, "values", |values| codec::list_partial(values, TagEntry::decode_partial));
        let replace = record.with_default(value, "replace", false, codec::bool_value);
        record.build(|| Some(TagFile { entries: entries?, replace })).into_result()
    }
}

pub type PendingTags = Vec<(String, Vec<(String, Vec<i32>)>)>;

pub fn tags_dir_path(registry: &str) -> String {
    match registry.split_once(':') {
        Some(("minecraft", path)) => format!("tags/{path}"),
        Some((namespace, path)) => format!("tags/{namespace}/{path}"),
        None => format!("tags/{registry}"),
    }
}

#[derive(Clone, Copy)]
pub enum ExistingTagLoading {
    Startup,
    Reload(&'static [&'static str]),
}

pub fn load_tags_for_existing_registries(resource_manager: &MultiPackResourceManager, loading: ExistingTagLoading) -> PendingTags {
    let mut registries: Vec<&str> = wafer_data::registry_names().chain(crate::core::data_registries::dynamic_registry_names()).collect();
    registries.sort_unstable();
    registries.dedup();
    registries
        .into_iter()
        .filter(|&registry| !(matches!(loading, ExistingTagLoading::Startup) && crate::core::data_registries::is_dimension_registry(registry)))
        .filter_map(|registry| load_tags_for_registry(resource_manager, registry, loading))
        .collect()
}

pub fn load_tags_for_dimension_registry(resource_manager: &MultiPackResourceManager) {
    load_tags_for_registry(resource_manager, crate::core::data_registries::DIMENSION_REGISTRY, ExistingTagLoading::Startup);
}

fn load_tags_for_registry(resource_manager: &MultiPackResourceManager, registry: &'static str, loading: ExistingTagLoading) -> Option<(String, Vec<(String, Vec<i32>)>)> {
    let entries = wafer_data::registry_entries(registry).unwrap_or_default();
    let ids: HashMap<&str, i32> = entries.iter().enumerate().map(|(id, &entry)| (entry, id as i32)).collect();
    let dynamic = crate::core::data_registries::is_dynamic_registry(registry);
    let context = match loading {
        ExistingTagLoading::Startup if dynamic => TagLoadContext::DynamicRegistry,
        ExistingTagLoading::Startup => TagLoadContext::ExistingRegistries,
        ExistingTagLoading::Reload(invoker) => TagLoadContext::ReloadedRegistries(invoker),
    };
    let startup_dynamic = dynamic && matches!(loading, ExistingTagLoading::Startup);
    let loader = TagLoader::with_context(&tags_dir_path(registry), Some(context));
    let lookup = |name: &str, required: bool| match ids.get(name) {
        Some(&id) => {
            if startup_dynamic && required {
                crate::core::data_registries::record_tag_reference(registry, name);
            }
            Some(id)
        }
        None if startup_dynamic && required => {
            crate::core::data_registries::record_unbound_tag_value(registry, name);
            Some(-1)
        }
        None => None,
    };
    let tags = loader.build(&loader.load(resource_manager), &lookup, &|a, b| a == b);
    if tags.len() == 0 {
        return None;
    }
    let mut tags: Vec<(String, Vec<i32>)> = tags.iter().map(|(id, values)| (id.to_string(), values.clone())).collect();
    tags.sort_unstable_by(|a, b| a.0.cmp(&b.0));
    Some((registry.to_string(), tags))
}

pub fn load_tags_for_loaded_registry(resource_manager: &MultiPackResourceManager, registry: &str, is_element: &dyn Fn(&str) -> bool, mut record_unbound: impl FnMut(&str)) {
    let loader = TagLoader::with_context(&tags_dir_path(registry), Some(TagLoadContext::DynamicRegistry));
    let unbound = std::cell::RefCell::new(Vec::new());
    loader.build(
        &loader.load(resource_manager),
        &|id, required| {
            if is_element(id) {
                Some(())
            } else if required {
                unbound.borrow_mut().push(id.to_string());
                Some(())
            } else {
                None
            }
        },
        &|_, _| true,
    );
    unbound.into_inner().iter().for_each(|id| record_unbound(id));
}

pub fn identifier_order<T>(entries: &mut [(String, T)]) {
    entries.sort_by(|(a, _), (b, _)| {
        let split = |id: &str| -> (String, String) {
            let (namespace, path) = id.split_once(':').unwrap_or(("minecraft", id));
            (path.to_string(), namespace.to_string())
        };
        let (a, b) = (split(a), split(b));
        a.0.encode_utf16().cmp(b.0.encode_utf16()).then_with(|| a.1.encode_utf16().cmp(b.1.encode_utf16()))
    });
}

impl TagLoader {
    pub fn with_context(directory: &str, context: Option<TagLoadContext>) -> TagLoader {
        TagLoader { context, directory: directory.to_string() }
    }

    fn parse_failed(&self, error: JavaThrowable) -> JavaThrowable {
        match self.context {
            Some(context) => registry_load_task::tag_parse_failed(context, error),
            None => error.without_stack_trace(),
        }
    }

    fn decode_failed(&self, message: &str) -> JavaThrowable {
        match self.context {
            Some(context) => registry_load_task::tag_decode_failed(context, message),
            None => JavaThrowable::new(format!("java.lang.IllegalStateException: {message}"), Vec::new()),
        }
    }

    fn file_to_id(&self, location: &str) -> String {
        let (namespace, path) = location.split_once(':').unwrap();
        let path = &path[self.directory.len() + 1..path.len() - ".json".len()];
        format!("{namespace}:{path}")
    }

    pub fn load(&self, resource_manager: &MultiPackResourceManager) -> JavaHashMap<Vec<EntryWithSource>> {
        let mut builders: JavaHashMap<Vec<EntryWithSource>> = JavaHashMap::new(identifier_hash);
        let mut stacks: Vec<_> = resource_manager.list_resource_stacks(&self.directory, ".json").into_iter().collect();
        identifier_order(&mut stacks);
        for (location, resources) in stacks {
            let id = self.file_to_id(&location);
            for resource in resources {
                let parsed = resource
                    .supplier
                    .read()
                    .map_err(|e| JavaThrowable::new(format!("java.io.IOException: {e}"), Vec::new()))
                    .and_then(|bytes| crate::util::gson::parse_strict_bytes_throwable(&bytes).map_err(|error| self.parse_failed(error)))
                    .and_then(|element| {
                        if builders.get(&id).is_none() {
                            builders.insert(id.clone(), Vec::new());
                        }
                        crate::nbt::with_json_source(|| TagFile::decode(&crate::world::item::component::json_to_nbt(&element))).map_err(|message| self.decode_failed(&message))
                    });
                match parsed {
                    Ok(parsed) => {
                        let contents = builders.get_mut(&id).unwrap();
                        if parsed.replace {
                            contents.clear();
                        }
                        contents.extend(parsed.entries.into_iter().map(|entry| EntryWithSource { entry, source: resource.source_pack_id.clone() }));
                    }
                    Err(exception) => {
                        error!("Couldn't read tag list {id} from {location} in data pack {}\n{}", resource.source_pack_id, exception.print_stack_trace().trim_end());
                    }
                }
            }
        }
        builders
    }

    pub fn build<V: Clone>(
        &self,
        builders: &JavaHashMap<Vec<EntryWithSource>>,
        element_lookup: &dyn Fn(&str, bool) -> Option<V>,
        same: &dyn Fn(&V, &V) -> bool,
    ) -> JavaHashMap<Vec<V>> {
        let mut new_tags: JavaHashMap<Vec<V>> = JavaHashMap::new(identifier_hash);
        let mut sorter = DependencySorter::new();
        for (id, entries) in builders.iter() {
            sorter.add_entry(id.to_string(), SortingEntry(entries));
        }
        sorter.order_by_dependencies(&mut |id, contents| {
            let lookup = TagLookup { element_lookup, tags: &new_tags };
            let mut values: Vec<V> = Vec::new();
            let mut missing_elements: Vec<&EntryWithSource> = Vec::new();
            for entry in contents.0 {
                let present = entry.entry.build(&lookup, &mut |value: V| {
                    if !values.iter().any(|v| same(v, &value)) {
                        values.push(value);
                    }
                });
                if !present {
                    missing_elements.push(entry);
                }
            }
            if missing_elements.is_empty() {
                new_tags.insert(id.to_string(), values);
            } else {
                let missing: Vec<String> = missing_elements.iter().map(|e| e.to_string()).collect();
                error!("Couldn't load tag {id} as it is missing following references: {}", missing.join(", "));
            }
        });
        new_tags
    }
}

struct TagLookup<'a, V> {
    element_lookup: &'a dyn Fn(&str, bool) -> Option<V>,
    tags: &'a JavaHashMap<Vec<V>>,
}

impl<V: Clone> Lookup<V> for TagLookup<'_, V> {
    fn element(&self, key: &str, required: bool) -> Option<V> {
        (self.element_lookup)(key, required)
    }

    fn tag(&self, key: &str) -> Option<Vec<V>> {
        self.tags.get(key).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::{ExistingTagLoading, TagFile, load_tags_for_existing_registries};

    fn decode_error(json: &str) -> String {
        let value: serde_json::Value = serde_json::from_str(json).unwrap();
        crate::nbt::with_json_source(|| TagFile::decode(&crate::world::item::component::json_to_nbt(&value)).err().expect("a decode error"))
    }

    #[test]
    fn tag_files_report_vanilla_json_messages() {
        assert_eq!(decode_error(r#"{"values":5}"#), "Not a json array: 5");
        assert_eq!(decode_error("[]"), "Not a JSON object: []");
        assert_eq!(decode_error("null"), "Not a JSON object: null");
        assert_eq!(decode_error("{}"), "No key values in MapLike[{}]");
        assert_eq!(decode_error(r#"{"values":[5]}"#), "Failed to parse either. First: Not a string: 5; Second: Not a JSON object: 5");
        assert_eq!(decode_error(r#"{"values":[],"replace":"x"}"#), "Not a boolean: \"x\"");
    }
    use crate::server::packs::repository::PackRepository;
    use crate::server::packs::resources::MultiPackResourceManager;

    #[test]
    fn vanilla_pack_tags_match_the_built_in_tags() {
        let mut repository = PackRepository::create_vanilla_trusted_repository();
        repository.reload();
        repository.set_selected(&["vanilla"]);
        let loaded = load_tags_for_existing_registries(&MultiPackResourceManager::new(repository.open_all_selected()), ExistingTagLoading::Reload(&[]));
        let built_in: Vec<(String, Vec<(String, Vec<i32>)>)> = wafer_data::TAGS
            .iter()
            .map(|registry| (registry.registry.to_string(), registry.tags.iter().map(|(name, values)| (name.to_string(), values.to_vec())).collect()))
            .collect();
        let loaded: Vec<_> = loaded.into_iter().filter(|(registry, _)| built_in.iter().any(|(name, _)| name == registry)).collect();
        assert_eq!(loaded, built_in);
    }
}
