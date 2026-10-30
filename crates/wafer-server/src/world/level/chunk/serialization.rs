use crate::util::java_throwable::JavaThrowable;
use wafer_core::ChunkPos;
use wafer_core::BlockPos;
use wafer_data::{Block, BlockState, DimensionType, Fluid, WORLD_VERSION, registry};
use wafer_protocol::nbt::Nbt;

use super::data_layer::{self, DataLayer};
use super::palette::{PackedData, PalettedContainer};
use super::proto_chunk::ProtoChunkData;
use super::upgrade_data::UpgradeData;
use super::{Biome, ChunkStructures, Heightmap, HeightmapType, LevelChunk, LevelChunkSection, biome_strategy, block_strategy};
use crate::server::level::chunk_level::ChunkStatus;
use crate::world::level::levelgen::structure::structure::StructureHolder;
use crate::world::level::levelgen::structure::structure_start::StructureStart;
use crate::world::ticks::{LevelChunkTicks, ProtoChunkTicks, SavedTick, TickPriority};

pub struct ReadChunk {
    pub chunk: LevelChunk,
    pub status: ChunkStatus,
    pub block_ticks: LevelChunkTicks<Block>,
    pub fluid_ticks: LevelChunkTicks<Fluid>,
    pub light: Vec<SectionLight>,
}

pub struct SectionLight {
    pub y: i32,
    pub block: Option<DataLayer>,
    pub sky: Option<DataLayer>,
}

fn layer_to_nbt(layer: &DataLayer) -> Nbt {
    Nbt::ByteArray(layer.to_bytes().iter().map(|&b| b as i8).collect())
}

#[derive(Debug)]
pub enum ReadError {
    MissingLevelData,
    Exception(JavaThrowable),
}

const PARSE_TAIL: [&str; 9] = [
    "net.minecraft.server.level.ChunkMap.lambda$scheduleChunkLoad$1(ChunkMap.java:585)",
    "java.base/java.util.Optional.map(Optional.java:260)",
    "net.minecraft.server.level.ChunkMap.lambda$scheduleChunkLoad$0(ChunkMap.java:584)",
    "java.base/java.util.concurrent.CompletableFuture$UniApply.tryFire(CompletableFuture.java:667)",
    "java.base/java.util.concurrent.CompletableFuture$Completion.exec(CompletableFuture.java:504)",
    "java.base/java.util.concurrent.ForkJoinTask.doExec(ForkJoinTask.java:511)",
    "java.base/java.util.concurrent.ForkJoinPool$WorkQueue.topLevelExec(ForkJoinPool.java:1450)",
    "java.base/java.util.concurrent.ForkJoinPool.runWorker(ForkJoinPool.java:2019)",
    "java.base/java.util.concurrent.ForkJoinWorkerThread.run(ForkJoinWorkerThread.java:187)",
];
const OPTIONAL_MAP: &str = "java.base/java.util.Optional.map(Optional.java:260)";
const PARSE_BLOCK_LIGHT: &str = "net.minecraft.world.level.chunk.storage.SerializableChunkData.parse(SerializableChunkData.java:196)";
const PARSE_SKY_LIGHT: &str = "net.minecraft.world.level.chunk.storage.SerializableChunkData.parse(SerializableChunkData.java:197)";
const DATA_LAYER_INIT: &str = "net.minecraft.world.level.chunk.DataLayer.<init>(DataLayer.java:32)";
const GET_OR_THROW: &str = "com.mojang.serialization.DataResult$Error.getOrThrow(DataResult.java:287)";
const CHUNK_READ_EXCEPTION: &str = "net.minecraft.world.level.chunk.storage.SerializableChunkData$ChunkReadException";

