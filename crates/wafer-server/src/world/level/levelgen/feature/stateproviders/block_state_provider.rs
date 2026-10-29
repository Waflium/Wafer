use wafer_core::java_string::JavaStr;
use std::sync::LazyLock;

use wafer_core::random::{LegacyRandomSource, RandomSource};
use wafer_core::{BlockPos, Direction, mth};
use wafer_data::properties::{AXIS, FACING, HORIZONTAL_FACING};
use wafer_data::{Block, BlockState};
use serde_json::Value;
use wafer_protocol::nbt::Nbt;

use crate::core::holder_set::HolderSet;
use crate::world::item::component::codec::{Codec, Dispatch, Selector, Stream};
use crate::world::item::component::json_to_nbt;
use crate::world::item::component::types::{self, Pair, opt, record, req};
use crate::world::level::block::state::block_state;
use crate::world::level::levelgen::blockpredicates::block_predicate;
use crate::world::level::levelgen::synth::parameters_codec;
use crate::world::level::world_gen_level::WorldGenLevel;
use crate::util::inclusive_range::InclusiveRange;
use crate::util::random::WeightedList;
use crate::util::valueproviders::{IntProvider, float, int_provider_codec};
use crate::world::level::levelgen::blockpredicates::block_predicate::{BlockPredicate, resource};
use crate::world::level::levelgen::synth::NoiseStack;
use crate::world::level::levelgen::{AlgorithmRandom, parse_block_state, parse_noise};

fn direct_codec_lazy() -> &'static Pair {
    static CODEC: LazyLock<Pair> = LazyLock::new(direct_codec);
    &CODEC
}

pub fn holder_codec() -> Pair {
    provider_holder()
}

fn provider_holder() -> Pair {
    (Codec::Holder("minecraft:worldgen/block_state_provider", Some(Box::new(Codec::Lazy(direct_codec_lazy)))), Stream::Nbt)
}

fn simple_provider_to_full(provider: Nbt) -> Result<Nbt, String> {
    match provider.get("type").and_then(Nbt::as_str) {
        Some("minecraft:simple") => block_state::to_full(provider.get("state").expect("a simple provider state")),
        _ => Ok(provider),
    }
}

pub fn direct_codec() -> Pair {
    let noise = || vec![req("seed", (Codec::long(), Stream::Long)), req("noise", parameters_codec()), req("scale", types::float(types::positive_float()))];
    let noise_provider = || {
        let mut fields = noise();
        fields.push(req("states", types::non_empty_list(block_state::codec())));
        fields
    };
    let mut dual_noise = vec![req("variety", InclusiveRange::int_codec_in::<1, 64>()), req("slow_noise", parameters_codec()), req("slow_scale", types::float(types::positive_float()))];
    dual_noise.extend(noise_provider());
    let mut noise_threshold = noise();
    noise_threshold.extend([
        req("threshold", types::float(types::codec_float_range(-1.0, 1.0))),
        req("high_chance", types::float(types::codec_float_range(0.0, 1.0))),
        req("default_state", block_state::codec()),
        req("low_states", types::non_empty_list(block_state::codec())),
        req("high_states", types::non_empty_list(block_state::codec())),
    ]);
    let cases = vec![
        ("minecraft:copy_properties", vec![req("source", provider_holder())]),
        ("minecraft:dual_noise", dual_noise),
        ("minecraft:noise", noise_provider()),
        ("minecraft:noise_threshold", noise_threshold),
        ("minecraft:random_block", vec![req("blocks", types::holder_set("minecraft:block"))]),
        ("minecraft:randomized_int", vec![req("source", provider_holder()), req("property", types::string()), req("values", int_provider_codec())]),
        ("minecraft:rotated", vec![req("state", provider_holder()), opt("direction", types::enumeration(&["down", "up", "north", "south", "west", "east"]))]),
        (
            "minecraft:rule_based",
            vec![
                opt("fallback", provider_holder()),
                req("rules", types::list(record(vec![req("if_true", block_predicate::codec()), req("then", provider_holder())]))),
            ],
        ),
        ("minecraft:simple", vec![req("state", block_state::codec())]),
        ("minecraft:weighted", vec![req("entries", types::non_empty_weighted_list(record(vec![req("data", block_state::codec()), req("weight", types::var_int(types::non_negative_int()))])))]),
    ];
    let typed = Codec::Normalize(
        Box::new(Codec::Dispatch(Box::new(Dispatch { key: "type", selector: Selector::Registry("minecraft:worldgen/block_state_provider_type"), cases }))),
        simple_provider_to_full,
    );
    (Codec::Xor(Box::new(block_state::full_codec().0), Box::new(typed), block_state::java_to_string, provider_to_string), Stream::Nbt)
}

