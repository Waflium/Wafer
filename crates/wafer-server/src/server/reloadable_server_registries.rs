use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{LazyLock, RwLock};

use wafer_data::RegistryTags;
use wafer_protocol::nbt::Nbt;

use super::packs::repository::{PackRepository, VANILLA_ID};
use super::packs::resources::MultiPackResourceManager;
use crate::advancements::advancement::Advancement;
use crate::advancements::advancement_holder::AdvancementHolder;
use crate::resources::registry_load_task::{self, ElementFailure};
use crate::tags::identifier_order;
use crate::util::java_throwable::JavaThrowable;
use crate::world::item::component::json_to_nbt;
use crate::world::item::crafting::recipe::Recipe;
use crate::world::item::crafting::recipe_holder::RecipeHolder;
use crate::world::item::crafting::recipe_manager::RecipeManager;
use crate::world::item::slot::slot_source::SlotSource;
use crate::world::level::storage::loot::functions::loot_item_function::LootItemFunction;
use crate::world::level::storage::loot::loot_table::LootTable;
use crate::util::problem_reporter::{PathElement, ProblemReporter};
use crate::world::level::storage::loot::parameters::loot_context_param_sets::{self, ContextKeySet};
use crate::world::level::storage::loot::validatable::Validatable;
use crate::world::level::storage::loot::validation_context::ValidationContext;
use crate::world::level::storage::loot::predicates::loot_item_condition::LootItemCondition;
use crate::world::level::storage::loot::providers::number::floats::ContextFloatProvider;
use crate::world::level::storage::loot::providers::number::ints::ContextIntProvider;

pub const LOOT_TABLE: &str = "minecraft:loot_table";
pub const PREDICATE: &str = "minecraft:predicate";
pub const CONTEXT_FLOAT_PROVIDER: &str = "minecraft:context_float_provider";
pub const CONTEXT_INT_PROVIDER: &str = "minecraft:context_int_provider";
pub const ITEM_MODIFIER: &str = "minecraft:item_modifier";
pub const SLOT_SOURCE: &str = "minecraft:slot_source";
pub const ADVANCEMENT: &str = "minecraft:advancement";
pub const RECIPE: &str = "minecraft:recipe";
const RELOADABLE_REGISTRIES: [&str; 8] = [LOOT_TABLE, PREDICATE, CONTEXT_FLOAT_PROVIDER, CONTEXT_INT_PROVIDER, ITEM_MODIFIER, SLOT_SOURCE, ADVANCEMENT, RECIPE];

#[derive(PartialEq)]
struct Source {
    id: String,
    pack: String,
    bytes: Vec<u8>,
}

pub struct ReloadableRegistries {
    sources: Vec<Vec<Source>>,
    tags: &'static [RegistryTags],
    pub loot_tables: HashMap<String, LootTable>,
    pub predicates: HashMap<String, LootItemCondition>,
    pub context_float_providers: HashMap<String, ContextFloatProvider>,
    pub context_int_providers: HashMap<String, ContextIntProvider>,
    pub item_modifiers: HashMap<String, LootItemFunction>,
    pub slot_sources: HashMap<String, SlotSource>,
    pub advancements: Vec<&'static AdvancementHolder>,
}

impl ReloadableRegistries {
    fn contains_key(&self, registry: &str, key: &str) -> bool {
        match registry {
            LOOT_TABLE => self.loot_tables.contains_key(key),
            PREDICATE => self.predicates.contains_key(key),
            CONTEXT_FLOAT_PROVIDER => self.context_float_providers.contains_key(key),
            CONTEXT_INT_PROVIDER => self.context_int_providers.contains_key(key),
            ITEM_MODIFIER => self.item_modifiers.contains_key(key),
            SLOT_SOURCE => self.slot_sources.contains_key(key),
            ADVANCEMENT => self.advancements.iter().any(|holder| holder.id == key),
            _ => false,
        }
    }
}

pub enum LoadResult {
    Unchanged,
    Loaded { registries: ReloadableRegistries, recipes: Vec<RecipeHolder> },
}

thread_local! {
    static VALIDATING: Cell<Option<*const ReloadableRegistries>> = const { Cell::new(None) };
    static PENDING_KEYS: RefCell<Option<HashMap<&'static str, HashSet<String>>>> = const { RefCell::new(None) };
    static UNBOUND: RefCell<Vec<(String, String)>> = const { RefCell::new(Vec::new()) };
}

pub fn is_loading() -> bool {
    PENDING_KEYS.with(|pending| pending.borrow().is_some())
}

pub fn is_reloadable(registry: &str) -> bool {
    RELOADABLE_REGISTRIES.contains(&registry)
}

pub fn record_unbound(registry: &str, key: &str) {
    UNBOUND.with(|unbound| unbound.borrow_mut().push((registry.to_string(), key.to_string())));
}

