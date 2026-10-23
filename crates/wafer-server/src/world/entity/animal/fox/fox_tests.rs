use wafer_core::{BlockPos, Vec3};
use wafer_data::{EquipmentSlot, blocks, items};
use wafer_protocol::nbt::Nbt;

use super::fox::{self, Fox, Variant};
use crate::server::level::{ServerLevel, testing};
use crate::world::clock::ServerClockManager;
use crate::world::entity::ai::goal::goal::Goal;
use crate::world::entity::ai::goal::target::nearest_attackable_target_goal::TargetType;
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::item::ItemEntity;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::level::BlockGetter;
use crate::world::level::update_flags::UPDATE_ALL;
use crate::world::{InteractionHand, InteractionResult};

fn setup(pos: Vec3) -> (ServerLevel, EntityId, EntityId) {
    let mut level = testing::level();
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 40.5), 0.0, 0.0);
    let fox = add_fox(&mut level, pos);
    (level, player, fox)
}

fn add_fox(level: &mut ServerLevel, pos: Vec3) -> EntityId {
    let mut mob = Fox::create();
    mob.entity.set_pos(pos);
    let id = mob.entity.id;
    level.add_fresh_entity(LevelEntity::Mob(Box::new(mob)));
    id
}

fn held(level: &ServerLevel, id: EntityId) -> Option<ItemStack> {
    level.item_by_slot(id, EquipmentSlot::Mainhand)
}

fn target_goals(level: &ServerLevel, id: EntityId) -> Vec<(i32, TargetType)> {
    let goals = level.mob(id).unwrap().ai.target_selector.get_available_goals();
    goals
        .iter()
        .filter_map(|goal| match goal.get_goal() {
            Goal::NearestAttackableTarget(target_goal) => Some((goal.get_priority(), target_goal.target_type())),
            _ => None,
        })
        .collect()
}

fn set_time(level: &mut ServerLevel, ticks: i64) {
    let mut clocks = ServerClockManager::new();
    clocks.set_total_ticks(ServerClockManager::id_of("minecraft:overworld").unwrap(), ticks);
    level.sync_clocks(&clocks);
}

fn fill(level: &mut ServerLevel, from: (i32, i32, i32), to: (i32, i32, i32), block: wafer_data::Block) {
    for x in from.0..=to.0 {
        for y in from.1..=to.1 {
            for z in from.2..=to.2 {
                level.set_block(BlockPos::new(x, y, z), block.default_state(), UPDATE_ALL);
            }
        }
    }
}

#[test]
fn foxes_save_and_load_their_state() {
    let (mut level, player, id) = setup(Vec3::new(0.5, -60.0, 0.5));
    let player_uuid = level.entity(player).uuid;
    let e = level.entity_mut(id);
    fox::set_variant(e, Variant::Snow);
    fox::set_sitting(e, true);
    fox::set_is_crouching(e, true);
    e.data.set(fox::DATA_TRUSTED_ID_0, crate::world::entity::data::DataValue::OptionalLivingEntityReference(Some(player_uuid)));
    let nbt = level.entities[&id].save().unwrap();
    assert_eq!(nbt.get("Type").and_then(Nbt::as_str), Some("snow"));
    assert_eq!(nbt.get("Sitting").and_then(Nbt::as_i64), Some(1));
    assert_eq!(nbt.get("Crouching").and_then(Nbt::as_i64), Some(1));
    assert_eq!(nbt.get("Sleeping").and_then(Nbt::as_i64), Some(0));
    assert!(matches!(nbt.get("Trusted"), Some(Nbt::List(list)) if list.len() == 1));
    let loaded = level.load_entity(&nbt).unwrap();
    let LevelEntity::Mob(mob) = &loaded else { panic!("a fox") };
    assert_eq!(fox::get_variant(&mob.entity), Variant::Snow);
    assert!(fox::is_sitting(&mob.entity));
    assert!(fox::is_crouching(&mob.entity));
    assert!(!fox::is_sleeping(&mob.entity));
    assert_eq!(fox::get_trusted_entities(&mob.entity).map(|r| r.get_uuid()).collect::<Vec<_>>(), vec![player_uuid]);
    let goals = mob.ai.target_selector.get_available_goals();
    assert_eq!(goals.len(), 4);
}

