use wafer_protocol::nbt::Nbt;

use crate::core::holder_set::HolderSet;
use crate::nbt::{java_double_to_string, java_float_to_string};
use crate::util::java_collections::identity_hash_code_of;
use crate::util::valueproviders::IntProvider;
use crate::world::attribute::attribute_string;
use crate::world::attribute::environment_attribute::EnvironmentAttributeInfo;
use crate::world::attribute::environment_attribute_map::Entry;
use crate::world::item::component::codec::{as_f64, as_i32, registry};
use crate::world::level::levelgen::density_function::resource_name;

pub fn direct_holder_to_string(tag: &Nbt) -> String {
    format!("Direct{{{}}}", crate::nbt::with_json_source(|| dimension_type_to_string(tag)))
}

fn dimension_type_to_string(tag: &Nbt) -> String {
    let flag = |key: &str| tag.get(key).and_then(Nbt::as_i64).is_some_and(|value| value != 0);
    let int = |key: &str| tag.get(key).map_or(0, as_i32);
    let enumeration = |key: &str, default: &str| tag.get(key).and_then(Nbt::as_str).unwrap_or(default).to_ascii_uppercase();
    let monster_spawn_light_test = tag
        .get("monster_spawn_light_level")
        .and_then(|light_level| IntProvider::parse(&crate::util::codec::to_json(light_level)).ok())
        .map_or_else(String::new, |provider| provider.java_to_string());
    let default_clock = tag.get("default_clock").and_then(Nbt::as_str).map(|clock| {
        let clock = resource_name(clock);
        format!("Optional[Reference{{ResourceKey[minecraft:world_clock / {clock}]=WorldClock[]}}]")
    });
    format!(
        "DimensionType[hasFixedTime={}, hasSkyLight={}, hasCeiling={}, hasEnderDragonFight={}, coordinateScale={}, minY={}, height={}, logicalHeight={}, infiniburn={}, ambientLight={}, monsterSettings=MonsterSettings[monsterSpawnLightTest={monster_spawn_light_test}, monsterSpawnBlockLightLimit={}], skybox={}, cardinalLightType={}, attributes={}, timelines={}, defaultClock={}]",
        flag("has_fixed_time"),
        flag("has_skylight"),
        flag("has_ceiling"),
        flag("has_ender_dragon_fight"),
        java_double_to_string(tag.get("coordinate_scale").map_or(0.0, as_f64)),
        int("min_y"),
        int("height"),
        int("logical_height"),
        tag.get("infiniburn").map_or_else(String::new, |infiniburn| holder_set_to_string("minecraft:block", infiniburn, block_reference)),
        java_float_to_string(tag.get("ambient_light").map_or(0.0, crate::world::item::component::codec::as_f32)),
        int("monster_spawn_block_light_limit"),
        enumeration("skybox", "overworld"),
        enumeration("cardinal_light", "default"),
        tag.get("attributes").map_or_else(|| "{}".to_string(), attributes_to_string),
        tag.get("timelines").map_or_else(|| "DirectSet[[]]".to_string(), |timelines| holder_set_to_string("minecraft:timeline", timelines, timeline_reference)),
        default_clock.unwrap_or_else(|| "Optional.empty".to_string()),
    )
}

fn block_reference(name: &str) -> String {
    format!("Reference{{ResourceKey[minecraft:block / {name}]=Block{{{name}}}}}")
}

fn timeline_reference(name: &str) -> String {
    format!("Reference{{ResourceKey[minecraft:timeline / {name}]=net.minecraft.world.timeline.Timeline@{:x}}}", identity_hash_code_of(&format!("timeline:{name}")))
}

fn holder_set_to_string(registry_name: &'static str, tag: &Nbt, reference: fn(&str) -> String) -> String {
    let Ok(set) = HolderSet::decode(registry_name, tag) else { return "DirectSet[[]]".to_string() };
    let entries = registry(registry_name);
    let contents: Vec<String> = set.contents().iter().filter_map(|&id| entries.get(usize::try_from(id).ok()?)).map(|name| reference(name)).collect();
    let contents = format!("[{}]", contents.join(", "));
    match &set {
        HolderSet::Direct(_) => format!("DirectSet[{contents}]"),
        HolderSet::Named(key, _) => format!("NamedSet(TagKey[{registry_name} / {key}])[{contents}]"),
    }
}

fn attributes_to_string(tag: &Nbt) -> String {
    let Nbt::Compound(fields) = tag else { return "{}".to_string() };
    let entries: Vec<String> = crate::util::codec::map_entry_order(fields)
        .into_iter()
        .filter_map(|index| {
            let (name, value) = &fields[index];
            let attribute = EnvironmentAttributeInfo::by_name(&resource_name(name))?;
            let json = crate::util::codec::to_json(value);
            let (entry, full) = Entry::parse_with_form(attribute, &json).ok()?;
            let argument = if full { value.get("argument")? } else { value };
            Some(format!("{}={}", attribute.name, attribute_string::entry_to_string(attribute, &entry, argument)))
        })
        .collect();
    format!("{{{}}}", entries.join(", "))
}
