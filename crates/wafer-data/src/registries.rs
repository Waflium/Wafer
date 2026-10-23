use std::cell::Cell;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::{LazyLock, RwLock};

use crate::Tag;
use std::sync::atomic::AtomicU64;
use crate::{BUILTIN_REGISTRIES, FEATURES, PACKET_NAMES, STRUCTURES, SYNCED_REGISTRIES, TAGS, TRADE_SETS, TYPED_TAGS, VILLAGER_TRADES};

static STRUCTURE_NAMES: LazyLock<Vec<&'static str>> = LazyLock::new(|| STRUCTURES.iter().map(|structure| structure.0).collect());
static FEATURE_NAMES: LazyLock<Vec<&'static str>> = LazyLock::new(|| FEATURES.iter().map(|feature| feature.0).collect());
static VILLAGER_TRADE_NAMES: LazyLock<Vec<&'static str>> = LazyLock::new(|| VILLAGER_TRADES.iter().map(|trade| trade.0).collect());
static TRADE_SET_NAMES: LazyLock<Vec<&'static str>> = LazyLock::new(|| TRADE_SETS.iter().map(|trade_set| trade_set.0).collect());

static ACTIVE_SYNCED_REGISTRIES: AtomicPtr<Vec<SyncedRegistry>> = AtomicPtr::new(std::ptr::null_mut());

pub fn synced_registries() -> &'static [SyncedRegistry] {
    let active = ACTIVE_SYNCED_REGISTRIES.load(Ordering::Acquire);
    if active.is_null() { SYNCED_REGISTRIES } else { unsafe { &*active } }
}

pub fn set_synced_registries(registries: Vec<SyncedRegistry>) {
    ACTIVE_SYNCED_REGISTRIES.store(Box::into_raw(Box::new(registries)), Ordering::Release);
}

pub fn builtin_synced_registries() -> &'static [SyncedRegistry] {
    SYNCED_REGISTRIES
}

pub fn registry_entries(name: &str) -> Option<&'static [&'static str]> {
    BUILTIN_REGISTRIES.iter().find(|r| r.0 == name).map(|r| r.1).or_else(|| synced_registries().iter().find(|r| r.name == name).map(|r| r.entries)).or_else(|| {
        match name {
            "minecraft:worldgen/structure" => Some(&STRUCTURE_NAMES[..]),
            "minecraft:worldgen/feature" => Some(&FEATURE_NAMES[..]),
            "minecraft:villager_trade" => Some(&VILLAGER_TRADE_NAMES[..]),
            "minecraft:trade_set" => Some(&TRADE_SET_NAMES[..]),
            _ => None,
        }
    })
}

pub fn registry_names() -> impl Iterator<Item = &'static str> {
    BUILTIN_REGISTRIES
        .iter()
        .map(|r| r.0)
        .chain(SYNCED_REGISTRIES.iter().map(|r| r.name))
        .chain(["minecraft:worldgen/structure", "minecraft:worldgen/feature", "minecraft:villager_trade", "minecraft:trade_set"])
}

pub struct SyncedRegistry {
    pub name: &'static str,
    pub entries: &'static [&'static str],
}

impl SyncedRegistry {
    pub fn get(name: &str) -> &'static SyncedRegistry {
        synced_registries()
            .iter()
            .find(|r| r.name == name)
            .unwrap_or_else(|| panic!("{name} is not a synchronized registry"))
    }

    pub fn id_of(&self, entry: &str) -> Option<i32> {
        self.entries.iter().position(|&e| e == entry).map(|i| i as i32)
    }
}

pub struct RegistryTags {
    pub registry: &'static str,
    pub tags: &'static [(&'static str, &'static [i32])],
}

pub fn registry_id(entries: &[&str], name: &str) -> i32 {
    entries.iter().position(|&e| e == name).unwrap_or_else(|| panic!("{name} is not registered")) as i32
}

pub fn packet_name(state: &str, direction: &str, id: i32) -> Option<&'static str> {
    PACKET_NAMES.iter().find(|p| p.0 == state && p.1 == direction && p.2 == id).map(|p| p.3)
}

static BOUND_TAGS: RwLock<Option<&'static [RegistryTags]>> = RwLock::new(None);

thread_local! {
    static PENDING_TAGS: Cell<Option<&'static [RegistryTags]>> = const { Cell::new(None) };
}

fn bound_tags() -> &'static [RegistryTags] {
    BOUND_TAGS.read().expect("bound tags").unwrap_or(TAGS)
}

pub fn tags() -> &'static [RegistryTags] {
    PENDING_TAGS.with(Cell::get).unwrap_or_else(bound_tags)
}

