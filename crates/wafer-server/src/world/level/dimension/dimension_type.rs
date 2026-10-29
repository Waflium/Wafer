use std::sync::LazyLock;

use wafer_data::DimensionType;

use crate::util::valueproviders::{IntProvider, int_provider_codec, int_provider_codec_in};
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

static INLINE_MONSTER_SPAWN_LIGHT_TESTS: std::sync::Mutex<Vec<(&'static str, &'static IntProvider)>> = std::sync::Mutex::new(Vec::new());

pub fn monster_spawn_light_test(dimension: &'static DimensionType) -> &'static IntProvider {
    match wafer_data::dimension_types().iter().position(|d| d.name == dimension.name) {
        Some(index) => &MONSTER_SPAWN_LIGHT_TESTS[index],
        None => {
            let mut inline = INLINE_MONSTER_SPAWN_LIGHT_TESTS.lock().unwrap();
            if let Some((_, provider)) = inline.iter().find(|(name, _)| *name == dimension.name) {
                return provider;
            }
            let json = serde_json::from_str(dimension.monster_spawn_light_level).expect("monster_spawn_light_level JSON");
            let provider: &'static IntProvider = Box::leak(Box::new(IntProvider::parse(&json).expect("a monster spawn light level")));
            inline.push((dimension.name, provider));
            provider
        }
    }
}

pub fn monster_spawn_block_light_limit(dimension: &'static DimensionType) -> i32 {
    dimension.monster_spawn_block_light_limit
}

pub const MIN_Y: i32 = -2032;
pub const MAX_Y: i32 = 2031;
pub const Y_SIZE: i32 = (1 << 12) - 32;

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
        req("monster_spawn_light_level", int_provider_codec_in::<0, 15>()),
        req("monster_spawn_block_light_limit", (Codec::Int(0, 15, RangeError::Outside), Stream::Int)),
        def("skybox", types::enumeration(&["none", "overworld", "end"]), types::s("overworld")),
        def("cardinal_light", types::enumeration(&["default", "nether"]), types::s("default")),
        opt("attributes", crate::world::attribute::environment_attribute_map::codec()),
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
        let map = EnvironmentAttributeMap::parse(attributes);
        let attributes = map.encode_network()?;
        if !map.entries().is_empty() {
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
