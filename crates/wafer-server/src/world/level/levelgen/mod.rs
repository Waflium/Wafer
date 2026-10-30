pub mod aquifer;
pub mod beardifier;
pub mod below_zero_retrogen;
pub mod blending;
pub mod blockpredicates;
pub mod carver;
pub mod chunk_generator;
pub mod column;
pub mod debug_level_source;
pub mod density_function;
pub mod feature;
pub mod flat;
pub mod presets;
pub mod world_dimensions;
pub mod world_gen_settings;
pub mod world_options;
pub mod geode_block_settings;
pub mod geode_crack_settings;
pub mod geode_layer_settings;
pub mod heightproviders;
pub mod material;
pub mod patrol_spawner;
pub mod phantom_spawner;
pub mod placement;
pub mod random_state;
pub mod structure;
pub mod synth;
pub mod worldgen_random;
#[cfg(test)]
mod codec_tests;
#[cfg(test)]
mod flat_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod patrol_spawner_tests;

use std::collections::HashMap;
use std::sync::{Arc, LazyLock};

use wafer_core::random::{LegacyPositionalRandomFactory, LegacyRandomSource, XoroshiroPositionalRandomFactory, XoroshiroRandomSource};
use wafer_core::RandomSource;
use wafer_data::{Block, BlockState};
use serde_json::Value;

pub use chunk_generator::NoiseBasedChunkGenerator;
pub use random_state::RandomState;

use density_function::{Df, Parser, resource_name};
use material::MaterialRule;
use synth::{NormalNoise, NormalNoiseParameters, Normalization};

#[derive(Clone)]
pub enum AlgorithmRandom {
    Xoroshiro(XoroshiroRandomSource),
    Legacy(LegacyRandomSource),
}

macro_rules! delegate {
    ($self:ident, $r:ident => $e:expr) => {
        match $self {
            AlgorithmRandom::Xoroshiro($r) => $e,
            AlgorithmRandom::Legacy($r) => $e,
        }
    };
}

impl RandomSource for AlgorithmRandom {
    fn set_seed(&mut self, seed: i64) {
        delegate!(self, r => r.set_seed(seed))
    }
    fn next_int(&mut self) -> i32 {
        delegate!(self, r => r.next_int())
    }
    fn next_int_bounded(&mut self, bound: i32) -> i32 {
        delegate!(self, r => r.next_int_bounded(bound))
    }
    fn next_long(&mut self) -> i64 {
        delegate!(self, r => r.next_long())
    }
    fn next_boolean(&mut self) -> bool {
        delegate!(self, r => r.next_boolean())
    }
    fn next_float(&mut self) -> f32 {
        delegate!(self, r => r.next_float())
    }
    fn next_double(&mut self) -> f64 {
        delegate!(self, r => r.next_double())
    }
    fn next_gaussian(&mut self) -> f64 {
        delegate!(self, r => r.next_gaussian())
    }
    fn consume_count(&mut self, rounds: i32) {
        delegate!(self, r => r.consume_count(rounds))
    }
}

impl AlgorithmRandom {
    pub fn new(legacy: bool, seed: i64) -> AlgorithmRandom {
        if legacy { AlgorithmRandom::Legacy(LegacyRandomSource::new(seed)) } else { AlgorithmRandom::Xoroshiro(XoroshiroRandomSource::new(seed)) }
    }

    pub fn fork_positional(&mut self) -> PositionalRandomFactory {
        match self {
            AlgorithmRandom::Xoroshiro(r) => PositionalRandomFactory::Xoroshiro(r.fork_positional()),
            AlgorithmRandom::Legacy(r) => PositionalRandomFactory::Legacy(r.fork_positional()),
        }
    }
}

#[derive(Clone, Copy)]
pub enum PositionalRandomFactory {
    Xoroshiro(XoroshiroPositionalRandomFactory),
    Legacy(LegacyPositionalRandomFactory),
}

impl PositionalRandomFactory {
    pub fn at(&self, x: i32, y: i32, z: i32) -> AlgorithmRandom {
        match self {
            PositionalRandomFactory::Xoroshiro(f) => AlgorithmRandom::Xoroshiro(f.at(x, y, z)),
            PositionalRandomFactory::Legacy(f) => AlgorithmRandom::Legacy(f.at(x, y, z)),
        }
    }