pub fn with_pending_tags<R>(tags: &'static [RegistryTags], f: impl FnOnce() -> R) -> R {
    let previous = PENDING_TAGS.with(|pending| pending.replace(Some(tags)));
    let result = f();
    PENDING_TAGS.with(|pending| pending.set(previous));
    result
}

pub fn tag(registry: &str, name: &str) -> &'static [i32] {
    tags().iter().find(|r| r.registry == registry).and_then(|r| r.tags.iter().find(|t| t.0 == name)).map_or(&[], |t| t.1)
}

fn same_tags(current: &[RegistryTags], tags: &[(String, Vec<(String, Vec<i32>)>)]) -> bool {
    current.len() == tags.len()
        && current.iter().zip(tags).all(|(current, (registry, tags))| {
            current.registry == registry
                && current.tags.len() == tags.len()
                && current.tags.iter().zip(tags).all(|((name, values), (other_name, other_values))| name == other_name && values == other_values)
        })
}

pub fn prepare_tags(tags: Vec<(String, Vec<(String, Vec<i32>)>)>) -> &'static [RegistryTags] {
    let current = bound_tags();
    if same_tags(current, &tags) {
        return current;
    }
    let prepared: Vec<RegistryTags> = tags
        .into_iter()
        .map(|(registry, tags)| {
            let tags: Vec<(&'static str, &'static [i32])> = tags.into_iter().map(|(name, values)| (&*name.leak(), &*values.leak())).collect();
            RegistryTags { registry: registry.leak(), tags: tags.leak() }
        })
        .collect();
    prepared.leak()
}

static DYNAMIC_TAGS: RwLock<Vec<(&'static str, &'static Tag)>> = RwLock::new(Vec::new());

pub fn dynamic_tag(registry: &'static str, name: &str) -> &'static Tag {
    if let Some(tag) = TYPED_TAGS.iter().find(|(r, _)| *r == registry).and_then(|(_, tags)| tags.iter().copied().find(|t| t.name == name)) {
        return tag;
    }
    let mut dynamic = DYNAMIC_TAGS.write().expect("dynamic tags");
    if let Some((_, tag)) = dynamic.iter().find(|(r, t)| *r == registry && t.name == name) {
        return tag;
    }
    let size = registry_entries(registry).map_or(0, <[&str]>::len) + 4096;
    let bits: &'static [AtomicU64] = Box::leak((0..size.div_ceil(64)).map(|_| AtomicU64::new(0)).collect());
    let tag: &'static Tag = Box::leak(Box::new(Tag::new(Box::leak(name.to_owned().into_boxed_str()), bits)));
    tag.bind(self::tag(registry, name));
    dynamic.push((registry, tag));
    tag
}

pub fn bind_tags(bound: &'static [RegistryTags]) {
    if std::ptr::eq(bound, bound_tags()) {
        return;
    }
    for (registry, typed) in TYPED_TAGS {
        let registry_tags = bound.iter().find(|r| r.registry == *registry).map_or(&[][..], |r| r.tags);
        for tag in typed.iter() {
            tag.bind(registry_tags.iter().find(|t| t.0 == tag.name).map_or(&[], |t| t.1));
        }
    }
    for (registry, tag) in DYNAMIC_TAGS.read().expect("dynamic tags").iter() {
        let registry_tags = bound.iter().find(|r| r.registry == *registry).map_or(&[][..], |r| r.tags);
        tag.bind(registry_tags.iter().find(|t| t.0 == tag.name).map_or(&[], |t| t.1));
    }
    *BOUND_TAGS.write().expect("bound tags") = Some(bound);
}

static ACTIVE_DIMENSION_TYPES: AtomicPtr<Vec<DimensionType>> = AtomicPtr::new(std::ptr::null_mut());

pub fn dimension_types() -> &'static [DimensionType] {
    let active = ACTIVE_DIMENSION_TYPES.load(Ordering::Acquire);
    if active.is_null() { crate::DIMENSION_TYPES } else { unsafe { &*active } }
}

pub fn set_dimension_types(dimension_types: Vec<DimensionType>) {
    ACTIVE_DIMENSION_TYPES.store(Box::into_raw(Box::new(dimension_types)), Ordering::Release);
}

pub struct DimensionType {
    pub name: &'static str,
    pub min_y: i32,
    pub height: u32,
    pub logical_height: u32,
    pub coordinate_scale: f64,
    pub has_skylight: bool,
    pub has_ceiling: bool,
    pub has_ender_dragon_fight: bool,
    pub has_fixed_time: bool,
    pub ambient_light: f32,
    pub infiniburn: &'static crate::Tag,
    pub default_clock: Option<&'static str>,
    pub attributes: &'static str,
    pub timelines: &'static str,
    pub monster_spawn_light_level: &'static str,
    pub monster_spawn_block_light_limit: i32,
}
