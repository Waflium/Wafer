use wafer_core::random::RandomSource;
use wafer_data::BlockState;

use crate::server::level::{ServerLevel, SoundSource};
use crate::world::InteractionHand;
use crate::world::InteractionResult;
use crate::world::entity::{EntityId, entity_selector, sound_event};
use crate::world::item::ItemStack;
use crate::world::level::BlockGetter;
use crate::world::level::block::brushable_block;
use crate::world::level::block::entity::BlockEntityKind;
use crate::world::phys::hit_result::{HitResult, Type};

pub const USE_DURATION: i32 = 200;

pub fn use_on(level: &mut ServerLevel, player: EntityId, hand: InteractionHand) -> InteractionResult {
    if calculate_hit_result(level, player).get_type() == Type::Block {
        level.start_using_item(player, hand);
    }
    InteractionResult::CONSUME
}

pub fn on_use_tick(level: &mut ServerLevel, living_entity: EntityId, item_stack: &ItemStack, ticks_remaining: i32) {
    if ticks_remaining < 0 || !level.is_player(living_entity) {
        level.release_using_item(living_entity);
        return;
    }
    let player = living_entity;
    let hit_result = calculate_hit_result(level, player);
    let HitResult::Block { hit, miss: false } = hit_result else {
        level.release_using_item(living_entity);
        return;
    };
    let time_elapsed = USE_DURATION - ticks_remaining + 1;
    let is_last_tick_before_backswing = time_elapsed % 10 == 5;
    if !is_last_tick_before_backswing {
        return;
    }
    let pos = hit.block_pos;
    let state = level.block_state(pos);
    if spawns_dust_particles(state) {
        spawn_dust_particles(level);
    }
    let brush_sound = brushable_block::of(state.block()).map_or("minecraft:item.brush.brushing.generic", |block| block.get_brush_sound());
    level.play_sound(Some(player), pos, sound_event(brush_sound), SoundSource::Blocks, 1.0, 1.0);
    if level.chunks.block_entity_ref(pos).is_some_and(|entity| matches!(entity.kind, BlockEntityKind::Brushable(_))) {
        let game_time = level.game_time;
        let brushing_updated_state = level.brushable_brush(pos, game_time, player, hit.direction, item_stack);
        if brushing_updated_state {
            let equipped_hand = level.used_item_hand(player).as_equipment_slot();
            level.hurt_and_break(player, equipped_hand, 1);
        }
    }
}

fn calculate_hit_result(level: &mut ServerLevel, player: EntityId) -> HitResult {
    let distance = level.attribute_value(player, crate::world::entity::attributes::Attribute::BlockInteractionRange);
    level.get_hit_result_on_view_vector(player, |level, entity| entity_selector::no_spectators(level, entity) && level.is_pickable(entity), distance)
}

fn spawns_dust_particles(state: BlockState) -> bool {
    let block = state.block();
    !["AirBlock", "BarrierBlock", "BubbleColumnBlock", "EndGatewayBlock", "EndPortalBlock", "LightBlock", "LiquidBlock", "StructureVoidBlock", "MovingPistonBlock"].iter().any(|class| block.is_instance_of(class))
}

fn spawn_dust_particles(level: &mut ServerLevel) {
    let particles = level.random.next_int_range(7, 12);
    for _ in 0..particles {
        level.random.next_double();
        level.random.next_double();
    }
}