    pub fn from_hash_of(&self, name: &str) -> AlgorithmRandom {
        match self {
            PositionalRandomFactory::Xoroshiro(f) => AlgorithmRandom::Xoroshiro(f.from_hash_of(name)),
            PositionalRandomFactory::Legacy(f) => AlgorithmRandom::Legacy(f.from_hash_of(name)),
        }
    }
}

pub mod quart_pos {
    pub const fn from_block(block: i32) -> i32 {
        block >> 2
    }
    pub const fn to_block(quart: i32) -> i32 {
        quart << 2
    }
}

pub fn parse_block_state(v: &Value) -> Result<BlockState, String> {
    let (name, properties) = match v {
        Value::String(name) => (name.as_str(), None),
        _ => (v.get("id").and_then(Value::as_str).ok_or("block state without id")?, v.get("properties").and_then(Value::as_object)),
    };
    let block = Block::by_name(&resource_name(name)).ok_or_else(|| format!("unknown block {name}"))?;
    let mut state = block.default_state();
    for (key, value) in properties.into_iter().flatten() {
        if let Some(with_property) = value.as_str().and_then(|value| state.with_property_by_name(key, value)) {
            state = with_property;
        }
    }
    Ok(state)
}

#[derive(Clone, Copy, Debug)]
pub struct NoiseSettings {
    pub min_y: i32,
    pub height: i32,
}

impl NoiseSettings {
    pub fn clamp_to(self, min_y: i32, height: i32) -> NoiseSettings {
        let new_min = self.min_y.max(min_y);
        let new_height = (self.min_y + self.height).min(min_y + height) - new_min;
        NoiseSettings { min_y: new_min, height: new_height }
    }
}

#[derive(Clone)]
pub struct NoiseRouter {
    pub temperature: Df,
    pub vegetation: Df,
    pub continents: Df,
    pub erosion: Df,
    pub depth: Df,
    pub ridges: Df,
    pub chunk_surface_level: Df,
    pub final_density: Df,
}

#[derive(Clone)]
pub struct AquiferConfig {
    pub barrier: Df,
    pub fluid_level_floodedness: Df,
    pub fluid_level_spread: Df,
    pub lava: Df,
    pub exclusion: Df,
    pub surface_level: Df,
}

#[derive(Clone)]
pub struct SpawnTargetPoint {
    pub parameters: Vec<(Df, crate::world::level::biome::climate::Parameter)>,
}

pub struct NoiseGeneratorSettings {
    pub name: String,
    pub noise: NoiseSettings,
    pub default_block: BlockState,
    pub default_fluid: BlockState,
    pub noise_router: NoiseRouter,
    pub material_rule: Arc<MaterialRule>,
    pub spawn_target: Vec<SpawnTargetPoint>,
    pub sea_level: i32,
    pub disable_mob_generation: bool,
    pub aquifers: Option<AquiferConfig>,
    pub legacy_random_source: bool,
}

pub struct WorldgenRegistries {
    pub noises: HashMap<String, Arc<NormalNoise>>,
    pub noise_settings: HashMap<String, Arc<NoiseGeneratorSettings>>,
}

pub static WORLDGEN: LazyLock<WorldgenRegistries> = LazyLock::new(|| WorldgenRegistries::load().expect("vanilla worldgen data"));

pub fn parse_noise(json: &Value) -> Result<NormalNoise, String> {
    let normalize = match json.get("normalize") {
        None | Some(Value::Bool(true)) => Normalization::Enabled,
        Some(Value::Bool(false)) => Normalization::Disabled,
        Some(Value::String(s)) if s == "legacy" => Normalization::Legacy,
        Some(other) => return Err(format!("bad normalize {other}")),
    };
    let modifiers = match json.get("amplitude_modifiers") {
        Some(Value::Array(a)) => a.iter().map(|v| v.as_f64().ok_or("bad amplitude")).collect::<Result<_, _>>()?,
        _ => Vec::new(),
    };
    Ok(NormalNoise::new(NormalNoiseParameters {
        base_amplitude: json.get("base_amplitude").and_then(Value::as_f64).unwrap_or(1.0),
        base_octave: json.get("base_octave").and_then(Value::as_i64).ok_or_else(|| crate::util::codec::no_key("base_octave", json))? as i32,
        octave_count: json.get("octave_count").and_then(Value::as_i64).unwrap_or(1) as i32,
        normalize,
        amplitude_modifiers: modifiers,
    }))
}