#[test]
fn loading_twice_keeps_the_first_target_goal_priorities() {
    let (mut level, _player, id) = setup(Vec3::new(0.5, -60.0, 0.5));
    let mut nbt = level.entities[&id].save().unwrap();
    assert!(target_goals(&level, id).is_empty());
    if let Nbt::Compound(fields) = &mut nbt {
        fields.retain(|(key, _)| key != "Type" && key != "UUID");
        fields.push(("Type".into(), Nbt::String("snow".into())));
    }
    let loaded = level.load_entity(&nbt).unwrap();
    let new_id = loaded.entity().id;
    level.add_fresh_entity(loaded);
    assert_eq!(target_goals(&level, new_id), vec![(4, TargetType::AbstractFish), (6, TargetType::Animal), (6, TargetType::Turtle)]);
    let difficulty = level.current_difficulty_at(BlockPos::new(0, -60, 0));
    level.finalize_spawn(new_id, &difficulty, EntitySpawnReason::Command, None);
    assert_eq!(fox::get_variant(level.entity(new_id)), Variant::Red);
    assert_eq!(target_goals(&level, new_id), vec![(4, TargetType::AbstractFish), (6, TargetType::Animal), (6, TargetType::Turtle)]);
}

#[test]
fn foxes_spawn_red_in_plains_and_snow_in_snowy_biomes() {
    let (mut level, _player, id) = setup(Vec3::new(0.5, -60.0, 0.5));
    let difficulty = level.current_difficulty_at(BlockPos::new(0, -60, 0));
    let group_data = level.finalize_spawn(id, &difficulty, EntitySpawnReason::Natural, None);
    assert_eq!(fox::get_variant(level.entity(id)), Variant::Red);
    assert_eq!(target_goals(&level, id), vec![(4, TargetType::Animal), (4, TargetType::Turtle), (6, TargetType::AbstractFish)]);
    let Some(crate::world::entity::spawn_group_data::SpawnGroupData::Fox(group_data)) = group_data else { panic!("fox group data") };
    assert_eq!(group_data.variant, Variant::Red);
    assert_eq!(group_data.ageable_mob_group_data.get_group_size(), 1);
    let snowy = (0..u16::MAX).map(crate::world::level::chunk::Biome).find(|biome| biome.is("minecraft:spawns_snow_foxes")).unwrap();
    assert_eq!(Variant::by_biome(snowy), Variant::Snow);
}

#[test]
fn the_third_fox_of_a_pack_is_a_baby_of_the_packs_variant() {
    let (mut level, _player, first) = setup(Vec3::new(0.5, -60.0, 0.5));
    let difficulty = level.current_difficulty_at(BlockPos::new(0, -60, 0));
    let mut group_data = Some(crate::world::entity::spawn_group_data::SpawnGroupData::Fox(fox::FoxGroupData::new(Variant::Snow)));
    let mut foxes = vec![first];
    for x in 1..3 {
        foxes.push(add_fox(&mut level, Vec3::new(x as f64 + 0.5, -60.0, 0.5)));
    }
    for &id in &foxes {
        group_data = level.finalize_spawn(id, &difficulty, EntitySpawnReason::Natural, group_data);
    }
    let ages: Vec<bool> = foxes.iter().map(|&id| level.is_baby(id)).collect();
    assert_eq!(ages, vec![false, false, true]);
    assert!(foxes.iter().all(|&id| fox::get_variant(level.entity(id)) == Variant::Snow));
}

