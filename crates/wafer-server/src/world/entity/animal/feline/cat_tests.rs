use wafer_core::{BlockPos, Vec3};
use wafer_data::{blocks, items};
use wafer_protocol::nbt::Nbt;

use super::cat::{self, Cat};
use super::ocelot::{self, Ocelot};
use crate::server::level::{ServerLevel, testing};
use crate::world::entity::ai::goal::goal::Goal;
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::mob::Mob;
use crate::world::entity::tamable_animal;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::item::dye_color::DyeColor;
use crate::world::level::BlockGetter;
use crate::world::level::update_flags::UPDATE_ALL;
use crate::world::{InteractionHand, InteractionResult};

fn add(level: &mut ServerLevel, mut mob: Mob, pos: Vec3) -> EntityId {
    mob.entity.set_pos(pos);
    let id = mob.entity.id;
    level.add_fresh_entity(LevelEntity::Mob(Box::new(mob)));
    id
}

fn setup() -> (ServerLevel, EntityId, EntityId) {
    let mut level = testing::level();
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    let cat = add(&mut level, Cat::create(), Vec3::new(0.5, -60.0, 2.0));
    (level, player, cat)
}

fn tamed() -> (ServerLevel, EntityId, EntityId) {
    let (mut level, player, cat) = setup();
    level.tame(cat, player);
    (level, player, cat)
}

fn interact(level: &mut ServerLevel, player: EntityId, mob: EntityId, held: ItemStack) -> InteractionResult {
    level.player_mut(player).set_item_in_hand(InteractionHand::MainHand, Some(held));
    let location = level.entity(mob).position();
    level.interact_on(player, mob, InteractionHand::MainHand, location)
}

fn held(level: &ServerLevel, player: EntityId) -> Option<ItemStack> {
    level.player(player).item_in_hand(InteractionHand::MainHand).filter(|stack| !stack.is_empty())
}

fn has_goal(level: &ServerLevel, mob: EntityId, matches: fn(&Goal) -> bool) -> bool {
    level.mob(mob).unwrap().ai.goal_selector.get_available_goals().iter().any(|goal| matches(goal.get_goal()))
}

fn goal_running(level: &ServerLevel, mob: EntityId, matches: fn(&Goal) -> bool) -> bool {
    level.mob(mob).unwrap().ai.goal_selector.get_available_goals().iter().any(|goal| goal.is_running() && matches(goal.get_goal()))
}

#[test]
fn stray_cats_avoid_players_until_tamed() {
    let (mut level, player, cat) = setup();
    let goals = level.mob(cat).unwrap().ai.goal_selector.get_available_goals();
    assert!(matches!(goals.last().map(|goal| (goal.get_priority(), goal.get_goal())), Some((4, Goal::CatAvoidEntity(_)))));
    level.tame(cat, player);
    assert!(!has_goal(&level, cat, |goal| matches!(goal, Goal::CatAvoidEntity(_))));
    level.set_tame(cat, false, true);
    assert!(has_goal(&level, cat, |goal| matches!(goal, Goal::CatAvoidEntity(_))));
}

#[test]
fn fish_tames_stray_cats_one_time_in_three_and_is_used_up() {
    let (mut level, player, cat) = setup();
    let mut tries = 0;
    while !level.is_tame(cat) {
        tries += 1;
        assert!(tries < 100, "the cat never accepted fish");
        assert_eq!(interact(&mut level, player, cat, ItemStack::new(items::COD, 1)), InteractionResult::SUCCESS);
        assert!(held(&level, player).is_none());
        assert!(level.mob(cat).unwrap().persistence_required);
    }
    assert_eq!(level.get_owner(cat), Some(player));
    assert!(level.is_ordered_to_sit(cat));
    assert_eq!(level.max_health(cat), 10.0);
    assert!(!has_goal(&level, cat, |goal| matches!(goal, Goal::CatAvoidEntity(_))));
}

#[test]
fn owners_toggle_sitting_and_dye_collars() {
    let (mut level, player, cat) = tamed();
    assert_eq!(cat::get_collar_color(level.entity(cat)).name, "red");
    assert_eq!(interact(&mut level, player, cat, ItemStack::new(items::STICK, 1)), InteractionResult::SUCCESS);
    assert!(level.is_ordered_to_sit(cat));
    assert_eq!(interact(&mut level, player, cat, ItemStack::new(items::STICK, 1)), InteractionResult::SUCCESS);
    assert!(!level.is_ordered_to_sit(cat));
    assert_eq!(interact(&mut level, player, cat, ItemStack::new(items::BLUE_DYE, 2)), InteractionResult::SUCCESS);
    assert_eq!(cat::get_collar_color(level.entity(cat)).name, "blue");
    assert_eq!(held(&level, player).map(|stack| stack.count), Some(1));
    assert!(!level.is_ordered_to_sit(cat));
    assert_eq!(interact(&mut level, player, cat, ItemStack::new(items::BLUE_DYE, 1)), InteractionResult::SUCCESS);
    assert!(level.is_ordered_to_sit(cat));
    assert_eq!(held(&level, player).map(|stack| stack.count), Some(1));
}

