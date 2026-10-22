use wafer_core::{BlockPos, Direction, Vec3};
use wafer_data::{game_rule, registry, registry_id};

use super::bucket_item::player_pov_hit_result;
use crate::server::level::ServerLevel;
use crate::server::level::game_mode::GameType;
use crate::world::level::block::entity::BlockEntityKind;
use crate::world::level::chunk::block_entities::EntityCreationType;
use crate::world::InteractionHand;
use crate::world::InteractionResult;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::item::component::{self as dc, values};
use crate::world::level::BlockGetter;
use crate::world::level::block::BlockHitResult;
use crate::world::level::clip::ClipFluid;

pub fn use_on(level: &mut ServerLevel, player: EntityId, hand: InteractionHand, hit: BlockHitResult, mut item_stack: ItemStack) -> InteractionResult {
    let Some(entity_type) = get_type(&item_stack) else { return InteractionResult::Fail };
    let pos = hit.block_pos;
    let clicked_face = hit.direction;
    let block_state = level.block_state(pos);
    if level.get_block_entity(pos).is_some_and(|block_entity| matches!(block_entity.kind, BlockEntityKind::Spawner(_))) {
        if !level.game_rules.get_bool(game_rule::SPAWNER_BLOCKS_WORK) {
            level.send_system_message(player, &crate::network::chat::Component::translatable("advMode.notEnabled.spawner"));
            return InteractionResult::Fail;
        }
        let entity_data = item_stack.get(dc::ENTITY_DATA).cloned();
        let is_op = level.is_op(level.player(player).profile.uuid);
        if let Some(block_entity) = level.chunks.block_entity(pos, EntityCreationType::Immediate)
            && let BlockEntityKind::Spawner(spawner_holder) = &mut block_entity.kind
        {
            match entity_data {
                Some(entity_data) if is_op => spawner_holder.spawner.set_entity_data(&entity_data, &mut level.random),
                _ => spawner_holder.spawner.set_entity_id(registry::ENTITY_TYPE[entity_type as usize], &mut level.random),
            }
        }
        level.block_entity_set_changed(pos);
        level.send_block_updated(pos, block_state, block_state);
        level.game_event_at(Some(player), crate::world::level::gameevent::game_event::BLOCK_CHANGE, pos);
        if level.player(player).game_mode.game_type != GameType::Creative {
            item_stack.shrink(1);
            level.player_mut(player).update_item_in_hand(hand, &item_stack);
        }
        return InteractionResult::SUCCESS;
    }
    let spawn_pos = if block_state.collision_voxel_shape().is_empty() { pos } else { pos.relative(clicked_face) };
    let moved_up = pos != spawn_pos && clicked_face == Direction::Up;
    let result = spawn_mob(level, entity_type, Some(player), &mut item_stack, spawn_pos, true, moved_up);
    level.player_mut(player).update_item_in_hand(hand, &item_stack);
    result
}

fn spawn_mob(level: &mut ServerLevel, entity_type: i32, user: Option<EntityId>, item_stack: &mut ItemStack, spawn_pos: BlockPos, try_move_down: bool, moved_up: bool) -> InteractionResult {
    let name = registry::ENTITY_TYPE[entity_type as usize];
    if level.spawn_entity_type_from_stack(name, Some(item_stack), spawn_pos, crate::world::entity::entity_spawn_reason::EntitySpawnReason::SpawnItemUse, try_move_down, moved_up).is_some() {
        item_stack.consume(1, level, user);
        level.game_event_at(user, crate::world::level::gameevent::game_event::ENTITY_PLACE, spawn_pos);
        InteractionResult::SUCCESS
    } else {
        InteractionResult::Fail
    }
}

pub fn use_item(level: &mut ServerLevel, player: EntityId, hand: InteractionHand) -> (InteractionResult, Option<ItemStack>) {
    let Some(mut item_stack) = level.player(player).item_in_hand(hand) else { return (InteractionResult::Pass, None) };
    let Some(hit) = player_pov_hit_result(level, player, ClipFluid::SourceOnly) else { return (InteractionResult::Pass, None) };
    let Some(entity_type) = get_type(&item_stack) else { return (InteractionResult::Fail, None) };
    let pos = hit.block_pos;
    if !level.block_state(pos).block().is_instance_of("LiquidBlock") {
        return (InteractionResult::Pass, None);
    }
    if level.may_interact(player, pos) && level.may_use_item_at(player, pos, hit.direction, &item_stack) {
        let this = item_stack.item;
        let result = spawn_mob(level, entity_type, Some(player), &mut item_stack, pos, false, false);
        if result == InteractionResult::SUCCESS {
            level.award_stat(player, crate::stats::stats::ITEM_USED.get(this.0), 1);
        }
        (result, Some(item_stack))
    } else {
        (InteractionResult::Fail, None)
    }
}