fn object_to_string(class: &str) -> String {
    let hash = class.java_utf16().fold(0i32, |hash, unit| hash.wrapping_mul(31).wrapping_add(unit as i32));
    format!("{class}@{hash:x}")
}

fn holder_to_string(holder: &Nbt) -> String {
    match holder {
        Nbt::String(id) => format!("Reference{{ResourceKey[minecraft:worldgen/block_state_provider / {id}]=null}}"),
        other => format!("Direct{{{}}}", provider_to_string(other)),
    }
}

fn block_holder_to_string(id: &str) -> String {
    format!("Reference{{ResourceKey[minecraft:block / {id}]=Block{{{id}}}}}")
}

fn provider_to_string(provider: &Nbt) -> String {
    const PACKAGE: &str = "net.minecraft.world.level.levelgen.feature.stateproviders";
    match provider.get("type").and_then(Nbt::as_str) {
        None => format!("SimpleStateProvider[state={}]", block_state::java_to_string(provider)),
        Some("minecraft:copy_properties") => format!("CopyPropertiesProvider[source={}]", provider.get("source").map_or_else(String::new, holder_to_string)),
        Some("minecraft:rotated") => {
            let direction = provider.get("direction").and_then(Nbt::as_str).map_or_else(|| "Optional.empty".to_string(), |direction| format!("Optional[{direction}]"));
            format!("RotatedBlockProvider[state={}, direction={direction}]", provider.get("state").map_or_else(String::new, holder_to_string))
        }
        Some("minecraft:random_block") => match provider.get("blocks") {
            Some(Nbt::String(id)) if !id.starts_with('#') => format!("RandomBlockProvider[blocks=DirectSet[[{}]]]", block_holder_to_string(id)),
            Some(Nbt::List(ids)) if ids.iter().all(|id| id.as_str().is_some_and(|id| !id.starts_with('#'))) => {
                let holders: Vec<String> = ids.iter().filter_map(Nbt::as_str).map(block_holder_to_string).collect();
                format!("RandomBlockProvider[blocks=DirectSet[[{}]]]", holders.join(", "))
            }
            _ => object_to_string(&format!("{PACKAGE}.RandomBlockProvider")),
        },
        Some("minecraft:weighted") => format!("WeightedStateProvider[weightedList={}]", object_to_string("net.minecraft.util.random.WeightedList")),
        Some("minecraft:dual_noise") => object_to_string(&format!("{PACKAGE}.DualNoiseProvider")),
        Some("minecraft:noise") => object_to_string(&format!("{PACKAGE}.NoiseProvider")),
        Some("minecraft:noise_threshold") => object_to_string(&format!("{PACKAGE}.NoiseThresholdProvider")),
        Some("minecraft:randomized_int") => object_to_string(&format!("{PACKAGE}.RandomizedIntStateProvider")),
        Some("minecraft:rule_based") => object_to_string(&format!("{PACKAGE}.RuleBasedStateProvider")),
        Some(other) => object_to_string(other),
    }
}

