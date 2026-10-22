use wafer_data::properties::AGE_25;

use super::ItemStack;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::{EntityId, sound_event};
use crate::world::level::BlockGetter;
use crate::world::level::block::BlockHitResult;
use crate::world::level::update_flags::UPDATE_ALL;
use crate::world::{InteractionHand, InteractionResult};

pub fn use_on(level: &mut ServerLevel, player: EntityId, hand: InteractionHand, hit: BlockHitResult, item_in_hand: ItemStack) -> InteractionResult {
    let pos = hit.block_pos;
    let state = level.block_state(pos);
    if state.block().is_instance_of("GrowingPlantHeadBlock") && state.get(AGE_25) != 25 {
        crate::advancements::triggers::criteria_triggers::ITEM_USED_ON_BLOCK.trigger(level, player, pos, &item_in_hand);
        level.play_sound(Some(player), pos, sound_event("minecraft:block.growing_plant.crop"), SoundSource::Blocks, 1.0, 1.0);
        let new_state = state.set(AGE_25, 25);
        level.set_block(pos, new_state, UPDATE_ALL);
        level.game_event_at_with(crate::world::level::gameevent::game_event::BLOCK_CHANGE, pos, crate::world::level::gameevent::game_event::Context::of_entity_and_state(Some(player), Some(new_state)));
        level.hurt_and_break(player, hand.as_equipment_slot(), 1);
        return InteractionResult::SUCCESS;
    }
    super::default_use_on(level, player, hand, hit, item_in_hand)
}

#[cfg(test)]
mod tests {
    use wafer_core::{BlockPos, Direction, Vec3};
    use wafer_data::properties::AGE_25;
    use wafer_data::{blocks, items};

    use crate::server::level::testing;
    use crate::world::item::ItemStack;
    use crate::world::level::BlockGetter;
    use crate::world::level::block::BlockHitResult;
    use crate::world::level::update_flags::UPDATE_ALL;
    use crate::world::{InteractionHand, InteractionResult};

    #[test]
    fn shears_stop_growing_plants() {
        let mut level = testing::level();
        let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 2.5), 180.0, 30.0);
        let vines = BlockPos::new(0, -59, 0);
        level.set_block(vines.below(), blocks::NETHERRACK.default_state(), UPDATE_ALL);
        level.set_block(vines, blocks::TWISTING_VINES.default_state(), UPDATE_ALL);
        level.player_mut(player).inventory.slots[36] = Some(ItemStack::new(items::SHEARS, 1));
        let hit = BlockHitResult { location: Vec3::new(0.5, -58.0, 0.5), direction: Direction::Up, block_pos: vines, inside: false, world_border_hit: false };
        assert_eq!(level.use_item_on(player, InteractionHand::MainHand, hit), InteractionResult::SUCCESS);
        assert_eq!(level.block_state(vines).get(AGE_25), 25);
        assert_eq!(level.player(player).inventory.slots[36].as_ref().map(ItemStack::damage_value), Some(1));
    }
}
