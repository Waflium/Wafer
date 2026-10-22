use wafer_core::random::RandomSource;
use wafer_core::{AABB, BlockPos, Direction, Vec3};
use wafer_data::properties::{HORIZONTAL_FACING, LEVEL_HONEY};

use wafer_data::{BlockState, EquipmentSlot, game_rule, items, registry, registry_id};

use super::mirror::Mirror;
use super::rotation::Rotation;
use super::{BlockBehaviour, BlockHitResult, campfire_block};
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::{EntityId, ItemEntity, LevelEntity, entity_types};
use crate::world::item::ItemStack;
use crate::world::item::component::values::block_state_properties_nbt;
use crate::world::item::component::{self as dc};
use crate::world::item::item_utils::has_infinite_materials;
use crate::world::level::{BlockGetter, LevelAccessor};
use crate::world::level::block;
use crate::world::level::block::entity::beehive_block_entity::BeeReleaseStatus;
use crate::world::level::block::entity::{BlockEntity, BlockEntityKind};
use crate::world::level::storage::loot::loot_params::LootParams;
use crate::world::level::update_flags::UPDATE_ALL;
use crate::world::{InteractionHand, InteractionResult};

fn anger_nearby_bees(level: &mut ServerLevel, pos: BlockPos) {
    let area_around_beehive = AABB::of_block(pos).inflate(8.0, 6.0, 8.0);
    let bees_to_anger = level.get_entities_matching(None, &area_around_beehive, |level, entity| level.is_bee(entity) && !level.is_spectator(entity));
    if bees_to_anger.is_empty() {
        return;
    }
    let players_to_be_angry_at = level.get_entities_matching(None, &area_around_beehive, |level, entity| level.is_player(entity) && !level.is_spectator(entity));
    if players_to_be_angry_at.is_empty() {
        return;
    }
    for bee in bees_to_anger {
        if level.get_target(bee).is_none() {
            let anger_target = players_to_be_angry_at[level.random.next_int_bounded(players_to_be_angry_at.len() as i32) as usize];
            level.set_target(bee, Some(anger_target));
        }
    }
}

pub fn drop_honeycomb(level: &mut ServerLevel, tool: &ItemStack, block_state: BlockState, entity: Option<EntityId>, pos: BlockPos) {
    let block_entity = level.get_block_entity(pos).cloned();
    block::drop_from_block_interact_loot_table(level, "minecraft:harvest/beehive", pos, block_state, block_entity.as_ref(), Some(tool), entity, |level, stack| {
        level.pop_resource(pos, stack)
    });
}

fn hive_contains_bees(level: &mut ServerLevel, pos: BlockPos) -> bool {
    match level.get_block_entity(pos).map(|block_entity| &block_entity.kind) {
        Some(BlockEntityKind::Beehive(beehive)) => !beehive.is_empty(),
        _ => false,
    }
}

pub fn release_bees_and_reset_honey_level(level: &mut ServerLevel, state: BlockState, pos: BlockPos, player: Option<EntityId>, bee_release_status: BeeReleaseStatus) {
    reset_honey_level(level, state, pos);
    level.with_beehive(pos, |level, beehive, block_entity_state| beehive.empty_all_living_from_hive(level, pos, block_entity_state, player, state, bee_release_status));
}

pub fn reset_honey_level(level: &mut ServerLevel, state: BlockState, pos: BlockPos) {
    level.set_block(pos, state.set(LEVEL_HONEY, 0), UPDATE_ALL);
}

fn is_explosive(level: &ServerLevel, entity: EntityId) -> bool {
    let entity_type = level.entity(entity).entity_type;
    [entity_types::TNT.id, entity_types::CREEPER.id, entity_types::WITHER_SKULL.id, entity_types::WITHER.id, entity_types::TNT_MINECART.id].contains(&entity_type)
}

pub struct BeehiveBlock;

impl BlockBehaviour for BeehiveBlock {
    fn get_analog_output_signal(&self, state: BlockState, _level: &mut ServerLevel, _pos: BlockPos, _direction: Direction) -> i32 {
        state.get(LEVEL_HONEY)
    }

    fn player_destroy(&self, level: &mut ServerLevel, player: EntityId, pos: BlockPos, state: BlockState, block_entity: Option<&BlockEntity>, tool: Option<ItemStack>) {
        let destroyed_with = tool.clone().unwrap_or_else(ItemStack::empty);
        level.drop_resources_with_entity(state, pos, block_entity, Some(player), tool);
        let Some(block_entity) = block_entity else { return };
        let BlockEntityKind::Beehive(beehive_block_entity) = &block_entity.kind else { return };
        let mut beehive_block_entity = beehive_block_entity.clone();
        if !crate::world::item::enchantment::enchantment_helper::has_tag(&destroyed_with, "minecraft:prevents_bee_spawns_when_mining") {
            beehive_block_entity.empty_all_living_from_hive(level, pos, block_entity.block_state(), Some(player), state, BeeReleaseStatus::Emergency);
            crate::world::containers::update_neighbours_after_destroy(level, state, pos);
            anger_nearby_bees(level, pos);
        }
        crate::advancements::triggers::criteria_triggers::BEE_NEST_DESTROYED.trigger(level, player, state, &destroyed_with, beehive_block_entity.get_occupant_count());
    }

    fn on_explosion_hit(
        &self,
        state: BlockState,
        level: &mut ServerLevel,
        pos: BlockPos,
        explosion: &crate::world::level::server_explosion::ServerExplosion,
        stacks: &mut Vec<(BlockPos, ItemStack)>,
    ) {
        level.block_on_explosion_hit(state, pos, explosion, stacks);
        anger_nearby_bees(level, pos);
    }

