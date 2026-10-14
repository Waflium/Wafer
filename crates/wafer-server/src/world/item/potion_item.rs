use wafer_core::Direction;
use wafer_core::random::RandomSource;
use wafer_data::{block_tags, blocks, items};

use super::{ItemStack, item_utils};
use crate::core::cauldron::cauldron_interactions::is_water_potion;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::{EntityId, sound_event};
use crate::world::level::BlockGetter;
use crate::world::level::block::BlockHitResult;
use crate::world::level::update_flags::UPDATE_ALL;
use crate::world::{InteractionHand, InteractionResult};

pub fn use_on(level: &mut ServerLevel, player: EntityId, hand: InteractionHand, hit: BlockHitResult, item_stack: ItemStack) -> InteractionResult {
    let pos = hit.block_pos;
    let block_state = level.block_state(pos);
    if hit.direction != Direction::Down && block_state.is(&block_tags::CONVERTIBLE_TO_MUD) && is_water_potion(&item_stack) {
        level.play_sound(None, pos, sound_event("minecraft:entity.generic.splash"), SoundSource::Blocks, 1.0, 1.0);
        let result = item_utils::create_filled_result(level, player, hand, item_stack, ItemStack::new(items::GLASS_BOTTLE, 1), true);
        level.player_mut(player).set_item_in_hand(hand, Some(result));
        for _ in 0..5 {
            let x = pos.x as f64 + level.random.next_double();
            let z = pos.z as f64 + level.random.next_double();
            level.send_particles("minecraft:splash", |_| {}, x, (pos.y + 1) as f64, z, 1, (0.0, 0.0, 0.0), 1.0);
        }
        level.play_sound(None, pos, sound_event("minecraft:item.bottle.empty"), SoundSource::Blocks, 1.0, 1.0);
        level.set_block(pos, blocks::MUD.default_state(), UPDATE_ALL);
        return InteractionResult::SUCCESS;
    }
    InteractionResult::Pass
}

#[cfg(test)]
mod tests {
    use wafer_core::{BlockPos, Direction, Vec3};
    use wafer_data::{blocks, items};
    use wafer_protocol::nbt::Nbt;

    use crate::server::level::testing;
    use crate::world::item::ItemStack;
    use crate::world::item::component as dc;
    use crate::world::level::BlockGetter;
    use crate::world::level::block::BlockHitResult;
    use crate::world::level::update_flags::UPDATE_ALL;
    use crate::world::{InteractionHand, InteractionResult};

    fn hit(pos: BlockPos, direction: Direction) -> BlockHitResult {
        BlockHitResult { location: Vec3::new(pos.x as f64 + 0.5, pos.y as f64 + 1.0, pos.z as f64 + 0.5), direction, block_pos: pos, inside: false, world_border_hit: false }
    }

    #[test]
    fn water_bottles_turn_dirt_into_mud() {
        let mut level = testing::level();
        let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 2.5), 180.0, 30.0);
        let dirt = BlockPos::new(0, -61, 0);
        level.set_block(dirt, blocks::DIRT.default_state(), UPDATE_ALL);
        let mut water = ItemStack::new(items::POTION, 1);
        water.set(dc::POTION_CONTENTS, Some(Nbt::Compound(vec![("potion".into(), Nbt::String("minecraft:water".into()))])));
        level.player_mut(player).inventory.slots[36] = Some(water.clone());
        assert_eq!(level.use_item_on(player, InteractionHand::MainHand, hit(dirt, Direction::Down)), InteractionResult::Pass);
        assert_eq!(level.use_item_on(player, InteractionHand::MainHand, hit(dirt, Direction::Up)), InteractionResult::SUCCESS);
        assert_eq!(level.block_state(dirt), blocks::MUD.default_state());
        assert_eq!(level.player(player).inventory.slots[36], Some(ItemStack::new(items::GLASS_BOTTLE, 1)));
    }
}
