use wafer_core::random::RandomSource;
use wafer_core::{BlockPos, Direction};
use serde_json::Value;

use super::stateproviders::block_state_provider::BlockStateProviderHolder;
use crate::world::level::levelgen::blockpredicates::block_predicate::BlockPredicate;
use crate::world::level::levelgen::placement::placed_feature::PlacedFeatureHolder;
use crate::world::level::world_gen_level::WorldGenLevel;

pub struct SingleBlockPillarFeature {
    block: BlockStateProviderHolder,
    can_replace: BlockPredicate,
    direction: Direction,
    chance_to_continue: f32,
    cap_feature: Option<PlacedFeatureHolder>,
}

impl SingleBlockPillarFeature {
    pub fn parse(json: &Value) -> Result<SingleBlockPillarFeature, String> {
        Ok(SingleBlockPillarFeature {
            block: BlockStateProviderHolder::parse(json.get("block").ok_or("missing block")?)?,
            can_replace: json.get("can_replace").map(BlockPredicate::parse).transpose()?.unwrap_or(BlockPredicate::True),
            direction: json.get("direction").and_then(Value::as_str).and_then(Direction::by_name).ok_or("bad direction")?,
            chance_to_continue: json.get("chance_to_continue").and_then(Value::as_f64).map_or(1.0, |v| v as f32),
            cap_feature: json.get("cap_feature").map(PlacedFeatureHolder::parse).transpose()?,
        })
    }

    pub fn place<R: RandomSource>(&self, level: &mut impl WorldGenLevel, random: &mut R, origin: BlockPos) -> bool {
        let mut pos = origin;
        while self.can_replace.test(level, pos) && random.next_float() < self.chance_to_continue && !level.is_outside_build_height(pos.y) {
            let state = self.block.value().get_state(level, random, pos);
            level.set_block(pos, state, 2);
            pos = pos.relative(self.direction);
        }
        pos = pos.relative(self.direction.opposite());
        if let Some(cap_feature) = &self.cap_feature {
            cap_feature.value().place(level, random, pos);
        }
        true
    }
}

fn verify_vertical(direction: wafer_protocol::nbt::Nbt) -> Result<wafer_protocol::nbt::Nbt, String> {
    match direction.as_str() {
        Some("up" | "down") => Ok(direction),
        _ => Err("Expected a vertical direction".into()),
    }
}

pub fn fields() -> Vec<crate::world::item::component::codec::Field> {
    use super::stateproviders::block_state_provider;
    use crate::util::range_codecs::float_range;
    use crate::world::item::component::codec::Codec;
    use crate::world::item::component::types::{self, compound, def, f, opt, req, s};
    use crate::world::level::levelgen::blockpredicates::block_predicate;
    use crate::world::level::levelgen::placement::placed_feature;
    let direction = types::enumeration(&["down", "up", "north", "south", "west", "east"]);
    vec![
        req("block", block_state_provider::holder_codec()),
        def("can_replace", block_predicate::codec(), compound(&[("type", s("minecraft:true"))])),
        req("direction", (Codec::Normalize(Box::new(direction.0), verify_vertical), direction.1)),
        def("chance_to_continue", float_range(0.0, 1.0), f(1.0)),
        opt("cap_feature", placed_feature::holder_codec()),
    ]
}