    fn use_item_on(
        &self,
        item_stack: ItemStack,
        state: BlockState,
        level: &mut ServerLevel,
        pos: BlockPos,
        player: EntityId,
        hand: InteractionHand,
        _hit: &BlockHitResult,
    ) -> InteractionResult {
        let honey_level = state.get(LEVEL_HONEY);
        let mut hive_emptied = false;
        if honey_level >= 5 {
            let item = item_stack.get_item();
            let player_pos = level.entity(player).position();
            if item_stack.is(items::SHEARS) {
                drop_honeycomb(level, &item_stack, state, Some(player), pos);
                level.play_sound_at(None, player_pos.x, player_pos.y, player_pos.z, registry_id(registry::SOUND_EVENT, "minecraft:block.beehive.shear"), SoundSource::Blocks, 1.0, 1.0);
                let slot = if hand == InteractionHand::MainHand { EquipmentSlot::Mainhand } else { EquipmentSlot::Offhand };
                level.hurt_and_break(player, slot, 1);
                hive_emptied = true;
            } else if item_stack.is(items::GLASS_BOTTLE) {
                let creative = has_infinite_materials(level, player);
                let mut item_stack = item_stack;
                item_stack.shrink(1);
                level.play_sound_at(Some(player), player_pos.x, player_pos.y, player_pos.z, registry_id(registry::SOUND_EVENT, "minecraft:item.bottle.fill"), SoundSource::Blocks, 1.0, 1.0);
                let honey_bottle = ItemStack::new(items::HONEY_BOTTLE, 1);
                if item_stack.is_empty() {
                    level.player_mut(player).set_item_in_hand(hand, Some(honey_bottle));
                } else {
                    if !creative {
                        level.player_mut(player).update_item_in_hand(hand, &item_stack);
                    }
                    if level.player_mut(player).pick_up(honey_bottle.clone()) > 0 {
                        level.player_drop(player, honey_bottle, false, crate::util::Prediction::Predicted);
                    }
                }
                hive_emptied = true;
            }
            if hive_emptied {
                level.award_stat(player, crate::stats::stats::ITEM_USED.get(item.0), 1);
            }
        }
        if hive_emptied {
            if !campfire_block::is_smokey_pos(level, pos) {
                if hive_contains_bees(level, pos) {
                    anger_nearby_bees(level, pos);
                }
                release_bees_and_reset_honey_level(level, state, pos, Some(player), BeeReleaseStatus::Emergency);
            } else {
                reset_honey_level(level, state, pos);
            }
            InteractionResult::SUCCESS
        } else {
            InteractionResult::TryWithEmptyHand
        }
    }

    fn player_will_destroy(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState, player: EntityId) -> BlockState {
        if level.prevents_block_drops(player)
            && level.game_rules.get_bool(game_rule::BLOCK_DROPS)
            && let Some(block_entity) = level.get_block_entity(pos)
            && let BlockEntityKind::Beehive(beehive_block_entity) = &block_entity.kind
        {
            let honey_level = state.get(LEVEL_HONEY);
            let has_bees = !beehive_block_entity.is_empty();
            if has_bees || honey_level > 0 {
                let mut item_stack = ItemStack::new(state.block().item(), 1);
                for (ty, value) in block_entity.collect_components() {
                    if let Some(value) = value {
                        item_stack.set(ty, Some(value));
                    }
                }
                item_stack.set(dc::BLOCK_STATE, Some(block_state_properties_nbt(&[("honey_level", &honey_level.to_string())])));
                let mut entity = ItemEntity::new(Vec3::new(pos.x as f64, pos.y as f64, pos.z as f64), item_stack);
                entity.set_default_pickup_delay();
                level.add_fresh_entity(LevelEntity::Item(entity));
            }
        }
        super::block_player_will_destroy(level, pos, state, player)
    }

    fn get_drops(&self, state: BlockState, params: LootParams) -> Vec<ItemStack> {
        if params.this_entity.is_some_and(|entity| is_explosive(params.level, entity))
            && let Some(block_entity) = params.block_entity
            && let BlockEntityKind::Beehive(beehive_block_entity) = &block_entity.kind
        {
            let mut emptied = block_entity.clone();
            let mut beehive_block_entity = beehive_block_entity.clone();
            beehive_block_entity.empty_all_living_from_hive(params.level, block_entity.pos(), block_entity.block_state(), None, state, BeeReleaseStatus::Emergency);
            emptied.kind = BlockEntityKind::Beehive(beehive_block_entity);
            return block::block_get_drops(state, LootParams { block_entity: Some(&emptied), ..params });
        }
        block::block_get_drops(state, params)
    }

    fn get_clone_item_stack(&self, _level: &ServerLevel, _pos: BlockPos, state: BlockState, include_data: bool) -> ItemStack {
        let mut stack = ItemStack::new(state.block().item(), 1);
        if include_data {
            stack.set(dc::BLOCK_STATE, Some(block_state_properties_nbt(&[("honey_level", &state.get(LEVEL_HONEY).to_string())])));
        }
        stack
    }

    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, _direction: Direction, neighbour_pos: BlockPos, _neighbour_state: BlockState) -> BlockState {
        if let Some(level) = level.as_server_level()
            && level.block_state(neighbour_pos).block().is_instance_of("FireBlock")
        {
            level.with_beehive(pos, |level, beehive, block_entity_state| beehive.empty_all_living_from_hive(level, pos, block_entity_state, None, state, BeeReleaseStatus::Emergency));
        }
        state
    }

    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        state.set(HORIZONTAL_FACING, rotation.rotate(state.get(HORIZONTAL_FACING)))
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        super::rotate(state, mirror.get_rotation(state.get(HORIZONTAL_FACING)))
    }
}
