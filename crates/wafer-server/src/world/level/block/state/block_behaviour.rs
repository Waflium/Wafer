use wafer_core::{BlockPos, Direction};
use wafer_data::{Block, BlockState, blocks};

use crate::world::entity::entity_type::EntityType;
use crate::world::entity::entity_types;
use crate::world::level::BlockGetter;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RenderShape {
    Invisible,
    Model,
}

const INVISIBLE_RENDER_SHAPE: &[&str] = &[
    "AirBlock",
    "BarrierBlock",
    "BubbleColumnBlock",
    "EndGatewayBlock",
    "EndPortalBlock",
    "LightBlock",
    "LiquidBlock",
    "MovingPistonBlock",
    "StructureVoidBlock",
];

pub fn get_render_shape(state: BlockState) -> RenderShape {
    let block = state.block();
    if INVISIBLE_RENDER_SHAPE.iter().any(|class| block.is_instance_of(class)) { RenderShape::Invisible } else { RenderShape::Model }
}

enum IsValidSpawn {
    Default,
    Never,
    Always,
    OcelotOrParrot,
    PolarBear,
    FireImmune,
}

const NEVER: &[Block] = &[
    blocks::BEDROCK,
    blocks::GLASS,
    blocks::WHITE_STAINED_GLASS,
    blocks::ORANGE_STAINED_GLASS,
    blocks::MAGENTA_STAINED_GLASS,
    blocks::LIGHT_BLUE_STAINED_GLASS,
    blocks::YELLOW_STAINED_GLASS,
    blocks::LIME_STAINED_GLASS,
    blocks::PINK_STAINED_GLASS,
    blocks::GRAY_STAINED_GLASS,
    blocks::LIGHT_GRAY_STAINED_GLASS,
    blocks::CYAN_STAINED_GLASS,
    blocks::PURPLE_STAINED_GLASS,
    blocks::BLUE_STAINED_GLASS,
    blocks::BROWN_STAINED_GLASS,
    blocks::GREEN_STAINED_GLASS,
    blocks::RED_STAINED_GLASS,
    blocks::BLACK_STAINED_GLASS,
    blocks::TINTED_GLASS,
    blocks::MOVING_PISTON,
    blocks::REPEATER,
    blocks::OAK_TRAPDOOR,
    blocks::SPRUCE_TRAPDOOR,
    blocks::BIRCH_TRAPDOOR,
    blocks::JUNGLE_TRAPDOOR,
    blocks::ACACIA_TRAPDOOR,
    blocks::CHERRY_TRAPDOOR,
    blocks::DARK_OAK_TRAPDOOR,
    blocks::PALE_OAK_TRAPDOOR,
    blocks::MANGROVE_TRAPDOOR,
    blocks::POPLAR_TRAPDOOR,
    blocks::BAMBOO_TRAPDOOR,
    blocks::CRIMSON_TRAPDOOR,
    blocks::WARPED_TRAPDOOR,
    blocks::IRON_TRAPDOOR,
    blocks::COPPER_TRAPDOOR,
    blocks::EXPOSED_COPPER_TRAPDOOR,
    blocks::WEATHERED_COPPER_TRAPDOOR,
    blocks::OXIDIZED_COPPER_TRAPDOOR,
    blocks::WAXED_COPPER_TRAPDOOR,
    blocks::WAXED_EXPOSED_COPPER_TRAPDOOR,
    blocks::WAXED_WEATHERED_COPPER_TRAPDOOR,
    blocks::WAXED_OXIDIZED_COPPER_TRAPDOOR,
    blocks::COPPER_GRATE,
    blocks::EXPOSED_COPPER_GRATE,
    blocks::WEATHERED_COPPER_GRATE,
    blocks::OXIDIZED_COPPER_GRATE,
    blocks::WAXED_COPPER_GRATE,
    blocks::WAXED_EXPOSED_COPPER_GRATE,
    blocks::WAXED_WEATHERED_COPPER_GRATE,
    blocks::WAXED_OXIDIZED_COPPER_GRATE,
    blocks::BARRIER,
    blocks::CHORUS_FLOWER,
    blocks::SCAFFOLDING,
];