#[test]
fn bred_foxes_trust_the_players_who_fed_their_parents() {
    let (mut level, player, a) = setup(Vec3::new(0.5, -60.0, 0.5));
    let b = add_fox(&mut level, Vec3::new(2.5, -60.0, 0.5));
    for id in [a, b] {
        level.player_mut(player).set_item_in_hand(InteractionHand::MainHand, Some(ItemStack::new(items::SWEET_BERRIES, 1)));
        let location = level.entity(id).position();
        assert_eq!(level.interact_on(player, id, InteractionHand::MainHand, location), InteractionResult::SUCCESS_SERVER);
    }
    let mut baby = None;
    for _ in 0..300 {
        level.tick();
        baby = level.entities.keys().copied().find(|&id| level.is_fox(id) && level.is_baby(id));
        if baby.is_some() {
            break;
        }
    }
    let baby = baby.expect("the foxes never bred");
    assert!(level.trusts(baby, player));
    assert!(!level.trusts(a, player));
    assert!(target_goals(&level, baby).is_empty());
}

#[test]
fn foxes_defend_trusted_players_from_their_attackers() {
    let (mut level, player, id) = setup(Vec3::new(0.5, -60.0, 36.5));
    level.difficulty = crate::world::difficulty::Difficulty::Normal;
    let player_uuid = level.entity(player).uuid;
    level.entity_mut(id).data.set(fox::DATA_TRUSTED_ID_0, crate::world::entity::data::DataValue::OptionalLivingEntityReference(Some(player_uuid)));
    let LevelEntity::Mob(mut zombie) = ServerLevel::create_entity("minecraft:zombie").unwrap() else { unreachable!() };
    zombie.entity.set_pos(Vec3::new(1.5, -60.0, 41.5));
    let zombie = {
        let zombie_id = zombie.entity.id;
        level.add_fresh_entity(LevelEntity::Mob(zombie));
        zombie_id
    };
    for _ in 0..5 {
        level.tick();
    }
    level.set_last_hurt_mob(zombie, player);
    let (zombie_uuid, tick_count) = (level.entity(zombie).uuid, level.entity(player).tick_count);
    let living = level.living_mut(player);
    living.last_hurt_by_mob = Some(zombie_uuid);
    living.last_hurt_by_mob_timestamp = tick_count;
    level.entity_mut(id).random = wafer_core::random::LegacyRandomSource::new(5);
    level.entity_mut(zombie).random = wafer_core::random::LegacyRandomSource::new(6);
    let mut defended = false;
    for _ in 0..80 {
        level.tick();
        if level.get_target(id) == Some(zombie) {
            defended = true;
            break;
        }
    }
    assert!(defended, "the fox never turned on the zombie");
    assert!(fox::is_defending(level.entity(id)));
    level.set_target(id, None);
    assert!(!fox::is_defending(level.entity(id)));
}

#[test]
fn foxes_pick_up_items_and_spit_out_what_they_held() {
    let (mut level, _player, id) = setup(Vec3::new(0.5, -60.0, 0.5));
    level.set_item_slot(id, EquipmentSlot::Mainhand, ItemStack::new(items::FEATHER, 1));
    level.add_fresh_entity(LevelEntity::Item(ItemEntity::with_movement(Vec3::new(0.5, -60.0, 0.5), ItemStack::new(items::WHEAT, 3), Vec3::ZERO)));
    let item = level.items_in(&level.entity(id).bounding_box().inflate(1.0, 1.0, 1.0), None)[0];
    level.fox_pick_up_item(id, item);
    assert!(held(&level, id).is_none_or(|stack| stack.is(items::FEATHER)), "feathers are not food, so foxes keep them");
    level.set_item_slot(id, EquipmentSlot::Mainhand, ItemStack::empty());
    level.fox_pick_up_item(id, item);
    assert_eq!(held(&level, id).map(|stack| (stack.get_item(), stack.count)), Some((items::WHEAT, 1)));
    assert!(level.entity(item).is_removed());
    let counts: Vec<i32> = level.items_in(&level.entity(id).bounding_box().inflate(2.0, 2.0, 2.0), None).into_iter().filter_map(|item| level.entities[&item].as_item().map(|item| item.item.count)).collect();
    assert_eq!(counts, vec![2]);
    assert_eq!(level.mob(id).unwrap().drop_chances[EquipmentSlot::Mainhand as usize], 2.0);
}

