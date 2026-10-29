use std::sync::{LazyLock, OnceLock};

use wafer_core::random::{LegacyRandomSource, RandomSource};
use wafer_core::{BlockPos, mth};
use wafer_data::BlockState;
use wafer_protocol::nbt::Nbt;
use serde_json::Value;

use super::pos_rule_test::{self, PosRuleTest};
use super::{blackstone_replace_processor, block_age_processor, lava_submerged_block_processor};
use super::rule_test::{self, RuleTest};
use super::structure_place_settings::StructurePlaceSettings;
use super::structure_template::StructureBlockInfo;
use crate::util::codec::DecodeResult;
use crate::world::item::component::codec::{Codec, Dispatch, Selector, Stream};
use crate::world::item::component::types::{self, Pair, def, lenient_opt, opt, req};
use crate::world::level::block::state::block_state;
use crate::world::level::world_gen_level::WorldGenLevel;
use crate::util::valueproviders::{IntProvider, float, positive_int_provider_codec};
use crate::world::level::chunk::HeightmapType;
use crate::world::level::levelgen::placement::placement_modifier::parse_heightmap;
use wafer_data::blocks;
use crate::world::level::levelgen::blockpredicates::block_predicate::{BlockSet, resource};
use crate::world::level::levelgen::parse_block_state;

pub enum StructureProcessor {
    BlockIgnore(Vec<wafer_data::Block>),
    BlockRot { rottable_blocks: Option<BlockSet>, integrity: f32 },
    Gravity { heightmap: HeightmapType, offset: i32 },
    JigsawReplacement,
    ProtectedBlocks(BlockSet),
    Rule(Vec<ProcessorRule>),
    Capped { delegate: Box<StructureProcessor>, limit: IntProvider },
    Nop,
    BlockAge { mossiness: f32 },
    LavaSubmergedBlock,
    BlackstoneReplace,
    NotImplemented,
}

pub struct ProcessorRule {
    input_predicate: RuleTest,
    loc_predicate: RuleTest,
    pos_predicate: PosRuleTest,
    output_state: BlockState,
    block_entity_modifier: RuleBlockEntityModifier,
}

pub enum RuleBlockEntityModifier {
    Passthrough,
    Clear,
    AppendLoot(String),
    AppendStatic(Vec<(String, Nbt)>),
}

fn put(tag: &mut Vec<(String, Nbt)>, key: &str, value: Nbt) {
    match tag.iter_mut().find(|(k, _)| k == key) {
        Some((_, existing)) => *existing = value,
        None => tag.push((key.to_owned(), value)),
    }
}

impl RuleBlockEntityModifier {
    pub fn codec() -> Pair {
        let cases = vec![
            ("minecraft:clear", vec![]),
            ("minecraft:passthrough", vec![]),
            ("minecraft:append_static", vec![req("data", (Codec::Func(compound_tag), Stream::Nbt))]),
            ("minecraft:append_loot", vec![req("loot_table", types::identifier())]),
        ];
        (Codec::Dispatch(Box::new(Dispatch { key: "type", selector: Selector::Registry("minecraft:rule_block_entity_modifier"), cases })), Stream::Dispatch)
    }

    fn parse(json: Option<&Value>) -> Result<RuleBlockEntityModifier, String> {
        let Some(json) = json else { return Ok(RuleBlockEntityModifier::Passthrough) };
        let ty = json.get("type").and_then(Value::as_str).ok_or("block entity modifier without type")?;
        Ok(match ty.strip_prefix("minecraft:").unwrap_or(ty) {
            "clear" => RuleBlockEntityModifier::Clear,
            "passthrough" => RuleBlockEntityModifier::Passthrough,
            "append_loot" => RuleBlockEntityModifier::AppendLoot(resource(json.get("loot_table").and_then(Value::as_str).ok_or("missing loot_table")?)),
            "append_static" => match json.get("data").and_then(crate::nbt::from_json) {
                Some(Nbt::Compound(tag)) => RuleBlockEntityModifier::AppendStatic(tag),
                _ => return Err("append_static without compound data".into()),
            },
            other => return Err(format!("unknown block entity modifier {other}")),
        })
    }

