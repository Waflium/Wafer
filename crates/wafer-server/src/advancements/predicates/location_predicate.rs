use wafer_core::BlockPos;
use wafer_protocol::nbt::Nbt;

use super::block_predicate::BlockPredicate;
use super::fluid_predicate::{self, FluidPredicate};
use super::light_predicate::{self, LightPredicate};
use super::min_max_bounds::{Doubles, doubles_codec};
use crate::core::holder_set::HolderSet;
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult};
use crate::world::item::component::custom;
use crate::world::item::component::types::{self, Pair, def, opt, record};
use crate::world::level::block::campfire_block;

fn position_codec() -> Pair {
    record(vec![def("x", doubles_codec(), types::empty_map()), def("y", doubles_codec(), types::empty_map()), def("z", doubles_codec(), types::empty_map())])
}

pub fn codec() -> Pair {
    record(vec![
        opt("position", position_codec()),
        opt("biomes", types::holder_set("minecraft:worldgen/biome")),
        opt("structures", types::holder_set("minecraft:worldgen/structure")),
        opt("dimension", types::identifier()),
        opt("smokey", types::bool()),
        opt("light", light_predicate::codec()),
        opt("block", custom::block_predicate()),
        opt("fluid", fluid_predicate::codec()),
        opt("can_see_sky", types::bool()),
    ])
}

#[derive(Debug)]
struct PositionPredicate {
    x: Doubles,
    y: Doubles,
    z: Doubles,
}

impl PositionPredicate {
    fn decode(tag: &Nbt) -> DataResult<PositionPredicate> {
        codec::map(tag)?;
        let mut record = codec::Record::default();
        let x = record.field(codec::decode_or(tag, "x", Doubles::ANY, Doubles::decode));
        let y = record.field(codec::decode_or(tag, "y", Doubles::ANY, Doubles::decode));
        let z = record.field(codec::decode_or(tag, "z", Doubles::ANY, Doubles::decode));
        record.finish()?;
        Ok(PositionPredicate { x: x.unwrap(), y: y.unwrap(), z: z.unwrap() })
    }

    fn matches(&self, x: f64, y: f64, z: f64) -> bool {
        self.x.matches(x) && self.y.matches(y) && self.z.matches(z)
    }
}

#[derive(Debug)]
pub struct LocationPredicate {
    position: Option<PositionPredicate>,
    biomes: Option<HolderSet>,
    structures: Option<HolderSet>,
    dimension: Option<String>,
    smokey: Option<bool>,
    light: Option<LightPredicate>,
    block: Option<BlockPredicate>,
    fluid: Option<FluidPredicate>,
    can_see_sky: Option<bool>,
}

fn decode_structures(tag: &Nbt) -> DataResult<HolderSet> {
    const REGISTRY: &str = "minecraft:worldgen/structure";
    if let Nbt::String(name) = tag
        && name.starts_with('#')
    {
        return HolderSet::decode(REGISTRY, tag);
    }
    codec::holder_set(tag, REGISTRY, |element| {
        let id = codec::identifier(element)?;
        if crate::core::data_registries::json_entries(REGISTRY).iter().any(|(key, _)| *key == id) {
            Ok(id)
        } else if codec::is_nbt_ops() {
            Err(format!("Failed to get element {id} from registry {REGISTRY}"))
        } else {
            Err(format!("Missing element ResourceKey[{REGISTRY} / {id}]"))
        }
    })
    .map_err(|error| error.message)?;
    HolderSet::decode(REGISTRY, tag)
}

impl LocationPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<LocationPredicate> {
        codec::map(tag)?;
        let mut record = codec::Record::default();
        let position = record.field(codec::decode_optional(tag, "position", PositionPredicate::decode));
        let biomes = record.field(codec::decode_optional(tag, "biomes", |t| HolderSet::decode("minecraft:worldgen/biome", t)));
        let structures = record.field(codec::decode_optional(tag, "structures", decode_structures));
        let dimension = record.field(codec::decode_optional(tag, "dimension", codec::identifier));
        let smokey = record.field(codec::decode_optional(tag, "smokey", codec::bool_value));
        let light = record.field(codec::decode_optional(tag, "light", LightPredicate::decode));
        let block = record.field(codec::decode_optional(tag, "block", BlockPredicate::decode));
        let fluid = record.field(codec::decode_optional(tag, "fluid", FluidPredicate::decode));
        let can_see_sky = record.field(codec::decode_optional(tag, "can_see_sky", codec::bool_value));
        record.finish()?;
        Ok(LocationPredicate {
            position: position.unwrap(),
            biomes: biomes.unwrap(),
            structures: structures.unwrap(),
            dimension: dimension.unwrap(),
            smokey: smokey.unwrap(),
            light: light.unwrap(),
            block: block.unwrap(),
            fluid: fluid.unwrap(),
            can_see_sky: can_see_sky.unwrap(),
        })
    }

    pub fn matches(&self, level: &mut ServerLevel, x: f64, y: f64, z: f64) -> bool {
        if self.position.as_ref().is_some_and(|position| !position.matches(x, y, z)) {
            return false;
        }
        if self.dimension.as_ref().is_some_and(|dimension| dimension != crate::server::OVERWORLD) {
            return false;
        }
        let pos = BlockPos::containing(x, y, z);
        let loaded = level.is_loaded(pos);
        if self.biomes.as_ref().is_some_and(|biomes| !(loaded && biomes.contains(level.get_biome(pos).0 as i32))) {
            return false;
        }
        if self.structures.as_ref().is_some_and(|structures| !(loaded && level.get_structure_with_piece_at(pos, &|structure| structures.contains(structure.0 as i32)).is_some())) {
            return false;
        }
        if self.smokey.is_some_and(|smokey| !(loaded && smokey == campfire_block::is_smokey_pos(level, pos))) {
            return false;
        }
        if self.light.as_ref().is_some_and(|light| !light.matches(level, pos)) {
            return false;
        }
        if self.block.as_ref().is_some_and(|block| !block.matches(level, pos)) {
            return false;
        }
        if self.fluid.as_ref().is_some_and(|fluid| !fluid.matches(level, pos)) {
            return false;
        }
        self.can_see_sky.is_none_or(|can_see_sky| can_see_sky == level.can_see_sky(pos))
    }
}