fn parse_exception(description: String, frames: &[&'static str]) -> ReadError {
    let mut frames = frames.to_vec();
    frames.extend(PARSE_TAIL);
    ReadError::Exception(JavaThrowable::new(description, frames))
}

fn layer_from_nbt(nbt: Option<&Nbt>, parse_line: &'static str) -> Result<Option<DataLayer>, ReadError> {
    match nbt {
        Some(Nbt::ByteArray(bytes)) => {
            let bytes: Box<[u8; data_layer::SIZE]> = bytes
                .iter()
                .map(|&b| b as u8)
                .collect::<Vec<u8>>()
                .into_boxed_slice()
                .try_into()
                .map_err(|_| parse_exception(format!("java.lang.IllegalArgumentException: DataLayer should be 2048 bytes not: {}", bytes.len()), &[DATA_LAYER_INIT, OPTIONAL_MAP, parse_line]))?;
            Ok(Some(DataLayer::from_bytes(bytes)))
        }
        _ => Ok(None),
    }
}

pub fn fluid_by_name(name: &str) -> Option<Fluid> {
    let id = registry::FLUID.iter().position(|&f| f == name)?;
    Some([Fluid::Empty, Fluid::FlowingWater, Fluid::Water, Fluid::FlowingLava, Fluid::Lava][id])
}

pub fn saved_ticks_to_nbt<T: Copy>(ticks: &[SavedTick<T>], name: impl Fn(T) -> &'static str) -> Nbt {
    Nbt::List(
        ticks
            .iter()
            .map(|t| {
                Nbt::Compound(vec![
                    ("i".into(), Nbt::String(name(t.ty).into())),
                    ("x".into(), Nbt::Int(t.pos.x)),
                    ("y".into(), Nbt::Int(t.pos.y)),
                    ("z".into(), Nbt::Int(t.pos.z)),
                    ("t".into(), Nbt::Int(t.delay)),
                    ("p".into(), Nbt::Int(t.priority.0)),
                ].into())
            })
            .collect(),
    )
}

pub fn saved_ticks_from_nbt<T: Copy + Eq + std::hash::Hash>(nbt: Option<&Nbt>, ty: impl Fn(Option<&Nbt>) -> Option<T>) -> Vec<SavedTick<T>> {
    nbt
        .map(Nbt::as_list)
        .unwrap_or(&[])
        .iter()
        .filter_map(|t| {
            let int = |k: &str| t.get(k).and_then(Nbt::as_i64).map(|v| v as i32);
            Some(SavedTick {
                ty: ty(t.get("i"))?,
                pos: BlockPos::new(int("x")?, int("y")?, int("z")?),
                delay: int("t")?,
                priority: TickPriority::by_value(int("p")?),
            })
        })
        .collect()
}

pub fn filter_tick_list_for_chunk<T>(saved_ticks: Vec<SavedTick<T>>, chunk_pos: ChunkPos) -> Vec<SavedTick<T>> {
    let pos_key = chunk_pos.pack();
    saved_ticks.into_iter().filter(|tick| ChunkPos::new(tick.pos.x >> 4, tick.pos.z >> 4).pack() == pos_key).collect()
}

pub fn state_to_nbt(state: BlockState) -> Nbt {
    let block = state.block();
    if block.default_state() == state {
        return Nbt::String(block.name().into());
    }
    let mut properties: Vec<(String, Nbt)> = state.properties().map(|(name, value)| (name.to_string(), Nbt::String(value.into()))).collect();
    properties.reverse();
    Nbt::Compound(vec![("id".into(), Nbt::String(block.name().into())), ("properties".into(), Nbt::Compound(properties.into()))].into())
}

pub fn state_from_nbt(nbt: &Nbt) -> Option<BlockState> {
    match nbt {
        Nbt::String(name) => Some(Block::by_name(name)?.default_state()),
        Nbt::Compound(_) if nbt.get("").is_some() => state_from_nbt(nbt.get("")?),
        Nbt::Compound(_) => {
            let mut state = Block::by_name(nbt.get("id")?.as_str()?)?.default_state();
            if let Some(Nbt::Compound(props)) = nbt.get("properties") {
                for (name, value) in props {
                    state = state.with_property_by_name(name, value.as_str()?)?;
                }
            }
            Some(state)
        }
        _ => None,
    }
}

pub fn decode_block_state(nbt: &Nbt) -> Result<BlockState, String> {
    let unknown_block = |name: &str| match crate::resources::identifier::read(name) {
        Ok(id) => match Block::by_name(&id) {
            Some(block) => Ok(block),
            None => Err(format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:block]: {id}")),
        },
        Err(error) => Err(error),
    };
    let text = crate::nbt::to_string(nbt);
    let (first, second) = match nbt {
        Nbt::String(name) => match unknown_block(name) {
            Ok(block) => return Ok(block.default_state()),
            Err(error) => (error, format!("Not a map: {text}")),
        },
        Nbt::Compound(_) => {
            let second = match nbt.get("id") {
                None => format!("No key id in MapLike[{text}]"),
                Some(Nbt::String(name)) => match unknown_block(name) {
                    Ok(block) => {
                        let mut state = block.default_state();
                        if let Some(Nbt::Compound(properties)) = nbt.get("properties") {
                            for (name, value) in properties {
                                if let Some(updated) = value.as_str().and_then(|value| state.with_property_by_name(name, value)) {
                                    state = updated;
                                }
                            }
                        }
                        return Ok(state);
                    }
                    Err(error) => error,
                },
                Some(_) => "Not a string".to_string(),
            };
            ("Not a string".to_string(), second)
        }
        _ => ("Not a string".to_string(), format!("Not a map: {text}")),
    };
    Err(format!("Failed to parse either. First: {first}; Second: {second}"))
}

fn decode_biome(nbt: &Nbt) -> Result<Biome, String> {
    let Nbt::String(name) = nbt else { return Err("Not a string".to_string()) };
    let id = crate::resources::identifier::read(name)?;
    Biome::by_name(&id).ok_or_else(|| format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:worldgen/biome]: {id}"))
}

fn mixed_list(items: Vec<Nbt>) -> Nbt {
    Nbt::List(items)
}

fn container_to_nbt<T: super::palette::PaletteValue>(container: &PalettedContainer<T>, entry: impl Fn(T) -> Nbt) -> Nbt {
    let packed = container.pack();
    let mut fields = vec![("palette".to_string(), mixed_list(packed.palette.into_iter().map(entry).collect()))];
    if let Some(data) = packed.storage {
        fields.push(("data".to_string(), Nbt::LongArray(data)));
    }
    Nbt::Compound(fields.into())
}

fn container_from_nbt<T: super::palette::PaletteValue>(
    nbt: &Nbt,
    strategy: super::palette::Strategy,
    default: T,
    entry: impl Fn(&Nbt) -> Result<T, String>,
) -> Result<(PalettedContainer<T>, Option<String>), String> {
    let mut recoverable = Vec::new();
    let palette = match nbt.get("palette") {
        None => return Err(format!("No key palette in MapLike[{}]", crate::nbt::to_string(nbt))),
        Some(Nbt::List(items)) => items
            .iter()
            .map(|item| {
                entry(item).unwrap_or_else(|message| {
                    recoverable.push(format!("({message} -> using default)"));
                    default
                })
            })
            .collect::<Vec<T>>(),
        Some(other) => return Err(format!("Not a list: {}", crate::nbt::to_string(other))),
    };
    let storage = match nbt.get("data") {
        Some(Nbt::LongArray(longs)) => Some(longs.clone()),
        _ => None,
    };
    let recoverable = (!recoverable.is_empty()).then(|| recoverable.into_iter().rev().collect::<Vec<_>>().join("; "));
    PalettedContainer::unpack(strategy, PackedData { palette, storage }).map(|container| (container, recoverable))
}

fn heightmaps_to_nbt(chunk: &LevelChunk, status: ChunkStatus) -> Nbt {
    let mut heightmaps = Vec::new();
    for kind in HeightmapType::VALUES {
        let data = match (HeightmapType::LIVE.iter().position(|&t| t == kind), &chunk.proto) {
            (Some(i), _) => (status >= ChunkStatus::Terrain).then(|| chunk.heightmaps[i].raw()),
            (None, Some(proto)) => {
                let i = HeightmapType::WORLDGEN.iter().position(|&t| t == kind).unwrap();
                proto.heightmaps[i].as_ref().map(|h| h.raw())
            }
            (None, None) => None,
        };
        if let Some(data) = data {
            heightmaps.push((kind.serialization_key().to_string(), Nbt::LongArray(data.to_vec())));
        }
    }
    Nbt::Compound(heightmaps.into())
}

pub fn write(chunk: &LevelChunk, status: ChunkStatus, block_entities: Vec<Nbt>, last_update: i64, block_ticks: &[SavedTick<Block>], fluid_ticks: &[SavedTick<Fluid>], light: &[SectionLight]) -> Nbt {
    let min_section = chunk.min_y() >> 4;
    let mut sections = Vec::new();
    for section_y in min_section - 1..=min_section + chunk.sections.len() as i32 {
        let section = chunk.sections.get((section_y - min_section) as usize).filter(|_| section_y >= min_section);
        let light = light.iter().find(|l| l.y == section_y);
        let block_light = light.and_then(|l| l.block.as_ref());
        let sky_light = light.and_then(|l| l.sky.as_ref());
        if section.is_none() && block_light.is_none() && sky_light.is_none() {
            continue;
        }
        let mut tag = Vec::new();
        if let Some(s) = section {
            tag.push(("block_states".into(), container_to_nbt(&s.states, state_to_nbt)));
            tag.push(("biomes".into(), container_to_nbt(&s.biomes, |b| Nbt::String(b.name().into()))));
        }
        if let Some(layer) = block_light {
            tag.push(("BlockLight".into(), layer_to_nbt(layer)));
        }
        if let Some(layer) = sky_light {
            tag.push(("SkyLight".into(), layer_to_nbt(layer)));
        }
        tag.push(("Y".into(), Nbt::Byte(section_y as i8)));
        sections.push(Nbt::Compound(tag.into()));
    }
    let mut tag = vec![
        ("DataVersion".into(), Nbt::Int(WORLD_VERSION)),
        ("xPos".into(), Nbt::Int(chunk.pos.x)),
        ("yPos".into(), Nbt::Int(min_section)),
        ("zPos".into(), Nbt::Int(chunk.pos.z)),
        ("LastUpdate".into(), Nbt::Long(last_update)),
        ("InhabitedTime".into(), Nbt::Long(chunk.inhabited_time)),
        ("Status".into(), Nbt::String(format!("minecraft:{}", status.get_name()))),
    ];
    if let Some(blending_data) = &chunk.blending_data {
        tag.push(("blending_data".into(), blending_data.pack()));
    }
    if let Some(retrogen) = chunk.proto.as_ref().and_then(|proto| proto.below_zero_retrogen.as_ref()) {
        tag.push(("below_zero_retrogen".into(), retrogen.encode()));
    }
    if !chunk.upgrade_data.is_empty() {
        tag.push(("UpgradeData".into(), chunk.upgrade_data.write()));
    }
    tag.push(("sections".into(), Nbt::List(sections)));
    if chunk.light_correct {
        tag.push(("isLightOn".into(), Nbt::Byte(1)));
    }
    tag.push(("block_entities".into(), Nbt::List(block_entities)));
    if status != ChunkStatus::Full {
        let entities = chunk.proto.as_ref().map(|proto| proto.entities.clone()).unwrap_or_default();
        tag.push(("entities".into(), Nbt::List(entities)));
    }
    let (block_ticks, fluid_ticks) = match &chunk.proto {
        Some(proto) => (proto.block_ticks.pack(), proto.fluid_ticks.pack()),
        None => (block_ticks, fluid_ticks),
    };
    let post_processing = chunk.post_processing.iter().map(|offsets| Nbt::List(offsets.iter().map(|&offset| Nbt::Short(offset as i16)).collect())).collect();
    tag.extend([
        ("block_ticks".into(), saved_ticks_to_nbt(block_ticks, Block::name)),
        ("fluid_ticks".into(), saved_ticks_to_nbt(fluid_ticks, |f| registry::FLUID[f as usize])),
        ("PostProcessing".into(), Nbt::List(post_processing)),
        ("Heightmaps".into(), heightmaps_to_nbt(chunk, status)),
        ("structures".into(), pack_structure_data(chunk.pos, &chunk.structures)),
    ]);
    Nbt::Compound(tag.into())
}

fn pack_structure_data(pos: ChunkPos, structures: &ChunkStructures) -> Nbt {
    let starts = structures.starts.iter().map(|(structure, start)| (structure.key().to_owned(), start.create_tag(pos))).collect();
    let references = structures
        .references
        .iter()
        .filter(|(_, set)| !set.is_empty())
        .map(|(structure, set)| (structure.key().to_owned(), Nbt::LongArray(set.iter().collect())))
        .collect();
    Nbt::Compound(vec![("References".into(), Nbt::Compound(references)), ("starts".into(), Nbt::Compound(starts))].into())
}

fn unpack_structure_data(pos: ChunkPos, tag: Option<&Nbt>, seed: i64) -> ChunkStructures {
    let mut structures = ChunkStructures::default();
    let Some(tag) = tag else { return structures };
    if let Some(Nbt::Compound(starts)) = tag.get("starts") {
        for (key, start_tag) in starts {
            match StructureHolder::by_name(key) {
                None => error!("Unknown structure start: {key}"),
                Some(structure) => {
                    if let Some(Some(start)) = StructureStart::load_static_start(start_tag, seed) {
                        structures.starts.insert(structure, std::sync::Arc::new(start));
                    }
                }
            }
        }
    }
    if let Some(Nbt::Compound(references)) = tag.get("References") {
        for (key, entry) in references {
            let Some(structure) = StructureHolder::by_name(key) else {
                warn!("Found reference to unknown structure '{key}' in chunk {pos}, discarding");
                continue;
            };
            let Nbt::LongArray(values) = entry else { continue };
            let mut set = wafer_core::fastutil::LongOpenHashSet::with_expected(values.len(), 0.75);
            for &value in values {
                let ref_pos = ChunkPos::unpack(value);
                if ref_pos.chessboard_distance(pos) > 8 {
                    warn!("Found invalid structure reference [ {key} @ {ref_pos} ] for chunk {pos}.");
                } else {
                    set.add(value);
                }
            }
            structures.references.insert(structure, set);
        }
    }
    structures
}

fn read_section_container<T: super::palette::PaletteValue>(
    tag: Option<&Nbt>,
    chunk_pos: (i32, i32),
    section_y: i32,
    strategy: super::palette::Strategy,
    default: T,
    entry: impl Fn(&Nbt) -> Result<T, String>,
    frames: [&'static str; 4],
) -> Result<Option<PalettedContainer<T>>, ReadError> {
    let Some(tag @ Nbt::Compound(_)) = tag else { return Ok(None) };
    match container_from_nbt(tag, strategy, default, entry) {
        Ok((container, recoverable)) => {
            if let Some(message) = recoverable {
                error!("Recoverable errors when loading section [{}, {section_y}, {}]: {message}", chunk_pos.0, chunk_pos.1);
            }
            Ok(Some(container))
        }
        Err(message) => {
            error!("Recoverable errors when loading section [{}, {section_y}, {}]: {message}", chunk_pos.0, chunk_pos.1);
            Err(parse_exception(format!("{CHUNK_READ_EXCEPTION}: {message}"), &frames))
        }
    }
}

const EMPTY_PALETTE_FRAMES: [&str; 5] = [
    "net.minecraft.world.level.chunk.SingleValuePalette.valueFor(SingleValuePalette.java:46)",
    "net.minecraft.world.level.chunk.PalettedContainer.count(PalettedContainer.java:319)",
    "net.minecraft.world.level.chunk.LevelChunkSection.recalcBlockCounts(LevelChunkSection.java:150)",
    "net.minecraft.world.level.chunk.LevelChunkSection.<init>(LevelChunkSection.java:36)",
    "net.minecraft.world.level.chunk.storage.SerializableChunkData.parse(SerializableChunkData.java:191)",
];

const STATES_FRAMES: [&str; 4] = [
    GET_OR_THROW,
    "net.minecraft.world.level.chunk.storage.SerializableChunkData.lambda$parse$3(SerializableChunkData.java:184)",
    OPTIONAL_MAP,
    "net.minecraft.world.level.chunk.storage.SerializableChunkData.parse(SerializableChunkData.java:184)",
];
const BIOMES_FRAMES: [&str; 4] = [
    GET_OR_THROW,
    "net.minecraft.world.level.chunk.storage.SerializableChunkData.lambda$parse$5(SerializableChunkData.java:188)",
    OPTIONAL_MAP,
    "net.minecraft.world.level.chunk.storage.SerializableChunkData.parse(SerializableChunkData.java:188)",
];

pub fn read(nbt: &Nbt, dimension: &DimensionType, seed: i64) -> Result<ReadChunk, ReadError> {
    let Some(status_name) = nbt.get("Status").and_then(Nbt::as_str) else { return Err(ReadError::MissingLevelData) };
    let status = match ChunkStatus::by_name(status_name) {
        Some(status) => status,
        None => {
            let message = match crate::resources::identifier::read(status_name) {
                Ok(id) => format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:chunk_status]: {id}"),
                Err(error) => error,
            };
            error!("Failed to read field (Status={}): {message}", crate::nbt::to_string(&Nbt::String(status_name.to_string())));
            ChunkStatus::Empty
        }
    };
    let x = nbt.get("xPos").and_then(Nbt::as_i64).unwrap_or(0) as i32;
    let z = nbt.get("zPos").and_then(Nbt::as_i64).unwrap_or(0) as i32;
    let plains = Biome::by_name("minecraft:plains").unwrap_or(Biome(0));
    let mut sections: Vec<LevelChunkSection> = (0..dimension.height / 16).map(|_| LevelChunkSection::new(plains)).collect();
    let min_section = dimension.min_y >> 4;
    let max_section = min_section + dimension.height as i32 / 16 - 1;
    let mut light = Vec::new();
    for section in nbt.get("sections").map(Nbt::as_list).unwrap_or(&[]) {
        if !matches!(section, Nbt::Compound(_)) {
            continue;
        }
        let y = section.get("Y").and_then(Nbt::as_i64).unwrap_or(0) as i8 as i32;
        let loaded = if (min_section..=max_section).contains(&y) {
            let states = read_section_container(section.get("block_states"), (x, z), y, block_strategy(), wafer_data::blocks::AIR.default_state(), decode_block_state, STATES_FRAMES)?;
            let biomes = read_section_container(section.get("biomes"), (x, z), y, biome_strategy(), plains, decode_biome, BIOMES_FRAMES)?;
            if states.is_some() && section.get("block_states").and_then(|states| states.get("palette")).is_some_and(|palette| palette.as_list().is_empty()) {
                return Err(parse_exception("java.lang.IllegalStateException: Missing Palette entry for id 0.".to_string(), &EMPTY_PALETTE_FRAMES));
            }
            Some((states, biomes))
        } else {
            None
        };
        let (block, sky) = (layer_from_nbt(section.get("BlockLight"), PARSE_BLOCK_LIGHT)?, layer_from_nbt(section.get("SkyLight"), PARSE_SKY_LIGHT)?);
        if block.is_some() || sky.is_some() {
            light.push(SectionLight { y, block, sky });
        }
        let Some((states, biomes)) = loaded else { continue };
        let target = &mut sections[(y - min_section) as usize];
        let states = states.unwrap_or_else(|| target.states.clone());
        let biomes = biomes.unwrap_or_else(|| target.biomes.clone());
        *target = LevelChunkSection::from_containers(states, biomes);
    }
    let sections_count = sections.len();
    let mut chunk = LevelChunk::with_sections(ChunkPos::new(x, z), dimension, sections);
    if let Some(upgrade_data @ Nbt::Compound(_)) = nbt.get("UpgradeData") {
        chunk.upgrade_data = UpgradeData::new(upgrade_data, sections_count);
    }
    chunk.inhabited_time = nbt.get("InhabitedTime").and_then(Nbt::as_i64).unwrap_or(0);
    chunk.light_correct = nbt.get("isLightOn").and_then(Nbt::as_i64).is_some_and(|b| b != 0);
    let block_entities = nbt.get("block_entities").map(Nbt::as_list).unwrap_or(&[]);
    if status == ChunkStatus::Full {
        chunk.post_load_block_entities = block_entities.to_vec();
    } else {
        for tag in block_entities {
            chunk.block_entities.pending.insert(crate::world::level::block::entity::BlockEntity::get_pos_from_tag(chunk.pos, tag), tag.clone());
        }
    }
    chunk.structures = unpack_structure_data(chunk.pos, nbt.get("structures"), seed);
    chunk.blending_data = nbt.get("blending_data").and_then(crate::world::level::levelgen::blending::blending_data::BlendingData::unpack).map(std::sync::Arc::new);
    let heightmap_data = |kind: HeightmapType| match nbt.get("Heightmaps").and_then(|h| h.get(kind.serialization_key())) {
        Some(Nbt::LongArray(data)) => Some(data),
        _ => None,
    };
    let mut missing = false;
    for (i, kind) in HeightmapType::LIVE.into_iter().enumerate() {
        missing |= !heightmap_data(kind).is_some_and(|data| chunk.heightmaps[i].set_raw(data));
    }
    if missing {
        chunk.prime_heightmaps();
    }
    for (section, offsets) in chunk.post_processing.iter_mut().zip(nbt.get("PostProcessing").map(Nbt::as_list).unwrap_or(&[])) {
        section.extend(offsets.as_list().iter().filter_map(Nbt::as_i64).map(|offset| offset as u16));
    }
    let block_ticks = filter_tick_list_for_chunk(saved_ticks_from_nbt(nbt.get("block_ticks"), |name| name?.as_str().and_then(Block::by_name)), chunk.pos);
    let fluid_ticks = filter_tick_list_for_chunk(saved_ticks_from_nbt(nbt.get("fluid_ticks"), |name| name?.as_str().and_then(fluid_by_name)), chunk.pos);
    if status == ChunkStatus::Full {
        chunk.legacy_entities = nbt.get("entities").map(Nbt::as_list).unwrap_or(&[]).to_vec();
        return Ok(ReadChunk { chunk, status, block_ticks: LevelChunkTicks::with_pending(block_ticks), fluid_ticks: LevelChunkTicks::with_pending(fluid_ticks), light });
    }
    let mut proto = ProtoChunkData::default();
    for (i, kind) in HeightmapType::WORLDGEN.into_iter().enumerate() {
        if let Some(data) = heightmap_data(kind) {
            let mut heightmap = Heightmap::new(kind, dimension.min_y, dimension.height as i32);
            if heightmap.set_raw(data) {
                proto.heightmaps[i] = Some(heightmap);
            }
        }
    }
    proto.block_ticks = ProtoChunkTicks::load(block_ticks);
    proto.fluid_ticks = ProtoChunkTicks::load(fluid_ticks);
    proto.entities = nbt.get("entities").map(Nbt::as_list).unwrap_or(&[]).to_vec();
    proto.below_zero_retrogen = nbt.get("below_zero_retrogen").and_then(crate::world::level::levelgen::below_zero_retrogen::BelowZeroRetrogen::decode);
    chunk.proto = Some(Box::new(proto));
    Ok(ReadChunk { chunk, status, block_ticks: LevelChunkTicks::default(), fluid_ticks: LevelChunkTicks::default(), light })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::level::chunk::FlatLevelSource;
    use wafer_core::BlockPos;
    use wafer_data::blocks;
    use wafer_data::properties::HORIZONTAL_FACING;
    use wafer_core::Direction;

    fn overworld() -> &'static DimensionType {
        &wafer_data::DIMENSION_TYPES[0]
    }

    fn saved_flat_chunk() -> Nbt {
        let chunk = FlatLevelSource::default().generate(ChunkPos::new(3, -7), overworld());
        write(&chunk, ChunkStatus::Full, Vec::new(), 0, &[], &[], &[])
    }

    fn set_field(nbt: &mut Nbt, name: &str, value: Option<Nbt>) {
        let Nbt::Compound(fields) = nbt else { panic!("compound") };
        fields.retain(|(key, _)| key != name);
        fields.extend(value.map(|value| (name.to_string(), value)));
    }

    fn first_section(nbt: &mut Nbt) -> &mut Nbt {
        let Nbt::Compound(fields) = nbt else { panic!("compound") };
        let Some((_, Nbt::List(sections))) = fields.iter_mut().find(|(key, _)| key == "sections") else { panic!("sections") };
        &mut sections[0]
    }

    #[test]
    fn parse_failures_carry_the_serializable_chunk_data_frames() {
        let mut nbt = saved_flat_chunk();
        set_field(first_section(&mut nbt), "SkyLight", Some(Nbt::ByteArray(vec![0; 3])));
        let Err(ReadError::Exception(failure)) = read(&nbt, overworld(), 0) else { panic!("expected an exception") };
        assert_eq!(failure.description, "java.lang.IllegalArgumentException: DataLayer should be 2048 bytes not: 3");
        assert_eq!(failure.stack_trace[..3], [DATA_LAYER_INIT, OPTIONAL_MAP, PARSE_SKY_LIGHT]);
        assert_eq!(failure.stack_trace.last().copied(), Some("java.base/java.util.concurrent.ForkJoinWorkerThread.run(ForkJoinWorkerThread.java:187)"));

        let mut nbt = saved_flat_chunk();
        let Nbt::Compound(states) = first_section(&mut nbt).get("block_states").cloned().unwrap() else { panic!("compound") };
        let mut states = Nbt::Compound(states);
        set_field(&mut states, "palette", Some(Nbt::String("oops".to_string())));
        set_field(first_section(&mut nbt), "block_states", Some(states));
        let Err(ReadError::Exception(failure)) = read(&nbt, overworld(), 0) else { panic!("expected an exception") };
        assert_eq!(failure.description, "net.minecraft.world.level.chunk.storage.SerializableChunkData$ChunkReadException: Not a list: \"oops\"");
        assert_eq!(failure.stack_trace[0], GET_OR_THROW);
    }

    #[test]
    fn parse_is_lenient_like_serializable_chunk_data() {
        let mut nbt = saved_flat_chunk();
        set_field(&mut nbt, "Status", None);
        assert!(matches!(read(&nbt, overworld(), 0), Err(ReadError::MissingLevelData)));
        let mut nbt = saved_flat_chunk();
        set_field(&mut nbt, "DataVersion", Some(Nbt::Int(99999)));
        set_field(&mut nbt, "xPos", None);
        set_field(first_section(&mut nbt), "Y", None);
        assert_eq!(read(&nbt, overworld(), 0).unwrap().chunk.pos, ChunkPos::new(0, -7));
    }

    #[test]
    fn chunk_round_trip() {
        let mut chunk = FlatLevelSource::default().generate(ChunkPos::new(3, -7), overworld());
        let stairs = blocks::OAK_STAIRS.default_state().set(HORIZONTAL_FACING, Direction::East);
        chunk.set_block_state(BlockPos::new(49, -60, -110), stairs);
        chunk.set_block_state(BlockPos::new(53, 100, -103), blocks::GLOWSTONE.default_state());
        let read = read(&write(&chunk, ChunkStatus::Full, Vec::new(), 0, &[], &[], &[]), overworld(), 0).unwrap().chunk;
        assert_eq!(read.pos, ChunkPos::new(3, -7));
        assert_eq!(read.block_state(BlockPos::new(49, -60, -110)), stairs);
        assert_eq!(read.block_state(BlockPos::new(53, 100, -103)), blocks::GLOWSTONE.default_state());
        assert_eq!(read.block_state(BlockPos::new(48, -64, -112)), blocks::BEDROCK.default_state());
    }

    #[test]
    fn upgrade_data_round_trips_through_the_chunk_tag() {
        let mut chunk = FlatLevelSource::default().generate(ChunkPos::new(3, -7), overworld());
        let tag = Nbt::Compound(vec![("Sides".to_string(), Nbt::Byte(5)), ("Indices".to_string(), Nbt::Compound(vec![("2".to_string(), Nbt::IntArray(vec![17]))].into()))].into());
        chunk.upgrade_data = UpgradeData::new(&tag, 24);
        let saved = write(&chunk, ChunkStatus::Full, Vec::new(), 0, &[], &[], &[]);
        assert_eq!(saved.get("UpgradeData").and_then(|data| data.get("Sides")), Some(&Nbt::Byte(5)));
        let read = read(&saved, overworld(), 0).unwrap().chunk;
        assert!(!read.upgrade_data.is_empty());
        assert_eq!(read.upgrade_data.write(), chunk.upgrade_data.write());
        let plain = write(&FlatLevelSource::default().generate(ChunkPos::new(3, -7), overworld()), ChunkStatus::Full, Vec::new(), 0, &[], &[], &[]);
        assert!(plain.get("UpgradeData").is_none());
    }

    #[test]
    fn ticks_of_other_chunks_are_dropped_on_read() {
        let chunk = FlatLevelSource::default().generate(ChunkPos::new(3, -7), overworld());
        let tick = |x: i32, z: i32| SavedTick { ty: blocks::STONE, pos: BlockPos::new(x, 70, z), delay: 1, priority: TickPriority::NORMAL };
        let saved = write(&chunk, ChunkStatus::Full, Vec::new(), 0, &[tick(50, -100), tick(64, -100), tick(50, -113)], &[], &[]);
        let read = read(&saved, overworld(), 0).unwrap();
        assert_eq!(read.block_ticks.count(), 1);
    }

    #[test]
    fn ticks_without_a_priority_are_dropped_on_read() {
        let ticks = Nbt::List(vec![Nbt::Compound(vec![("i".to_string(), Nbt::String("minecraft:stone".to_string())), ("x".to_string(), Nbt::Int(1)), ("y".to_string(), Nbt::Int(1)), ("z".to_string(), Nbt::Int(1)), ("t".to_string(), Nbt::Int(1))].into())]);
        assert!(saved_ticks_from_nbt(Some(&ticks), |name| name?.as_str().and_then(Block::by_name)).is_empty());
    }

    #[test]
    fn mixed_palette_wraps_ids() {
        let stairs = blocks::OAK_STAIRS.default_state().set(HORIZONTAL_FACING, Direction::East);
        let list = mixed_list(vec![state_to_nbt(blocks::DIRT.default_state()), state_to_nbt(stairs)]);
        let mut w = wafer_protocol::Writer::new();
        list.write(&mut w);
        assert_eq!(&w.as_slice()[..2], &[9, 10]);
        assert_eq!(Nbt::read(&mut wafer_protocol::Reader::new(w.as_slice())).unwrap(), list);
        assert_eq!(list.as_list().iter().map(state_from_nbt).collect::<Vec<_>>(), [Some(blocks::DIRT.default_state()), Some(stairs)]);
    }
}