    fn apply(&self, random: &mut impl RandomSource, existing_tag: Option<Nbt>) -> Option<Nbt> {
        match self {
            RuleBlockEntityModifier::Passthrough => existing_tag,
            RuleBlockEntityModifier::Clear => Some(Nbt::Compound(Vec::new())),
            RuleBlockEntityModifier::AppendLoot(loot_table) => {
                let mut result = match existing_tag {
                    Some(Nbt::Compound(tag)) => tag,
                    _ => Vec::new(),
                };
                put(&mut result, "LootTable", Nbt::String(loot_table.clone()));
                put(&mut result, "LootTableSeed", Nbt::Long(random.next_long()));
                Some(Nbt::Compound(result))
            }
            RuleBlockEntityModifier::AppendStatic(tag) => match existing_tag {
                Some(Nbt::Compound(mut existing)) => {
                    crate::nbt::merge(&mut existing, tag);
                    Some(Nbt::Compound(existing))
                }
                _ => Some(Nbt::Compound(tag.clone())),
            },
        }
    }
}

impl ProcessorRule {
    pub fn new(input_predicate: RuleTest, loc_predicate: RuleTest, pos_predicate: PosRuleTest, output_state: BlockState, block_entity_modifier: RuleBlockEntityModifier) -> ProcessorRule {
        ProcessorRule { input_predicate, loc_predicate, pos_predicate, output_state, block_entity_modifier }
    }

    pub fn codec() -> Pair {
        types::record(vec![
            req("input_predicate", (Codec::Lazy(rule_test::codec_lazy), Stream::Lazy)),
            req("location_predicate", (Codec::Lazy(rule_test::codec_lazy), Stream::Lazy)),
            lenient_opt("position_predicate", pos_rule_test::codec()),
            req("output_state", block_state::codec()),
            lenient_opt("block_entity_modifier", RuleBlockEntityModifier::codec()),
        ])
    }

    fn parse(json: &Value) -> Result<ProcessorRule, String> {
        let field = |key: &str| json.get(key).ok_or_else(|| crate::util::codec::no_key(key, json));
        Ok(ProcessorRule {
            input_predicate: RuleTest::parse(field("input_predicate")?)?,
            loc_predicate: RuleTest::parse(field("location_predicate")?)?,
            pos_predicate: json.get("position_predicate").map(PosRuleTest::parse).transpose()?.unwrap_or(PosRuleTest::AlwaysTrue),
            output_state: parse_block_state(field("output_state")?)?,
            block_entity_modifier: RuleBlockEntityModifier::parse(json.get("block_entity_modifier"))?,
        })
    }

    fn test(
        &self,
        level: &mut impl WorldGenLevel,
        input_state: BlockState,
        in_template_pos: BlockPos,
        world_pos: BlockPos,
        reference: BlockPos,
        random: &mut impl RandomSource,
    ) -> bool {
        self.input_predicate.test(input_state, world_pos, random)
            && self.loc_predicate.test(level.get_block_state(world_pos), world_pos, random)
            && self.pos_predicate.test(in_template_pos, world_pos, reference, random)
    }
}

