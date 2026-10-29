use wafer_core::random::RandomSource;
use wafer_core::{BlockPos, Direction};
use wafer_data::{Block, BlockState, blocks};
use serde_json::Value;

use crate::util::valueproviders::IntProvider;
use crate::world::level::levelgen::parse_block_state;
use crate::world::level::world_gen_level::WorldGenLevel;

const CANNOT_REPLACE: [Block; 7] =
    [blocks::BEDROCK, blocks::NETHER_BRICKS, blocks::NETHER_BRICK_FENCE, blocks::NETHER_BRICK_STAIRS, blocks::NETHER_WART, blocks::CHEST, blocks::SPAWNER];
const RIM_SPAWN_CHANCE: f64 = 0.9;

pub struct DeltaFeature {
    contents: BlockState,
    rim: BlockState,
    size: IntProvider,
    rim_size: IntProvider,
}

impl DeltaFeature {
    pub fn parse(json: &Value) -> Result<DeltaFeature, String> {
        let field = |key: &str| json.get(key).ok_or_else(|| crate::util::codec::no_key(key, json));
        Ok(DeltaFeature {
            contents: parse_block_state(field("contents")?)?,
            rim: parse_block_state(field("rim")?)?,
            size: IntProvider::parse(field("size")?)?,
            rim_size: IntProvider::parse(field("rim_size")?)?,
        })
    }

    pub fn place<R: RandomSource>(&self, level: &mut impl WorldGenLevel, random: &mut R, origin: BlockPos) -> bool {
        let mut any_placed = false;
        let spawn_rim = random.next_double() < RIM_SPAWN_CHANCE;
        let rim_x = if spawn_rim { self.rim_size.sample(random) } else { 0 };
        let rim_z = if spawn_rim { self.rim_size.sample(random) } else { 0 };
        let has_rim = spawn_rim && rim_x != 0 && rim_z != 0;
        let radius_x = self.size.sample(random);
        let radius_z = self.size.sample(random);
        let radius_limit = radius_x.max(radius_z);
        for pos in BlockPos::within_box_by_manhattan_distance(origin, radius_x, 0, radius_z) {
            if pos.dist_manhattan(origin) > radius_limit {
                break;
            }
            if self.is_clear(level, pos) {
                if has_rim {
                    any_placed = true;
                    level.set_block_and_update(pos, self.rim);
                }
                let pos_offset = pos.offset(rim_x, 0, rim_z);
                if self.is_clear(level, pos_offset) {
                    any_placed = true;
                    level.set_block_and_update(pos_offset, self.contents);
                }
            }
        }
        any_placed
    }

    fn is_clear(&self, level: &mut impl WorldGenLevel, pos: BlockPos) -> bool {
        let state = level.get_block_state(pos);
        if state.is_block(self.contents.block()) || CANNOT_REPLACE.contains(&state.block()) {
            return false;
        }
        for d in Direction::VALUES {
            let is_air = level.get_block_state(pos.relative(d)).is_air();
            if is_air && d != Direction::Up || !is_air && d == Direction::Up {
                return false;
            }
        }
        true
    }
}

pub fn fields() -> Vec<crate::world::item::component::codec::Field> {
    use crate::util::valueproviders::int_provider_codec_in;
    use crate::world::item::component::types::req;
    use crate::world::level::block::state::block_state;
    vec![
        req("contents", block_state::codec()),
        req("rim", block_state::codec()),
        req("size", int_provider_codec_in::<0, 16>()),
        req("rim_size", int_provider_codec_in::<0, 16>()),
    ]
}