pub enum BlockStateProvider {
    Simple(BlockState),
    Weighted(WeightedList<BlockState>),
    Noise(NoiseProvider),
    DualNoise { variety: (i32, i32), slow_noise: NoiseStack, slow_scale: f32, provider: NoiseProvider },
    NoiseThreshold { noise: NoiseStack, scale: f32, threshold: f32, high_chance: f32, default_state: BlockState, low_states: Vec<BlockState>, high_states: Vec<BlockState> },
    RandomBlock(HolderSet),
    RandomizedInt { source: BlockStateProviderHolder, property: String, values: IntProvider },
    Rotated { state: BlockStateProviderHolder, direction: Option<Direction> },
    RuleBased { fallback: Option<BlockStateProviderHolder>, rules: Vec<(BlockPredicate, BlockStateProviderHolder)> },
    CopyProperties(BlockStateProviderHolder),
}

pub struct NoiseProvider {
    noise: NoiseStack,
    scale: f32,
    states: Vec<BlockState>,
}

impl NoiseProvider {
    fn parse(json: &Value) -> Result<NoiseProvider, String> {
        let (noise, scale) = parse_noise_based(json)?;
        Ok(NoiseProvider { noise, scale, states: parse_states(json.get("states").ok_or("missing states")?)? })
    }

    fn get_noise_value(&self, pos: BlockPos, scale: f64) -> f32 {
        get_noise_value(&self.noise, pos, scale)
    }

    fn get_random_state_at(&self, states: &[BlockState], pos: BlockPos, scale: f64) -> BlockState {
        let noise_value = self.get_noise_value(pos, scale);
        get_random_state(states, noise_value)
    }
}

fn get_noise_value(noise: &NoiseStack, pos: BlockPos, scale: f64) -> f32 {
    noise.get(pos.x as f64 * scale, pos.y as f64 * scale, pos.z as f64 * scale)
}

fn get_random_state(states: &[BlockState], noise_value: f32) -> BlockState {
    let placement_value = ((1.0 + noise_value) / 2.0).clamp(0.0, 0.9999);
    states[(placement_value * states.len() as f32) as usize]
}

fn parse_noise_based(json: &Value) -> Result<(NoiseStack, f32), String> {
    let seed = json.get("seed").and_then(Value::as_i64).ok_or("missing seed")?;
    let parameters = parse_noise(json.get("noise").ok_or("missing noise")?)?;
    let noise = parameters.create(&mut AlgorithmRandom::Legacy(LegacyRandomSource::new(seed)));
    Ok((noise, float(json.get("scale").ok_or("missing scale")?)?))
}

fn parse_states(json: &Value) -> Result<Vec<BlockState>, String> {
    json.as_array().ok_or("expected a list of states")?.iter().map(parse_block_state).collect()
}

static BLOCK_STATE_PROVIDERS: LazyLock<Vec<BlockStateProvider>> = LazyLock::new(|| {
    crate::core::data_registries::json_entries("minecraft:worldgen/block_state_provider")
        .iter()
        .map(|(_, json)| {
            let json: Value = serde_json::from_str(json).unwrap_or(Value::Null);
            BlockStateProvider::parse(&json).unwrap_or_else(|_| BlockStateProvider::Simple(Block::by_name("minecraft:air").expect("air").default_state()))
        })
        .collect()
});

pub enum BlockStateProviderHolder {
    Reference(u32),
    Direct(Box<BlockStateProvider>),
}

impl BlockStateProviderHolder {
    pub fn parse(json: &Value) -> Result<BlockStateProviderHolder, String> {
        match json {
            Value::String(name) => {
                let name = resource(name);
                crate::core::data_registries::json_entries("minecraft:worldgen/block_state_provider")
                    .iter()
                    .position(|(n, _)| *n == name)
                    .map(|i| BlockStateProviderHolder::Reference(i as u32))
                    .ok_or_else(|| format!("unknown block state provider {name}"))
            }
            other => Ok(BlockStateProviderHolder::Direct(Box::new(BlockStateProvider::parse(other)?))),
        }
    }

    pub fn value(&self) -> &BlockStateProvider {
        match self {
            BlockStateProviderHolder::Reference(index) => &BLOCK_STATE_PROVIDERS[*index as usize],
            BlockStateProviderHolder::Direct(provider) => provider,
        }
    }
}