impl WorldgenRegistries {
    fn load() -> Result<WorldgenRegistries, String> {
        let mut noises = HashMap::new();
        for (name, source) in crate::core::data_registries::present_json_entries("minecraft:worldgen/noise") {
            let json: Value = serde_json::from_str(source).map_err(|e| format!("{name}: {e}"))?;
            noises.insert(name.to_string(), Arc::new(parse_noise(&json).map_err(|e| format!("{name}: {e}"))?));
        }
        let sources: HashMap<&'static str, &'static str> = crate::core::data_registries::present_json_entries("minecraft:worldgen/density_function").copied().collect();
        let mut parser = Parser { noises: &noises, sources: &sources, parsed: HashMap::new() };
        let mut materials = material::MaterialParser::new();
        let mut noise_settings = HashMap::new();
        for (name, source) in crate::core::data_registries::present_json_entries("minecraft:worldgen/noise_settings") {
            let json: Value = serde_json::from_str(source).map_err(|e| format!("{name}: {e}"))?;
            let settings = Self::parse_settings(name, &json, &mut parser, &mut materials).map_err(|e| format!("{name}: {e}"))?;
            noise_settings.insert(name.to_string(), Arc::new(settings));
        }
        Ok(WorldgenRegistries { noises, noise_settings })
    }

    pub fn parse_inline_settings(&self, json: &Value) -> Result<Arc<NoiseGeneratorSettings>, String> {
        let sources: HashMap<&'static str, &'static str> = crate::core::data_registries::present_json_entries("minecraft:worldgen/density_function").copied().collect();
        let mut parser = Parser { noises: &self.noises, sources: &sources, parsed: HashMap::new() };
        let mut materials = material::MaterialParser::new();
        Ok(Arc::new(Self::parse_settings("inline", json, &mut parser, &mut materials)?))
    }

    fn parse_settings(
        name: &str,
        json: &Value,
        parser: &mut Parser,
        materials: &mut material::MaterialParser,
    ) -> Result<NoiseGeneratorSettings, String> {
        let field = |key: &str| json.get(key).ok_or_else(|| crate::util::codec::no_key(key, json));
        let noise = field("noise")?;
        let router = field("noise_router")?;
        let mut f = |key: &str| parser.parse(router.get(key).ok_or_else(|| format!("missing noise_router.{key}"))?);
        let noise_router = NoiseRouter {
            temperature: f("temperature")?,
            vegetation: f("vegetation")?,
            continents: f("continents")?,
            erosion: f("erosion")?,
            depth: f("depth")?,
            ridges: f("ridges")?,
            chunk_surface_level: f("chunk_surface_level")?,
            final_density: f("final_density")?,
        };
        let aquifers = match json.get("aquifers") {
            Some(a) => {
                let mut f = |key: &str| parser.parse(a.get(key).ok_or_else(|| format!("missing aquifers.{key}"))?);
                Some(AquiferConfig {
                    barrier: f("barrier")?,
                    fluid_level_floodedness: f("fluid_level_floodedness")?,
                    fluid_level_spread: f("fluid_level_spread")?,
                    lava: f("lava")?,
                    exclusion: f("exclusion")?,
                    surface_level: f("surface_level")?,
                })
            }
            None => None,
        };
        let mut spawn_target = Vec::new();
        for point in field("spawn_target")?.as_array().ok_or("spawn_target is a list")? {
            let mut parameters = Vec::new();
            for (function, range) in point.as_object().ok_or("spawn target is a map")? {
                let function = parser.parse(&Value::String(function.clone()))?;
                parameters.push((function, crate::world::level::biome::climate::Parameter::parse(range)?));
            }
            spawn_target.push(SpawnTargetPoint { parameters });
        }
        Ok(NoiseGeneratorSettings {
            name: name.to_owned(),
            noise: NoiseSettings {
                min_y: noise.get("min_y").and_then(Value::as_i64).ok_or("min_y")? as i32,
                height: noise.get("height").and_then(Value::as_i64).ok_or("height")? as i32,
            },
            default_block: parse_block_state(field("default_block")?)?,
            default_fluid: parse_block_state(field("default_fluid")?)?,
            noise_router,
            material_rule: materials.rule(field("material_rule")?, parser)?,
            spawn_target,
            sea_level: field("sea_level")?.as_i64().ok_or("sea_level")? as i32,
            disable_mob_generation: field("disable_mob_generation")?.as_bool().ok_or("disable_mob_generation")?,
            aquifers,
            legacy_random_source: field("legacy_random_source")?.as_bool().ok_or("legacy_random_source")?,
        })
    }
}

