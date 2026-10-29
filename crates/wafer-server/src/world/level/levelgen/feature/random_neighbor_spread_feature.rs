use wafer_core::random::RandomSource;
use wafer_core::{BlockPos, Direction};
use serde_json::Value;

use super::stateproviders::block_state_provider::BlockStateProviderHolder;
use crate::util::valueproviders::IntProvider;
use crate::world::level::levelgen::blockpredicates::block_predicate::{BlockPredicate, BlockSet};
use crate::world::level::world_gen_level::WorldGenLevel;

pub struct RandomNeighborSpreadFeature {
    block: BlockStateProviderHolder,
    accepted_neighbors: BlockSet,
    can_replace: BlockPredicate,
    attempts: IntProvider,
    xz_offset: IntProvider,
    y_offset: IntProvider,
}

impl RandomNeighborSpreadFeature {
    pub fn parse(json: &Value) -> Result<RandomNeighborSpreadFeature, String> {
        let field = |key: &str| json.get(key).ok_or_else(|| crate::util::codec::no_key(key, json));
        Ok(RandomNeighborSpreadFeature {
            block: BlockStateProviderHolder::parse(field("block")?)?,
            accepted_neighbors: BlockSet::parse(field("accepted_neighbors")?)?,
            can_replace: BlockPredicate::parse(field("can_replace")?)?,
            attempts: IntProvider::parse(field("attempts")?)?,
            xz_offset: IntProvider::parse(field("xz_offset")?)?,
            y_offset: IntProvider::parse(field("y_offset")?)?,
        })
    }

    pub fn place<R: RandomSource>(&self, level: &mut impl WorldGenLevel, random: &mut R, origin: BlockPos) -> bool {
        let state = self.block.value().get_state(level, random, origin);
        level.set_block(origin, state, 2);
        let attempts = self.attempts.sample(random);
        for _ in 0..attempts {
            let dx = self.xz_offset.sample(random);
            let dy = self.y_offset.sample(random);
            let dz = self.xz_offset.sample(random);
            let place_pos = origin.offset(dx, dy, dz);
            if self.can_replace.test(level, place_pos) {
                let mut neighbours = 0;
                for direction in Direction::VALUES {
                    if self.accepted_neighbors.contains(level.get_block_state(place_pos.relative(direction)).block()) {
                        neighbours += 1;
                    }
                    if neighbours > 1 {
                        break;
                    }
                }
                if neighbours == 1 {
                    let state = self.block.value().get_state(level, random, place_pos);
                    level.set_block(place_pos, state, 2);
                }
            }
        }
        true
    }
}

pub fn fields() -> Vec<crate::world::item::component::codec::Field> {
    use super::stateproviders::block_state_provider;
    use crate::util::valueproviders::int_provider_codec_in;
    use crate::world::item::component::types::{self, req};
    use crate::world::level::levelgen::blockpredicates::block_predicate;
    vec![
        req("block", block_state_provider::holder_codec()),
        req("accepted_neighbors", types::holder_set("minecraft:block")),
        req("can_replace", block_predicate::codec()),
        req("attempts", int_provider_codec_in::<1, 3000>()),
        req("xz_offset", int_provider_codec_in::<-16, 16>()),
        req("y_offset", int_provider_codec_in::<-16, 16>()),
    ]
}