impl BlockStateProvider {
    pub fn parse(json: &Value) -> Result<BlockStateProvider, String> {
        let Some(ty) = json.get("type").and_then(Value::as_str) else {
            return Ok(BlockStateProvider::Simple(parse_block_state(json)?));
        };
        let field = |key: &str| json.get(key).ok_or_else(|| crate::util::codec::no_key(key, json));
        Ok(match ty.strip_prefix("minecraft:").unwrap_or(ty) {
            "simple" => BlockStateProvider::Simple(parse_block_state(field("state")?)?),
            "weighted" => BlockStateProvider::Weighted(WeightedList::parse(field("entries")?, parse_block_state)?),
            "noise" => BlockStateProvider::Noise(NoiseProvider::parse(json)?),
            "dual_noise" => {
                let variety = field("variety")?;
                let variety = match variety {
                    Value::Array(v) if v.len() == 2 => (v[0].as_i64().unwrap_or(1) as i32, v[1].as_i64().unwrap_or(1) as i32),
                    Value::Number(n) => (n.as_i64().unwrap_or(1) as i32, n.as_i64().unwrap_or(1) as i32),
                    other => (
                        other.get("min_inclusive").and_then(Value::as_i64).ok_or("bad variety")? as i32,
                        other.get("max_inclusive").and_then(Value::as_i64).ok_or("bad variety")? as i32,
                    ),
                };
                let seed = field("seed")?.as_i64().ok_or("bad seed")?;
                let slow_noise = parse_noise(field("slow_noise")?)?.create(&mut AlgorithmRandom::Legacy(LegacyRandomSource::new(seed)));
                BlockStateProvider::DualNoise { variety, slow_noise, slow_scale: float(field("slow_scale")?)?, provider: NoiseProvider::parse(json)? }
            }
            "noise_threshold" => {
                let (noise, scale) = parse_noise_based(json)?;
                BlockStateProvider::NoiseThreshold {
                    noise,
                    scale,
                    threshold: float(field("threshold")?)?,
                    high_chance: float(field("high_chance")?)?,
                    default_state: parse_block_state(field("default_state")?)?,
                    low_states: parse_states(field("low_states")?)?,
                    high_states: parse_states(field("high_states")?)?,
                }
            }
            "random_block" => BlockStateProvider::RandomBlock(HolderSet::decode("minecraft:block", &json_to_nbt(field("blocks")?))?),
            "randomized_int" => BlockStateProvider::RandomizedInt {
                source: BlockStateProviderHolder::parse(field("source")?)?,
                property: field("property")?.as_str().ok_or("bad property")?.to_owned(),
                values: IntProvider::parse(field("values")?)?,
            },
            "rotated" => BlockStateProvider::Rotated {
                state: BlockStateProviderHolder::parse(field("state")?)?,
                direction: json.get("direction").and_then(Value::as_str).map(|d| Direction::by_name(d).ok_or_else(|| format!("unknown direction {d}"))).transpose()?,
            },
            "rule_based" => BlockStateProvider::RuleBased {
                fallback: json.get("fallback").map(BlockStateProviderHolder::parse).transpose()?,
                rules: field("rules")?
                    .as_array()
                    .ok_or("rules is a list")?
                    .iter()
                    .map(|rule| {
                        Ok((
                            BlockPredicate::parse(rule.get("if_true").ok_or("missing if_true")?)?,
                            BlockStateProviderHolder::parse(rule.get("then").ok_or("missing then")?)?,
                        ))
                    })
                    .collect::<Result<_, String>>()?,
            },
            "copy_properties" => BlockStateProvider::CopyProperties(BlockStateProviderHolder::parse(field("source")?)?),
            other => return Err(format!("unknown block state provider type {other}")),
        })
    }