pub fn by_id(entity_type: i32) -> Option<wafer_data::Item> {
    static SPAWN_EGGS: std::sync::LazyLock<Vec<(i32, wafer_data::Item)>> = std::sync::LazyLock::new(|| {
        (0..wafer_data::Item::count()).map(|i| wafer_data::Item(i as u16)).filter_map(|item| get_type(&ItemStack::new(item, 1)).map(|ty| (ty, item))).collect()
    });
    SPAWN_EGGS.iter().find(|&&(ty, _)| ty == entity_type).map(|&(_, item)| item)
}

pub fn spawns_entity(item_stack: &ItemStack, entity_type: i32) -> bool {
    get_type(item_stack) == Some(entity_type)
}

pub fn get_type(item_stack: &ItemStack) -> Option<i32> {
    let entity_data = item_stack.get(dc::ENTITY_DATA)?;
    Some(registry_id(registry::ENTITY_TYPE, &values::typed_entity_data(entity_data).0))
}

pub fn spawn_offspring_from_spawn_egg(level: &mut ServerLevel, player: EntityId, parent: EntityId, entity_type: i32, pos: Vec3, spawn_egg_stack: &mut ItemStack) -> Option<EntityId> {
    if !spawns_entity(spawn_egg_stack, entity_type) {
        return None;
    }
    let mut offspring = if level.mob(parent).is_some_and(|mob| mob.ageable().is_some()) {
        level.get_breed_offspring(parent, parent)?
    } else {
        let LevelEntity::Mob(mob) = ServerLevel::create_entity(registry::ENTITY_TYPE[entity_type as usize])? else { return None };
        mob
    };
    offspring.set_baby(true);
    if !offspring.is_baby() {
        return None;
    }
    offspring.entity.snap_to(pos.x, pos.y, pos.z, 0.0, 0.0);
    offspring.apply_components_from_item_stack(spawn_egg_stack);
    let id = offspring.entity.id;
    level.add_fresh_entity(LevelEntity::Mob(offspring));
    spawn_egg_stack.consume(1, level, Some(player));
    Some(id)
}

#[cfg(test)]
mod tests {
    use wafer_core::{BlockPos, Direction, Vec3};
    use wafer_data::{blocks, game_rule, items};

    use crate::server::level::testing;
    use crate::world::item::ItemStack;
    use crate::world::level::block::BlockHitResult;
    use crate::world::level::block::entity::BlockEntityKind;
    use crate::world::level::update_flags::UPDATE_ALL;
    use crate::world::{InteractionHand, InteractionResult};

    fn spawner_entity_id(level: &mut crate::server::level::ServerLevel, pos: BlockPos) -> Option<String> {
        let BlockEntityKind::Spawner(spawner) = &level.get_block_entity(pos)?.kind else { return None };
        let mut output = Vec::new();
        spawner.save_additional(&mut output);
        let spawn_data = output.into_iter().find(|(key, _)| key == "SpawnData")?.1;
        spawn_data.get("entity")?.get("id")?.as_str().map(str::to_string)
    }

    #[test]
    fn spawn_eggs_set_the_spawner_type() {
        let mut level = testing::level();
        let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 2.5), 180.0, 30.0);
        let pos = BlockPos::new(0, -60, 0);
        level.set_block(pos, blocks::SPAWNER.default_state(), UPDATE_ALL);
        level.player_mut(player).inventory.slots[36] = Some(ItemStack::new(items::PIG_SPAWN_EGG, 2));
        let hit = BlockHitResult { location: Vec3::new(0.5, -59.0, 0.5), direction: Direction::Up, block_pos: pos, inside: false, world_border_hit: false };
        level.game_rules.set(game_rule::SPAWNER_BLOCKS_WORK, 0);
        assert_eq!(level.use_item_on(player, InteractionHand::MainHand, hit), InteractionResult::Fail);
        level.game_rules.set(game_rule::SPAWNER_BLOCKS_WORK, 1);
        assert_eq!(level.use_item_on(player, InteractionHand::MainHand, hit), InteractionResult::SUCCESS);
        assert_eq!(spawner_entity_id(&mut level, pos).as_deref(), Some("minecraft:pig"));
        assert_eq!(level.player(player).inventory.slots[36], Some(ItemStack::new(items::PIG_SPAWN_EGG, 1)));
    }
}