impl StructureProcessor {
    pub fn parse(json: &Value) -> Result<StructureProcessor, String> {
        let ty = json.get("processor_type").and_then(Value::as_str).ok_or("processor without processor_type")?;
        Ok(match ty.strip_prefix("minecraft:").unwrap_or(ty) {
            "block_ignore" => StructureProcessor::BlockIgnore(
                json.get("blocks")
                    .and_then(Value::as_array)
                    .ok_or("missing blocks")?
                    .iter()
                    .map(|state| parse_block_state(state).map(|state| state.block()))
                    .collect::<Result<_, _>>()?,
            ),
            "gravity" => StructureProcessor::Gravity {
                heightmap: parse_heightmap(json.get("heightmap").and_then(Value::as_str).unwrap_or("WORLD_SURFACE_WG"))?,
                offset: json.get("offset").and_then(Value::as_i64).unwrap_or(0) as i32,
            },
            "jigsaw_replacement" => StructureProcessor::JigsawReplacement,
            "capped" => StructureProcessor::Capped {
                delegate: Box::new(StructureProcessor::parse(json.get("delegate").ok_or("missing delegate")?)?),
                limit: IntProvider::parse(json.get("limit").ok_or("missing limit")?)?,
            },
            "nop" => StructureProcessor::Nop,
            "block_rot" => StructureProcessor::BlockRot {
                rottable_blocks: json.get("rottable_blocks").map(BlockSet::parse).transpose()?,
                integrity: float(json.get("integrity").ok_or("missing integrity")?)?,
            },
            "protected_blocks" => StructureProcessor::ProtectedBlocks(BlockSet::parse(json.get("value").ok_or("missing value")?)?),
            "rule" => StructureProcessor::Rule(json.get("rules").and_then(Value::as_array).ok_or("missing rules")?.iter().map(ProcessorRule::parse).collect::<Result<_, _>>()?),
            "block_age" => StructureProcessor::BlockAge { mossiness: float(json.get("mossiness").ok_or("missing mossiness")?)? },
            "lava_submerged_block" => StructureProcessor::LavaSubmergedBlock,
            "blackstone_replace" => StructureProcessor::BlackstoneReplace,
            _ => StructureProcessor::NotImplemented,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn process_block<R: RandomSource>(
        &self,
        level: &mut impl WorldGenLevel,
        _target_position: BlockPos,
        reference_pos: BlockPos,
        template_relative_pos: BlockPos,
        processed_block_info: StructureBlockInfo,
        settings: &StructurePlaceSettings,
        random: &mut R,
    ) -> Option<StructureBlockInfo> {
        match self {
            StructureProcessor::BlockIgnore(to_ignore) => {
                if to_ignore.contains(&processed_block_info.state.block()) { None } else { Some(processed_block_info) }
            }
            StructureProcessor::BlockRot { rottable_blocks, integrity } => {
                let mut random = settings.get_random(processed_block_info.pos, random);
                let rottable = rottable_blocks.as_ref().is_none_or(|blocks| blocks.contains(processed_block_info.state.block()));
                if rottable && random.next_float() > *integrity { None } else { Some(processed_block_info) }
            }
            StructureProcessor::ProtectedBlocks(cannot_replace) => {
                if cannot_replace.contains(level.get_block_state(processed_block_info.pos).block()) { None } else { Some(processed_block_info) }
            }
            StructureProcessor::Rule(rules) => {
                let pos = processed_block_info.pos;
                let mut random = LegacyRandomSource::new(mth::get_seed(pos.x, pos.y, pos.z));
                for rule in rules {
                    if rule.test(level, processed_block_info.state, template_relative_pos, pos, reference_pos, &mut random) {
                        let nbt = rule.block_entity_modifier.apply(&mut random, processed_block_info.nbt);
                        return Some(StructureBlockInfo { pos, state: rule.output_state, nbt });
                    }
                }
                Some(processed_block_info)
            }
            StructureProcessor::Gravity { heightmap, offset } => {
                let pos = processed_block_info.pos;
                let height = level.get_height(*heightmap, pos.x, pos.z) + offset;
                let delta = template_relative_pos.y;
                Some(StructureBlockInfo { pos: BlockPos::new(pos.x, height + delta, pos.z), state: processed_block_info.state, nbt: processed_block_info.nbt })
            }
            StructureProcessor::JigsawReplacement => {
                if !processed_block_info.state.is_block(blocks::JIGSAW) {
                    return Some(processed_block_info);
                }
                let Some(nbt) = &processed_block_info.nbt else {
                    warn!("Jigsaw block at {} is missing nbt, will not replace", _target_position);
                    return Some(processed_block_info);
                };
                let state_string = nbt.get("final_state").and_then(Nbt::as_str).unwrap_or("minecraft:air");
                let Some(block_state) = parse_block_state_string(state_string) else {
                    error!("Failed to parse jigsaw replacement state '{state_string}' at {}", _target_position);
                    return None;
                };
                if block_state.is_block(blocks::STRUCTURE_VOID) {
                    None
                } else {
                    Some(StructureBlockInfo { pos: processed_block_info.pos, state: block_state, nbt: None })
                }
            }
            StructureProcessor::BlockAge { mossiness } => block_age_processor::process_block(*mossiness, processed_block_info, settings, random),
            StructureProcessor::LavaSubmergedBlock => lava_submerged_block_processor::process_block(level, processed_block_info),
            StructureProcessor::BlackstoneReplace => blackstone_replace_processor::process_block(processed_block_info),
            StructureProcessor::Capped { .. } | StructureProcessor::Nop | StructureProcessor::NotImplemented => Some(processed_block_info),
        }
    }

    pub fn evaluates_entire_piece_state(&self) -> bool {
        matches!(self, StructureProcessor::Capped { .. })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn finalize_processing<R: RandomSource>(
        &self,
        level: &mut impl WorldGenLevel,
        position: BlockPos,
        reference_pos: BlockPos,
        original_block_info_list: &[&StructureBlockInfo],
        processed_block_info_list: &mut [StructureBlockInfo],
        settings: &StructurePlaceSettings,
        random: &mut R,
    ) {
        let StructureProcessor::Capped { delegate, limit } = self else { return };
        if limit.max_inclusive() == 0 || processed_block_info_list.is_empty() {
            return;
        }
        if original_block_info_list.len() != processed_block_info_list.len() {
            return;
        }
        let mut positional = LegacyRandomSource::new(level.get_seed()).fork_positional().at(position.x, position.y, position.z);
        let max_to_replace = limit.sample(&mut positional).min(processed_block_info_list.len() as i32);
        if max_to_replace < 1 {
            return;
        }
        let mut indices: Vec<usize> = (0..processed_block_info_list.len()).collect();
        for i in (2..=indices.len()).rev() {
            let swap_to = positional.next_int_bounded(i as i32) as usize;
            indices.swap(i - 1, swap_to);
        }
        let mut replaced = 0;
        for index in indices {
            if replaced >= max_to_replace {
                break;
            }
            let original_block_info = original_block_info_list[index];
            let processed_block_info = processed_block_info_list[index].clone();
            let maybe_altered = delegate.process_block(level, position, reference_pos, original_block_info.pos, processed_block_info.clone(), settings, random);
            if let Some(maybe_altered) = maybe_altered
                && maybe_altered != processed_block_info
            {
                replaced += 1;
                processed_block_info_list[index] = maybe_altered;
            }
        }
    }
}

pub fn parse_block_state_string(state_string: &str) -> Option<BlockState> {
    let utf16: Vec<u16> = state_string.encode_utf16().collect();
    let mut reader = crate::commands::brigadier::StringReader::new(&utf16);
    crate::commands::arguments::blocks::block_state_parser::parse_for_block(&mut reader, true).ok().map(|result| result.state)
}

pub struct StructureProcessorList {
    pub list: Vec<StructureProcessor>,
}

impl StructureProcessorList {
    pub fn parse(json: &Value) -> Result<StructureProcessorList, String> {
        let processors = json.get("processors").and_then(Value::as_array).ok_or("processor list without processors")?;
        Ok(StructureProcessorList { list: processors.iter().map(StructureProcessor::parse).collect::<Result<_, _>>()? })
    }
}

static PROCESSOR_LISTS: LazyLock<Vec<OnceLock<StructureProcessorList>>> = LazyLock::new(|| (0..crate::core::data_registries::json_entries("minecraft:worldgen/processor_list").len()).map(|_| OnceLock::new()).collect());

pub enum StructureProcessorListHolder {
    Reference(usize),
    Direct(StructureProcessorList),
}

impl StructureProcessorListHolder {
    pub fn parse(json: &Value) -> Result<StructureProcessorListHolder, String> {
        match json {
            Value::String(name) => {
                let name = resource(name);
                crate::core::data_registries::json_entries("minecraft:worldgen/processor_list")
                    .iter()
                    .position(|(n, _)| *n == name)
                    .map(StructureProcessorListHolder::Reference)
                    .ok_or_else(|| format!("unknown processor list {name}"))
            }
            Value::Array(list) => Ok(StructureProcessorListHolder::Direct(StructureProcessorList { list: list.iter().map(StructureProcessor::parse).collect::<Result<_, _>>()? })),
            other => Ok(StructureProcessorListHolder::Direct(StructureProcessorList::parse(other)?)),
        }
    }

    pub fn to_nbt(&self) -> Nbt {
        match self {
            StructureProcessorListHolder::Reference(index) => Nbt::String(crate::core::data_registries::json_entries("minecraft:worldgen/processor_list")[*index].0.to_owned()),
            StructureProcessorListHolder::Direct(_) => Nbt::Compound(vec![("processors".to_owned(), Nbt::List(Vec::new()))]),
        }
    }

    pub fn value(&self) -> &StructureProcessorList {
        match self {
            StructureProcessorListHolder::Reference(index) => PROCESSOR_LISTS[*index].get_or_init(|| {
                let (name, source) = crate::core::data_registries::json_entries("minecraft:worldgen/processor_list")[*index];
                let json: Value = serde_json::from_str(source).expect("processor list json");
                StructureProcessorList::parse(&json).unwrap_or_else(|e| panic!("processor list {name}: {e}"))
            }),
            StructureProcessorListHolder::Direct(list) => list,
        }
    }
}

const HEIGHTMAP_TYPES: &[&str] = &["WORLD_SURFACE_WG", "WORLD_SURFACE", "OCEAN_FLOOR_WG", "OCEAN_FLOOR", "MOTION_BLOCKING", "MOTION_BLOCKING_NO_LEAVES"];

fn single_codec_lazy() -> &'static Pair {
    static CODEC: LazyLock<Pair> = LazyLock::new(single_codec);
    &CODEC
}

fn list_direct_codec_lazy() -> &'static Pair {
    static CODEC: LazyLock<Pair> = LazyLock::new(list_direct_codec);
    &CODEC
}

pub fn single_codec() -> Pair {
    let cases = vec![
        ("minecraft:blackstone_replace", vec![]),
        ("minecraft:block_age", vec![req("mossiness", types::float(Codec::float()))]),
        ("minecraft:block_ignore", vec![req("blocks", types::list(block_state::codec()))]),
        ("minecraft:block_rot", vec![opt("rottable_blocks", types::holder_set("minecraft:block")), req("integrity", types::float(types::codec_float_range(0.0, 1.0)))]),
        ("minecraft:capped", vec![req("delegate", (Codec::Lazy(single_codec_lazy), Stream::Lazy)), req("limit", positive_int_provider_codec())]),
        ("minecraft:gravity", vec![def("heightmap", types::enumeration(HEIGHTMAP_TYPES), Nbt::String("WORLD_SURFACE_WG".into())), def("offset", (Codec::int(), Stream::Int), Nbt::Int(0))]),
        ("minecraft:jigsaw_replacement", vec![]),
        ("minecraft:lava_submerged_block", vec![]),
        ("minecraft:nop", vec![]),
        ("minecraft:protected_blocks", vec![req("value", types::holder_set("minecraft:block"))]),
        ("minecraft:rule", vec![req("rules", types::list(ProcessorRule::codec()))]),
    ];
    (Codec::Dispatch(Box::new(Dispatch { key: "processor_type", selector: Selector::Registry("minecraft:worldgen/structure_processor"), cases })), Stream::Dispatch)
}

pub fn list_object_codec() -> Pair {
    types::list((Codec::Lazy(single_codec_lazy), Stream::Lazy))
}

pub fn list_direct_codec() -> Pair {
    let primary = types::record(vec![req("processors", list_object_codec())]);
    (Codec::Either(Box::new(primary.0), Box::new(list_object_codec().0)), Stream::Nbt)
}

pub fn list_holder_codec() -> Pair {
    (Codec::Holder("minecraft:worldgen/processor_list", Some(Box::new(Codec::Lazy(list_direct_codec_lazy)))), Stream::Nbt)
}

fn convert_and_print(tag: &Nbt) -> String {
    let narrowest = |value: i64| {
        if value as i8 as i64 == value {
            Nbt::Byte(value as i8)
        } else if value as i16 as i64 == value {
            Nbt::Short(value as i16)
        } else if value as i32 as i64 == value {
            Nbt::Int(value as i32)
        } else {
            Nbt::Long(value)
        }
    };
    match tag {
        Nbt::List(items) => format!("[{}]", items.iter().map(convert_and_print).collect::<Vec<_>>().join(",")),
        Nbt::Long(value) => crate::nbt::to_snbt(&narrowest(*value)),
        Nbt::Double(value) if value.fract() == 0.0 && value.abs() < 9.2e18 => crate::nbt::to_snbt(&narrowest(*value as i64)),
        Nbt::Double(value) if f64::from(*value as f32) == *value => crate::nbt::to_snbt(&Nbt::Float(*value as f32)),
        other => crate::nbt::to_snbt(other),
    }
}

fn contains_null(tag: &Nbt) -> bool {
    match tag {
        Nbt::End => true,
        Nbt::List(items) => items.iter().any(contains_null),
        Nbt::Compound(fields) => fields.iter().any(|(_, value)| contains_null(value)),
        _ => false,
    }
}

fn compound_tag(tag: &Nbt) -> DecodeResult<Nbt> {
    if contains_null(tag) {
        crate::core::data_registries::throw(
            "java.lang.NullPointerException",
            "Cannot invoke \"com.google.gson.JsonElement.getAsJsonPrimitive()\" because \"input\" is null".into(),
            &["com.mojang.serialization.JsonOps.convertTo(JsonOps.java:61)", "com.mojang.serialization.JsonOps.convertTo(JsonOps.java:25)"],
        );
    }
    match tag {
        Nbt::Compound(_) => DecodeResult::success(tag.clone()),
        other => DecodeResult::error(format!("Not a compound tag: {}", convert_and_print(other))),
    }
}