const ALWAYS: &[Block] = &[blocks::SOUL_SAND, blocks::CARVED_PUMPKIN, blocks::JACK_O_LANTERN, blocks::REDSTONE_LAMP, blocks::MUD];

const OCELOT_OR_PARROT: &[Block] = &[
    blocks::OAK_LEAVES,
    blocks::SPRUCE_LEAVES,
    blocks::BIRCH_LEAVES,
    blocks::JUNGLE_LEAVES,
    blocks::ACACIA_LEAVES,
    blocks::CHERRY_LEAVES,
    blocks::DARK_OAK_LEAVES,
    blocks::PALE_OAK_LEAVES,
    blocks::MANGROVE_LEAVES,
    blocks::AZALEA_LEAVES,
    blocks::FLOWERING_AZALEA_LEAVES,
    blocks::ORANGE_POPLAR_LEAVES,
    blocks::RED_POPLAR_LEAVES,
    blocks::YELLOW_POPLAR_LEAVES,
];

fn is_valid_spawn_predicate(block: Block) -> IsValidSpawn {
    if NEVER.contains(&block) {
        IsValidSpawn::Never
    } else if ALWAYS.contains(&block) {
        IsValidSpawn::Always
    } else if OCELOT_OR_PARROT.contains(&block) {
        IsValidSpawn::OcelotOrParrot
    } else if block == blocks::ICE || block == blocks::FROSTED_ICE {
        IsValidSpawn::PolarBear
    } else if block == blocks::MAGMA_BLOCK {
        IsValidSpawn::FireImmune
    } else {
        IsValidSpawn::Default
    }
}

pub fn is_valid_spawn(level: &mut (impl BlockGetter + ?Sized), state: BlockState, pos: BlockPos, entity_type: &EntityType) -> bool {
    match is_valid_spawn_predicate(state.block()) {
        IsValidSpawn::Default => level.is_face_sturdy_full_at(pos, state, Direction::Up) && state.light_emission() < 14,
        IsValidSpawn::Never => false,
        IsValidSpawn::Always => true,
        IsValidSpawn::OcelotOrParrot => *entity_type == entity_types::OCELOT || *entity_type == entity_types::PARROT,
        IsValidSpawn::PolarBear => *entity_type == entity_types::POLAR_BEAR,
        IsValidSpawn::FireImmune => entity_type.fire_immune(),
    }
}

#[cfg(test)]
mod tests {
    use wafer_core::BlockPos;
    use wafer_data::{Block, BlockState, blocks};

    use super::is_valid_spawn;
    use crate::world::entity::entity_types;
    use crate::world::level::BlockGetter;

    struct Empty;

    impl BlockGetter for Empty {
        fn block_state(&mut self, _pos: BlockPos) -> BlockState {
            blocks::AIR.default_state()
        }

        fn get_min_y(&self) -> i32 {
            0
        }
    }

    #[test]
    fn is_valid_spawn_matches_vanilla() {
        let probes = [&entity_types::ZOMBIE, &entity_types::OCELOT, &entity_types::PARROT, &entity_types::POLAR_BEAR, &entity_types::BLAZE];
        let oracle: Vec<u32> = include_str!("valid_spawn_oracle.txt").trim().chars().map(|c| c.to_digit(32).unwrap()).collect();
        let mut wrong = Vec::new();
        for (id, &expected) in oracle.iter().enumerate() {
            let state = wafer_data::BlockState(id as u16);
            let actual = probes.iter().enumerate().fold(0, |bits, (i, probe)| bits | (is_valid_spawn(&mut Empty, state, BlockPos::new(0, 0, 0), probe) as u32) << i);
            if actual != expected {
                let block: Block = state.block();
                let entry = format!("{} {expected} {actual}", block.info().name);
                if !wrong.contains(&entry) {
                    wrong.push(entry);
                }
            }
        }
        assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    }
}