static BOUND: RwLock<Option<&'static ReloadableRegistries>> = RwLock::new(None);
static VANILLA: LazyLock<(&'static ReloadableRegistries, &'static RecipeManager)> = LazyLock::new(|| {
    let mut repository = PackRepository::create_vanilla_trusted_repository();
    repository.reload();
    repository.set_selected(&[VANILLA_ID]);
    let resource_manager = MultiPackResourceManager::new(repository.open_all_selected());
    match load(&resource_manager, wafer_data::tags(), None) {
        Ok(LoadResult::Loaded { registries, recipes }) => (Box::leak(Box::new(registries)), Box::leak(Box::new(RecipeManager::new(recipes)))),
        Ok(LoadResult::Unchanged) => unreachable!(),
        Err(errors) => panic!("{errors}"),
    }
});

pub fn get() -> &'static ReloadableRegistries {
    if let Some(validating) = VALIDATING.with(Cell::get) {
        return unsafe { &*validating };
    }
    match *BOUND.read().expect("reloadable registries") {
        Some(registries) => registries,
        None => VANILLA.0,
    }
}

pub fn vanilla_recipe_manager() -> &'static RecipeManager {
    VANILLA.1
}

pub fn contains_key(registry: &str, key: &str) -> bool {
    PENDING_KEYS
        .with(|pending| pending.borrow().as_ref().map(|keys| keys.get(registry).is_some_and(|keys| keys.contains(key))))
        .unwrap_or_else(|| get().contains_key(registry, key))
}

pub fn with_validating<R>(registries: &ReloadableRegistries, f: impl FnOnce() -> R) -> R {
    let previous = VALIDATING.with(|validating| validating.replace(Some(registries as *const ReloadableRegistries)));
    let result = f();
    VALIDATING.with(|validating| validating.set(previous));
    result
}

fn run_validation<T: Validatable>(problems: &ProblemReporter, registry: &str, sources: &[Source], elements: &HashMap<String, T>, context: impl Fn(&T) -> &'static ContextKeySet) {
    for source in sources {
        let Some(value) = elements.get(&source.id) else { continue };
        let root = ValidationContext::new(problems.clone(), context(value))
            .enter_element(PathElement::RootElement { id: source.id.clone(), registry: registry.to_string() }, registry, &source.id);
        value.validate(&root);
    }
}

fn all_params<T>(_: &T) -> &'static ContextKeySet {
    &loot_context_param_sets::ALL_PARAMS
}

fn validate_loot_registries(registries: &ReloadableRegistries, sources: &[Vec<Source>]) -> Result<(), JavaThrowable> {
    let problems = ProblemReporter::default();
    with_validating(registries, || {
        run_validation(&problems, PREDICATE, &sources[1], &registries.predicates, all_params);
        run_validation(&problems, ITEM_MODIFIER, &sources[4], &registries.item_modifiers, all_params);
        run_validation(&problems, SLOT_SOURCE, &sources[5], &registries.slot_sources, all_params);
        run_validation(&problems, LOOT_TABLE, &sources[0], &registries.loot_tables, |table: &LootTable| table.param_set);
        run_validation(&problems, CONTEXT_FLOAT_PROVIDER, &sources[2], &registries.context_float_providers, all_params);
        run_validation(&problems, CONTEXT_INT_PROVIDER, &sources[3], &registries.context_int_providers, all_params);
    });
    problems.for_each(|id, problem| warn!("Found loot table element validation problem in {id}: {}", problem.description));
    if problems.has_fatal_problems() {
        return Err(registry_load_task::loot_table_validation_failed());
    }
    Ok(())
}

pub fn bind(registries: ReloadableRegistries) {
    *BOUND.write().expect("reloadable registries") = Some(Box::leak(Box::new(registries)));
}

fn elements_dir_path(registry: &str) -> &str {
    registry.strip_prefix("minecraft:").unwrap_or(registry)
}

fn list_sources(resource_manager: &MultiPackResourceManager, registry: &str) -> Vec<Source> {
    let directory = elements_dir_path(registry);
    let mut resources: Vec<_> = resource_manager.list_resources(directory, ".json").into_iter().collect();
    identifier_order(&mut resources);
    resources
        .into_iter()
        .map(|(location, resource)| {
            let (namespace, path) = location.split_once(':').unwrap();
            let id = format!("{namespace}:{}", &path[directory.len() + 1..path.len() - ".json".len()]);
            let bytes = resource.supplier.read().unwrap_or_default();
            Source { id, pack: resource.source_pack_id, bytes }
        })
        .collect()
}

pub struct LoadingErrors(pub BTreeMap<(String, String), JavaThrowable>);

impl LoadingErrors {
    pub fn parse_error(&mut self, registry: &str, id: &str, pack: &str, resource_count: usize, failure: ElementFailure) {
        self.0.insert((registry.to_string(), id.to_string()), registry_load_task::failed_to_parse(id, pack, resource_count, failure));
    }