    pub fn get_state(&self, level: &mut impl WorldGenLevel, random: &mut impl RandomSource, pos: BlockPos) -> BlockState {
        match self {
            BlockStateProvider::Simple(state) => *state,
            BlockStateProvider::Weighted(list) => *list.get_random_or_throw(random),
            BlockStateProvider::Noise(provider) => provider.get_random_state_at(&provider.states, pos, provider.scale as f64),
            BlockStateProvider::DualNoise { variety, slow_noise, slow_scale, provider } => {
                let slow_noise_value = |pos: BlockPos| slow_noise.get(pos.x as f64 * *slow_scale as f64, pos.y as f64 * *slow_scale as f64, pos.z as f64 * *slow_scale as f64);
                let variety_noise = slow_noise_value(pos) as f64;
                let local_variety = mth::clamped_map(variety_noise, -1.0, 1.0, variety.0 as f64, (variety.1 + 1) as f64) as i32;
                let mut possible_states = Vec::with_capacity(local_variety.max(0) as usize);
                for i in 0..local_variety {
                    possible_states.push(get_random_state(&provider.states, slow_noise_value(pos.offset(i * 54545, 0, i * 34234))));
                }
                provider.get_random_state_at(&possible_states, pos, provider.scale as f64)
            }
            BlockStateProvider::NoiseThreshold { noise, scale, threshold, high_chance, default_state, low_states, high_states } => {
                let local_value = get_noise_value(noise, pos, *scale as f64) as f64;
                if local_value < *threshold as f64 {
                    low_states[random.next_int_bounded(low_states.len() as i32) as usize]
                } else if random.next_float() < *high_chance {
                    high_states[random.next_int_bounded(high_states.len() as i32) as usize]
                } else {
                    *default_state
                }
            }
            BlockStateProvider::RandomBlock(_) | BlockStateProvider::RuleBased { .. } => match self.get_optional_state(level, random, pos) {
                Some(state) => state,
                None => level.get_block_state(pos),
            },
            BlockStateProvider::RandomizedInt { source, property, values } => {
                let unmodified_state = source.value().get_state(level, random, pos);
                let is_integer = unmodified_state
                    .block()
                    .property_by_name(property)
                    .is_some_and(|(_, possible)| possible.iter().all(|v| v.parse::<i32>().is_ok()));
                if !is_integer {
                    return unmodified_state;
                }
                let value = values.sample(random).to_string();
                unmodified_state.with_property_by_name(property, &value).unwrap_or_else(|| panic!("Cannot set property {property} to {value} on {:?}", unmodified_state.block()))
            }
            BlockStateProvider::Rotated { state, direction } => {
                let direction = direction.unwrap_or_else(|| Direction::VALUES[random.next_int_bounded(Direction::VALUES.len() as i32) as usize]);
                let new_state = state.value().get_state(level, random, pos).try_set(AXIS, direction.axis()).try_set(FACING, direction);
                if direction.axis().is_horizontal() { new_state.try_set(HORIZONTAL_FACING, direction) } else { new_state }
            }
            BlockStateProvider::CopyProperties(source) => {
                let state = source.value().get_state(level, random, pos);
                state.with_properties_of(level.get_block_state(pos))
            }
        }
    }

    pub fn get_optional_state(&self, level: &mut impl WorldGenLevel, random: &mut impl RandomSource, pos: BlockPos) -> Option<BlockState> {
        match self {
            BlockStateProvider::RandomBlock(blocks) => {
                let blocks = blocks.contents();
                if blocks.is_empty() {
                    return None;
                }
                let block = Block(blocks[random.next_int_bounded(blocks.len() as i32) as usize] as u16);
                Some(block.default_state())
            }
            BlockStateProvider::RuleBased { fallback, rules } => {
                for (if_true, then) in rules {
                    if if_true.test(level, pos)
                        && let Some(state) = then.value().get_optional_state(level, random, pos)
                    {
                        return Some(state);
                    }
                }
                fallback.as_ref().and_then(|fallback| fallback.value().get_optional_state(level, random, pos))
            }
            _ => Some(self.get_state(level, random, pos)),
        }
    }
}
