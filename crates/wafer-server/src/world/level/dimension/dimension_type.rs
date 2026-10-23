use std::sync::LazyLock;

use wafer_data::DimensionType;

use crate::util::valueproviders::{IntProvider, int_provider_codec};
use serde_json::Value;
use wafer_protocol::nbt::Nbt;
use crate::world::attribute::EnvironmentAttributeMap;
use crate::world::item::component::codec::{Codec, RangeError, Stream};
use crate::world::item::component::json_to_nbt;
use crate::world::item::component::custom::DecodeOnly;
use crate::world::item::component::types::{self, Pair, def, opt, record, req};

static MONSTER_SPAWN_LIGHT_TESTS: LazyLock<Vec<IntProvider>> = LazyLock::new(|| {
    wafer_data::dimension_types()
        .iter()
        .map(|dimension| {
            let json = serde_json::from_str(dimension.monster_spawn_light_level).expect("monster_spawn_light_level JSON");
            IntProvider::parse(&json).unwrap_or_else(|e| panic!("monster_spawn_light_level of {}: {e}", dimension.name))
        })
        .collect()
});

pub fn monster_spawn_light_test(dimension: &'static DimensionType) -> &'static IntProvider {
    let index = wafer_data::dimension_types().iter().position(|d| d.name == dimension.name).expect("a registered dimension type");
    &MONSTER_SPAWN_LIGHT_TESTS[index]
}

pub fn monster_spawn_block_light_limit(dimension: &'static DimensionType) -> i32 {
    dimension.monster_spawn_block_light_limit
}

pub const MIN_Y: i32 = -2032;
pub const MAX_Y: i32 = 2031;
pub const Y_SIZE: i32 = (1 << 12) - 32;

fn monster_spawn_light_level(tag: &Nbt) -> Result<(), String> {
    let json = serde_json::from_str(&crate::nbt::to_string(tag)).map_err(|e| e.to_string())?;
    let provider = IntProvider::parse(&json)?;
    if provider.min_inclusive() < 0 {
        return Err(format!("Value provider too low: 0 [{}-{}]", provider.min_inclusive(), provider.max_inclusive()));
    }
    if provider.max_inclusive() > 15 {
        return Err(format!("Value provider too high: 15 [{}-{}]", provider.min_inclusive(), provider.max_inclusive()));
    }
    Ok(())
}

fn coordinate_scale(tag: &Nbt) -> Result<(), String> {
    let value = Codec::Double.decode(tag)?.as_f64().unwrap_or(0.0);
    let (min, max) = (1.0e-5f32 as f64, 3.0e7);
    if value < min || value > max {
        return Err(format!(
            "Value {} outside of range [{}:{}]",
            crate::nbt::java_double_to_string(value),
            crate::nbt::java_double_to_string(min),
            crate::nbt::java_double_to_string(max)
        ));
    }
    Ok(())
}

fn attributes(tag: &Nbt) -> Result<(), String> {
    let json = serde_json::from_str(&crate::nbt::to_string(tag)).map_err(|e| e.to_string())?;
    EnvironmentAttributeMap::parse(&json, false).map(drop)
}

fn custom(decode: fn(&Nbt) -> Result<(), String>) -> Pair {
    (Codec::Custom(Box::new(DecodeOnly(decode))), Stream::Custom)
}

pub fn direct_codec() -> Pair {
    record(vec![
        def("has_fixed_time", types::bool(), types::b(false)),
        req("has_skylight", types::bool()),
        req("has_ceiling", types::bool()),
        req("has_ender_dragon_fight", types::bool()),
        req("coordinate_scale", custom(coordinate_scale)),
        req("min_y", (Codec::Int(MIN_Y, MAX_Y, RangeError::Outside), Stream::Int)),
        req("height", (Codec::Int(16, Y_SIZE, RangeError::Outside), Stream::Int)),
        req("logical_height", (Codec::Int(0, Y_SIZE, RangeError::Outside), Stream::Int)),
        req("infiniburn", types::holder_set("minecraft:block")),
        req("ambient_light", types::float(Codec::float())),
        req("monster_spawn_light_level", custom(monster_spawn_light_level)),
        req("monster_spawn_block_light_limit", (Codec::Int(0, 15, RangeError::Outside), Stream::Int)),
        def("skybox", types::enumeration(&["none", "overworld", "end"]), types::s("overworld")),
        def("cardinal_light", types::enumeration(&["default", "nether"]), types::s("default")),
        opt("attributes", custom(attributes)),
        def("timelines", types::holder_set("minecraft:timeline"), types::empty_list()),
        opt("default_clock", (Codec::Holder("minecraft:world_clock", None), Stream::Holder("minecraft:world_clock", None))),
    ])
}

fn network_codec() -> Pair {
    record(vec![
        def("has_fixed_time", types::bool(), types::b(false)),
        req("has_skylight", types::bool()),
        req("has_ceiling", types::bool()),
        req("has_ender_dragon_fight", types::bool()),
        req("coordinate_scale", (Codec::Double, Stream::Double)),
        req("min_y", (Codec::Int(MIN_Y, MAX_Y, RangeError::Outside), Stream::Int)),
        req("height", (Codec::Int(16, Y_SIZE, RangeError::Outside), Stream::Int)),
        req("logical_height", (Codec::Int(0, Y_SIZE, RangeError::Outside), Stream::Int)),
        req("infiniburn", types::holder_set("minecraft:block")),
        req("ambient_light", types::float(Codec::float())),
        req("monster_spawn_light_level", int_provider_codec()),
        req("monster_spawn_block_light_limit", (Codec::Int(0, 15, RangeError::Outside), Stream::Int)),
        def("skybox", types::enumeration(&["none", "overworld", "end"]), types::s("overworld")),
        def("cardinal_light", types::enumeration(&["default", "nether"]), types::s("default")),
        def("timelines", types::holder_set("minecraft:timeline"), types::empty_list()),
        opt("default_clock", (Codec::Holder("minecraft:world_clock", None), Stream::Holder("minecraft:world_clock", None))),
    ])
}

pub fn encode_network(json: &Value) -> Result<Nbt, String> {
    let Nbt::Compound(mut fields) = network_codec().0.decode(&json_to_nbt(json))? else { return Err("A dimension type is not a compound".into()) };
    if let Some(attributes) = json.get("attributes") {
        let attributes = EnvironmentAttributeMap::parse(attributes, false)?.encode_network()?;
        if !matches!(&attributes, Nbt::Compound(entries) if entries.is_empty()) {
            fields.push(("attributes".into(), attributes));
        }
    }
    Ok(Nbt::Compound(fields))
}

pub fn check(tag: &Nbt) -> Result<(), String> {
    let int = |key: &str| tag.get(key).and_then(Nbt::as_i64).unwrap_or(0) as i32;
    let (min_y, height, logical_height) = (int("min_y"), int("height"), int("logical_height"));
    if height < 16 {
        return Err("height has to be at least 16".into());
    }
    if min_y + height > MAX_Y + 1 {
        return Err(format!("min_y + height cannot be higher than: {}", MAX_Y + 1));
    }
    if logical_height > height {
        return Err("logical_height cannot be higher than height".into());
    }
    if height % 16 != 0 {
        return Err("height has to be multiple of 16".into());
    }
    if min_y % 16 != 0 {
        return Err("min_y has to be a multiple of 16".into());
    }
    Ok(())
}