#[test]
fn feeding_heals_by_the_nutrition() {
    let (mut level, player, cat) = tamed();
    level.set_health(cat, 5.0);
    assert_eq!(interact(&mut level, player, cat, ItemStack::new(items::COD, 2)), InteractionResult::SUCCESS);
    assert_eq!(level.health(cat), 7.0);
    assert_eq!(held(&level, player).map(|stack| stack.count), Some(1));
}

#[test]
fn tamed_cats_save_and_load_like_vanilla() {
    let (mut level, player, cat) = tamed();
    cat::set_collar_color(level.entity_mut(cat), DyeColor::VALUES[11]);
    level.set_ordered_to_sit(cat, true);
    let mut out = Vec::new();
    level.save_without_id(cat, &mut out);
    let get = |key: &str| out.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone());
    assert_eq!(get("CollarColor"), Some(Nbt::Byte(11)));
    assert_eq!(get("variant"), Some(Nbt::String("minecraft:black".into())));
    assert_eq!(get("sound_variant"), Some(Nbt::String("minecraft:classic".into())));
    assert_eq!(get("Sitting"), Some(Nbt::Byte(1)));
    assert!(get("Owner").is_some());
    let keys: Vec<&str> = out.iter().map(|(k, _)| k.as_str()).collect();
    let tail = &keys[keys.iter().position(|&k| k == "Owner").unwrap()..];
    assert_eq!(tail, ["Owner", "Sitting", "variant", "sound_variant", "CollarColor"]);
    let mut loaded = ServerLevel::create_entity("minecraft:cat").unwrap();
    loaded.load(&Nbt::Compound(out.clone())).unwrap();
    let LevelEntity::Mob(mob) = &loaded else { unreachable!() };
    assert!(tamable_animal::is_tame(&mob.entity));
    assert!(tamable_animal::is_in_sitting_pose(&mob.entity));
    assert_eq!(tamable_animal::get_owner_reference(&mob.entity).map(|owner| owner.get_uuid()), Some(level.entity(player).uuid));
    assert_eq!(cat::get_collar_color(&mob.entity).id, 11);
    assert!(!mob.ai.goal_selector.get_available_goals().iter().any(|goal| matches!(goal.get_goal(), Goal::CatAvoidEntity(_))));
}

#[test]
fn kittens_of_tamed_cats_belong_to_the_owner() {
    let (mut level, player, cat) = tamed();
    let partner = add(&mut level, Cat::create(), Vec3::new(1.5, -60.0, 2.0));
    level.set_in_love(cat, Some(player));
    level.set_in_love(partner, Some(player));
    assert!(!level.can_mate(cat, partner));
    level.tame(partner, player);
    assert!(level.can_mate(cat, partner));
    let kitten = level.get_breed_offspring(cat, partner).unwrap();
    assert!(tamable_animal::is_tame(&kitten.entity));
    assert_eq!(tamable_animal::get_owner_reference(&kitten.entity).map(|owner| owner.get_uuid()), Some(level.entity(player).uuid));
    assert!(!kitten.ai.goal_selector.get_available_goals().iter().any(|goal| matches!(goal.get_goal(), Goal::CatAvoidEntity(_))));
}

#[test]
fn creepers_flee_cats() {
    let (mut level, player, cat) = setup();
    level.player_mut(player).game_mode.game_type = crate::server::level::game_mode::GameType::Creative;
    level.mob_mut(cat).unwrap().set_no_ai(true);
    let creeper = ServerLevel::create_entity("minecraft:creeper").unwrap();
    let creeper_id = creeper.entity().id;
    level.add_fresh_entity(creeper);
    level.entity_mut(creeper_id).set_pos(Vec3::new(3.5, -60.0, 2.0));
    let mut fled = false;
    for _ in 0..200 {
        level.tick();
        fled |= goal_running(&level, creeper_id, |goal| matches!(goal, Goal::AvoidEntity(_)));
    }
    assert!(fled, "the creeper never avoided the cat");
}