fn guard_y(settings: wafer_protocol::nbt::Nbt) -> Result<wafer_protocol::nbt::Nbt, String> {
    use crate::world::item::component::codec::as_i32;
    use crate::world::level::dimension::dimension_type::MAX_Y;
    let min_y = settings.get("min_y").map_or(0, as_i32);
    let height = settings.get("height").map_or(0, as_i32);
    if min_y + height > MAX_Y + 1 {
        Err(format!("min_y + height cannot be higher than: {}", MAX_Y + 1))
    } else if height % 16 != 0 {
        Err("height has to be a multiple of 16".into())
    } else if min_y % 16 != 0 {
        Err("min_y has to be a multiple of 16".into())
    } else {
        Ok(settings)
    }
}

pub fn noise_settings_codec() -> crate::world::item::component::types::Pair {
    use crate::world::item::component::codec::{Codec, RangeError, Stream};
    use crate::world::item::component::types::req;
    use crate::world::level::dimension::dimension_type::{MAX_Y, MIN_Y, Y_SIZE};
    let fields = vec![
        req("min_y", (Codec::Int(MIN_Y, MAX_Y, RangeError::Outside), Stream::Int)),
        req("height", (Codec::Int(0, Y_SIZE, RangeError::Outside), Stream::Int)),
    ];
    (Codec::Normalize(Box::new(Codec::Record(fields)), guard_y), Stream::Nbt)
}

pub fn noise_router_codec() -> crate::world::item::component::types::Pair {
    use crate::world::item::component::types::{record, req};
    let function = density_function::holder_codec;
    record(vec![
        req("temperature", function()),
        req("vegetation", function()),
        req("continents", function()),
        req("erosion", function()),
        req("depth", function()),
        req("ridges", function()),
        req("chunk_surface_level", function()),
        req("final_density", function()),
    ])
}

pub fn spawn_target_point_codec() -> crate::world::item::component::types::Pair {
    use crate::world::item::component::codec::{Codec, Stream};
    (
        Codec::Map(Box::new(density_function::reference_codec().0), Box::new(crate::world::level::biome::climate::parameter_codec().0)),
        Stream::Nbt,
    )
}

fn debug_function_entry_codec() -> crate::world::item::component::types::Pair {
    use crate::world::item::component::types::{self, record, req};
    record(vec![req("label", types::string()), req("function", density_function::holder_codec())])
}

pub fn noise_settings_direct_codec() -> crate::world::item::component::types::Pair {
    use crate::world::item::component::codec::{Codec, Stream};
    use crate::world::item::component::types::{self, def, opt, record, req};
    use crate::world::level::block::state::block_state;
    record(vec![
        req("noise", noise_settings_codec()),
        req("default_block", block_state::codec()),
        req("default_fluid", block_state::codec()),
        req("noise_router", noise_router_codec()),
        req("material_rule", material::rule::material_rule::holder_codec()),
        req("spawn_target", types::list(spawn_target_point_codec())),
        req("sea_level", (Codec::int(), Stream::Int)),
        req("disable_mob_generation", types::bool()),
        opt("aquifers", aquifer::config_codec()),
        req("legacy_random_source", types::bool()),
        def("debug_functions", (Codec::Framed(Box::new(types::list(debug_function_entry_codec()).0), crate::world::item::component::codec::Wrap::Xmap), Stream::Nbt), types::empty_list()),
    ])
}

pub fn noise_settings_holder_codec() -> crate::world::item::component::types::Pair {
    crate::world::item::component::types::holder("minecraft:worldgen/noise_settings", noise_settings_direct_codec())
}
