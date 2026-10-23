use wafer_core::BlockPos;
use wafer_data::{blocks, items};
use wafer_protocol::nbt::Nbt;

use crate::server::level::{ServerLevel, SoundSource};
use crate::util::Prediction;
use crate::world::entity::ai::village::poi::poi_types::PoiType;
use crate::world::entity::{EntityId, sound_event};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::level::BlockGetter;
use crate::world::level::block::BlockHitResult;
use crate::world::{InteractionHand, InteractionResult};

fn tracker(dimension: &str, pos: BlockPos) -> Nbt {
    Nbt::Compound(vec![(
        "target".into(),
        Nbt::Compound(vec![("dimension".into(), Nbt::String(dimension.into())), ("pos".into(), Nbt::IntArray(vec![pos.x, pos.y, pos.z]))]),
    )])
}

pub fn inventory_tick(item_stack: &mut ItemStack, level: &mut ServerLevel) {
    let Some(tracker) = item_stack.get(dc::LODESTONE_TRACKER) else { return };
    let tracked = tracker.get("tracked").and_then(Nbt::as_i64).is_none_or(|tracked| tracked as i8 != 0);
    let Some(target) = tracker.get("target").filter(|_| tracked) else { return };
    if target.get("dimension").and_then(Nbt::as_str) != Some(level.dimension) {
        return;
    }
    let Some(Nbt::IntArray(pos)) = target.get("pos") else { return };
    let block_pos = BlockPos::new(pos[0], pos[1], pos[2]);
    if !(level.is_in_world_bounds(block_pos) && level.chunks.poi_manager.exists(block_pos, |ty| ty == PoiType::Lodestone)) {
        item_stack.set(dc::LODESTONE_TRACKER, Some(Nbt::Compound(Vec::new())));
    }
}

pub fn use_on(level: &mut ServerLevel, player: EntityId, hand: InteractionHand, hit: BlockHitResult, mut item_stack: ItemStack) -> InteractionResult {
    let block_pos = hit.block_pos;
    if !level.block_state(block_pos).is_block(blocks::LODESTONE) {
        return InteractionResult::Pass;
    }
    level.play_sound_at(None, block_pos.x as f64 + 0.5, block_pos.y as f64 + 0.5, block_pos.z as f64 + 0.5, sound_event("minecraft:item.lodestone_compass.lock"), SoundSource::Players, 1.0, 1.0);
    let replace_existing_stack = !level.has_infinite_materials(player) && item_stack.count == 1;
    let target = tracker(level.dimension, block_pos);
    if replace_existing_stack {
        item_stack.set(dc::LODESTONE_TRACKER, Some(target));
        level.player_mut(player).update_item_in_hand(hand, &item_stack);
    } else {
        let mut lodestone_compass = item_stack.transmute_copy(items::COMPASS, 1);
        item_stack.consume(1, level, Some(player));
        level.player_mut(player).update_item_in_hand(hand, &item_stack);
        lodestone_compass.set(dc::LODESTONE_TRACKER, Some(target));
        let infinite_materials = level.has_infinite_materials(player);
        let mut copy = lodestone_compass.copy();
        if !level.player_mut(player).inventory.add_stack(&mut copy, infinite_materials).0 {
            level.player_drop(player, lodestone_compass, false, Prediction::Predicted);
        }
    }
    InteractionResult::SUCCESS
}