#[test]
fn tamed_cats_sit_on_closed_chests_and_block_them() {
    let (mut level, player, cat) = tamed();
    level.set_ordered_to_sit(cat, false);
    let chest = BlockPos::new(3, -60, 2);
    level.set_block(chest, blocks::CHEST.default_state(), UPDATE_ALL);
    level.entity_mut(player).set_pos(Vec3::new(0.5, -60.0, 4.5));
    let mut sat = false;
    for _ in 0..200 {
        level.tick();
        if level.is_in_sitting_pose(cat) && level.entity(cat).block_position() == chest {
            sat = true;
            break;
        }
    }
    assert!(sat, "the cat never sat on the chest, it is at {:?}", level.entity(cat).position());
    assert!(goal_running(&level, cat, |goal| matches!(goal, Goal::CatSitOnBlock(_))));
    let state = level.block_state(chest);
    assert!(crate::world::level::block::chest_block::get_container(state, &mut level, chest, false).is_none());
}

#[test]
fn tamed_cats_lie_down_on_their_sleeping_owners_bed() {
    let (mut level, player, cat) = tamed();
    level.set_ordered_to_sit(cat, false);
    let head = BlockPos::new(0, -60, 5);
    let bed = blocks::RED_BED.default_state().set(wafer_data::properties::HORIZONTAL_FACING, wafer_core::Direction::North);
    level.set_block(head.south(), bed, UPDATE_ALL);
    level.set_block(head, bed.set(wafer_data::properties::BED_PART, wafer_data::properties::BedPart::Head), UPDATE_ALL);
    assert!(level.start_sleeping(player, head));
    let mut lying = false;
    for _ in 0..200 {
        level.tick();
        if level.is_lying(cat) {
            lying = true;
            break;
        }
    }
    assert!(lying, "the cat never lay down, it is at {:?}", level.entity(cat).position());
    assert!(goal_running(&level, cat, |goal| matches!(goal, Goal::CatRelaxOnOwner(_))));
    level.stop_sleeping(player);
    for _ in 0..4 {
        level.tick();
    }
    assert!(!level.is_lying(cat));
    assert!(!goal_running(&level, cat, |goal| matches!(goal, Goal::CatRelaxOnOwner(_))));
}

#[test]
fn ocelots_learn_to_trust_players_who_hold_still_with_fish() {
    let mut level = testing::level();
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    let ocelot = add(&mut level, Ocelot::create(), Vec3::new(0.5, -60.0, 2.0));
    assert_eq!(interact(&mut level, player, ocelot, ItemStack::new(items::COD, 64)), InteractionResult::SUCCESS_SERVER);
    assert!(level.is_in_love(ocelot));
    let mut tries = 0;
    while !ocelot::is_trusting(level.entity(ocelot)) {
        tries += 1;
        assert!(tries < 400, "the ocelot never trusted the player");
        level.tick();
        if goal_running(&level, ocelot, |goal| matches!(goal, Goal::OcelotTempt(_))) && level.entity(player).distance_to_sqr(level.entity(ocelot)) < 9.0 {
            let count = held(&level, player).map_or(0, |stack| stack.count);
            assert_eq!(interact(&mut level, player, ocelot, ItemStack::new(items::COD, count)), InteractionResult::SUCCESS);
            assert_eq!(held(&level, player).map_or(0, |stack| stack.count), count - 1);
        }
    }
    assert!(!has_goal(&level, ocelot, |goal| matches!(goal, Goal::OcelotAvoidEntity(_))));
    let mut out = Vec::new();
    level.save_without_id(ocelot, &mut out);
    assert_eq!(out.iter().find(|(k, _)| k == "Trusting").map(|(_, v)| v.clone()), Some(Nbt::Byte(1)));
    let mut loaded = ServerLevel::create_entity("minecraft:ocelot").unwrap();
    loaded.load(&Nbt::Compound(out)).unwrap();
    let LevelEntity::Mob(mob) = &loaded else { unreachable!() };
    assert!(ocelot::is_trusting(&mob.entity));
    assert!(!mob.ai.goal_selector.get_available_goals().iter().any(|goal| matches!(goal.get_goal(), Goal::OcelotAvoidEntity(_))));
}

#[test]
fn ocelot_groups_spawn_with_babies_after_the_first() {
    let mut level = testing::level();
    let mut group_data = None;
    for i in 0..3 {
        let mut mob = Ocelot::create();
        mob.entity.set_pos(Vec3::new(0.5, -60.0, 0.5));
        let difficulty = level.current_difficulty_at(mob.entity.block_position());
        let (entity, next) = level.finalize_spawn_detached(LevelEntity::Mob(Box::new(mob)), &difficulty, EntitySpawnReason::Natural, group_data);
        group_data = next;
        let LevelEntity::Mob(mob) = entity else { unreachable!() };
        assert_eq!(mob.is_baby(), i > 0);
    }
}