#[test]
fn foxes_swap_held_items_for_food_and_spit_the_old_item_out() {
    let (mut level, _player, id) = setup(Vec3::new(0.5, -60.0, 0.5));
    level.set_item_slot(id, EquipmentSlot::Mainhand, ItemStack::new(items::FEATHER, 1));
    level.tick();
    level.add_fresh_entity(LevelEntity::Item(ItemEntity::with_movement(Vec3::new(0.5, -60.0, 0.5), ItemStack::new(items::SWEET_BERRIES, 1), Vec3::ZERO)));
    let item = level.items_in(&level.entity(id).bounding_box().inflate(1.0, 1.0, 1.0), None).into_iter().find(|&item| level.entities[&item].as_item().is_some_and(|i| i.item.is(items::SWEET_BERRIES))).unwrap();
    level.fox_pick_up_item(id, item);
    assert_eq!(held(&level, id).map(|stack| stack.get_item()), Some(items::SWEET_BERRIES));
    let spat = level.items_in(&level.entity(id).bounding_box().inflate(3.0, 3.0, 3.0), None).into_iter().filter_map(|item| level.entities[&item].as_item()).find(|item| item.item.is(items::FEATHER)).unwrap();
    assert_eq!(spat.pickup_delay, 40);
    assert_eq!(spat.thrower, Some(level.entity(id).uuid));
}

#[test]
fn foxes_eat_held_food_after_six_hundred_ticks() {
    let (mut level, _player, id) = setup(Vec3::new(0.5, -60.0, 0.5));
    level.set_item_slot(id, EquipmentSlot::Mainhand, ItemStack::new(items::SWEET_BERRIES, 1));
    for _ in 0..600 {
        level.tick();
    }
    assert!(held(&level, id).is_some());
    level.tick();
    assert!(held(&level, id).is_none());
    assert_eq!(fox::fox(level.mob(id).unwrap()).ticks_since_eaten(), 0);
}

#[test]
fn foxes_drop_what_they_hold_when_killed() {
    let (mut level, _player, id) = setup(Vec3::new(0.5, -60.0, 0.5));
    level.set_item_slot(id, EquipmentSlot::Mainhand, ItemStack::new(items::EMERALD, 1));
    level.kill(id);
    let dropped: Vec<_> = level.items_in(&level.entity(id).bounding_box().inflate(2.0, 2.0, 2.0), None).into_iter().filter_map(|item| level.entities[&item].as_item().map(|item| (item.item.get_item(), item.item.count))).collect();
    assert_eq!(dropped, vec![(items::EMERALD, 1)]);
}

#[test]
fn foxes_sleep_under_a_roof_by_day_and_wake_at_night() {
    let mut level = testing::level();
    fill(&mut level, (-2, -60, -2), (2, -59, 2), blocks::STONE);
    fill(&mut level, (-1, -60, -1), (1, -60, 1), blocks::AIR);
    let _player = testing::player(&mut level, Vec3::new(0.5, -60.0, 40.5), 0.0, 0.0);
    set_time(&mut level, 1000);
    let id = add_fox(&mut level, Vec3::new(0.5, -60.0, 0.5));
    let mut slept = false;
    for _ in 0..400 {
        level.tick();
        slept |= level.is_sleeping(id);
    }
    assert!(slept, "the fox never fell asleep");
    assert!(level.is_sleeping(id));
    assert_eq!(level.block_state(BlockPos::new(0, -59, 0)).block(), blocks::STONE);
    set_time(&mut level, 14000);
    for _ in 0..40 {
        level.tick();
    }
    assert!(!level.is_sleeping(id));
}

#[test]
fn foxes_with_hunger_keep_ticking() {
    let (mut level, _, fox) = setup(Vec3::new(0.5, -60.0, 0.5));
    let effect = crate::world::effect::MobEffectInstance::new(wafer_data::mob_effects::HUNGER, 40, 0);
    assert!(level.add_effect(fox, effect, None));
    for _ in 0..20 {
        level.tick();
    }
    assert!(level.has_effect(fox, wafer_data::mob_effects::HUNGER));
}