    pub fn report(&self) -> String {
        let mut by_registry: BTreeMap<&str, Vec<(&str, &JavaThrowable)>> = BTreeMap::new();
        for ((registry, id), error) in &self.0 {
            by_registry.entry(registry).or_default().push((id, error));
        }
        let mut collected_errors = String::new();
        for (registry, mut elements) in by_registry {
            collected_errors.push_str(&format!("> Errors in registry {registry}:\n"));
            elements.sort_by(|a, b| {
                let split = |id: &str| id.split_once(':').map(|(namespace, path)| (path.to_string(), namespace.to_string())).unwrap_or_default();
                split(a.0).cmp(&split(b.0))
            });
            for (id, error) in elements {
                collected_errors.push_str(&format!(">> Errors in element {id}:\n{}", error.print_stack_trace()));
            }
        }
        format!("Registry loading errors:\n{collected_errors}")
    }
}

fn decode_registry<T>(registry: &str, sources: &[Source], decode: fn(&Nbt) -> Result<T, String>, errors: &mut LoadingErrors) -> HashMap<String, T> {
    let mut elements = HashMap::with_capacity(sources.len());
    for source in sources {
        let parsed = match crate::util::gson::parse_strict_bytes_throwable(&source.bytes) {
            Ok(json) => crate::nbt::with_json_source(|| decode(&json_to_nbt(&json))).map_err(ElementFailure::Decode),
            Err(throwable) => Err(ElementFailure::Parse(throwable)),
        };
        match parsed {
            Ok(element) => {
                elements.insert(source.id.clone(), element);
            }
            Err(failure) => errors.parse_error(registry, &source.id, &source.pack, sources.len(), failure),
        }
    }
    elements
}

pub fn load(resource_manager: &MultiPackResourceManager, tags: &'static [RegistryTags], current: Option<&ReloadableRegistries>) -> Result<LoadResult, JavaThrowable> {
    let sources: Vec<Vec<Source>> = RELOADABLE_REGISTRIES.iter().map(|registry| list_sources(resource_manager, registry)).collect();
    if current.is_some_and(|current| std::ptr::eq(current.tags, tags) && current.sources == sources) {
        return Ok(LoadResult::Unchanged);
    }
    let keys = RELOADABLE_REGISTRIES.iter().zip(&sources).map(|(&registry, sources)| (registry, sources.iter().map(|source| source.id.clone()).collect())).collect();
    let previous = PENDING_KEYS.with(|pending| pending.replace(Some(keys)));
    let mut errors = LoadingErrors(BTreeMap::new());
    let (registries, recipes) = wafer_data::with_pending_tags(tags, || {
        let loot_tables = decode_registry(LOOT_TABLE, &sources[0], LootTable::decode, &mut errors);
        let predicates = decode_registry(PREDICATE, &sources[1], LootItemCondition::decode, &mut errors);
        let context_float_providers = decode_registry(CONTEXT_FLOAT_PROVIDER, &sources[2], ContextFloatProvider::decode, &mut errors);
        let context_int_providers = decode_registry(CONTEXT_INT_PROVIDER, &sources[3], ContextIntProvider::decode, &mut errors);
        let item_modifiers = decode_registry(ITEM_MODIFIER, &sources[4], |tag| LootItemFunction::decode_typed(tag).into_result(), &mut errors);
        let slot_sources = decode_registry(SLOT_SOURCE, &sources[5], SlotSource::decode_typed, &mut errors);
        let mut decoded_advancements = decode_registry(ADVANCEMENT, &sources[6], Advancement::decode, &mut errors);
        let advancements: Vec<&'static AdvancementHolder> = sources[6]
            .iter()
            .filter_map(|source| decoded_advancements.remove(&source.id).map(|value| &*Box::leak(Box::new(AdvancementHolder { id: source.id.clone(), value }))))
            .collect();
        let mut decoded_recipes = decode_registry(RECIPE, &sources[7], Recipe::decode, &mut errors);
        let recipes: Vec<RecipeHolder> =
            sources[7].iter().filter_map(|source| decoded_recipes.remove(&source.id).map(|value| RecipeHolder { id: source.id.clone().leak(), value })).collect();
        (
            ReloadableRegistries {
                sources: Vec::new(),
                tags,
                loot_tables,
                predicates,
                context_float_providers,
                context_int_providers,
                item_modifiers,
                slot_sources,
                advancements,
            },
            recipes,
        )
    });
    PENDING_KEYS.with(|pending| pending.replace(previous));
    for (registry, keys) in crate::core::data_registries::group_unbound(UNBOUND.with(|unbound| std::mem::take(&mut *unbound.borrow_mut()))) {
        errors.0.insert(("minecraft:root".to_string(), registry.clone()), registry_load_task::unbound_values(&format!("ResourceKey[minecraft:root / {registry}]"), &keys));
    }
    if !errors.0.is_empty() {
        error!("{}", errors.report());
        return Err(registry_load_task::registry_loading_failed(false));
    }
    wafer_data::with_pending_tags(tags, || validate_loot_registries(&registries, &sources))?;
    Ok(LoadResult::Loaded { registries: ReloadableRegistries { sources, ..registries }, recipes })
}
