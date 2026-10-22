use wafer_core::Vec3;
use wafer_data::items;
use wafer_protocol::nbt::Nbt;

use super::rabbit::{self, Rabbit, Variant};
use crate::server::level::{ServerLevel, SoundSource, testing};
use crate::world::entity::ai::goal::goal::Goal;
use crate::world::entity::attributes::Attribute;
use crate::world::entity::mob::{self, Mob};
use crate::world::entity::spawn_group_data::SpawnGroupData;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::{InteractionHand, InteractionResult};

fn add(level: &mut ServerLevel, mut mob: Mob, pos: Vec3) -> EntityId {
    mob.entity.set_pos(pos);
    let id = mob.entity.id;
    level.add_fresh_entity(LevelEntity::Mob(Box::new(mob)));
    id
}

fn has_goal(mob: &Mob, matches: fn(&Goal) -> bool) -> bool {
    mob.ai.goal_selector.get_available_goals().iter().any(|goal| matches(goal.get_goal())) || mob.ai.target_selector.get_available_goals().iter().any(|goal| matches(goal.get_goal()))
}

#[test]
fn the_evil_variant_is_the_killer_bunny() {
    let mut mob = Rabbit::create();
    assert!(!has_goal(&mob, |goal| matches!(goal, Goal::MeleeAttack(_))));
    rabbit::set_variant(&mut mob, Variant::Evil);
    assert_eq!(rabbit::get_variant(&mob.entity), Variant::Evil);
    assert_eq!(mob.living.attributes.value(Attribute::Armor), 8.0);
    assert_eq!(mob.living.attributes.value(Attribute::AttackDamage), 8.0);
    assert!(matches!(mob.entity.custom_name(), Some(_)));
    assert!(has_goal(&mob, |goal| matches!(goal, Goal::MeleeAttack(_))));
    assert!(has_goal(&mob, |goal| matches!(goal, Goal::HurtByTarget(_))));
    assert_eq!(Rabbit::get_sound_source(&mob), SoundSource::Hostile);
    rabbit::set_variant(&mut mob, Variant::Gold);
    assert_eq!(mob.living.attributes.value(Attribute::AttackDamage), 3.0);
    assert_eq!(Rabbit::get_sound_source(&mob), SoundSource::Neutral);
}

#[test]
fn named_killer_bunnies_keep_their_name() {
    let mut mob = Rabbit::create();
    mob.entity.set_custom_name(Some(Nbt::String("Toast".into())));
    rabbit::set_variant(&mut mob, Variant::Evil);
    assert!(matches!(mob.entity.data.get(crate::world::entity::data::ids::CUSTOM_NAME), crate::world::entity::data::DataValue::OptionalComponent(Some(Nbt::String(name))) if name == "Toast"));
}

#[test]
fn rabbit_types_save_with_their_legacy_ids() {
    let mut mob = Rabbit::create();
    rabbit::set_variant(&mut mob, Variant::WhiteSplotched);
    rabbit::rabbit_mut(&mut mob).more_carrot_ticks = 12;
    let mut out = Vec::new();
    mob.save(&mut out);
    let nbt = Nbt::Compound(out);
    assert_eq!(nbt.get("RabbitType"), Some(&Nbt::Int(3)));
    assert_eq!(nbt.get("MoreCarrotTicks"), Some(&Nbt::Int(12)));
    let mut loaded = Rabbit::create();
    loaded.load(&nbt);
    assert_eq!(rabbit::get_variant(&loaded.entity), Variant::WhiteSplotched);
    assert_eq!(rabbit::rabbit(&loaded).more_carrot_ticks, 12);
    let mut unknown = Rabbit::create();
    unknown.load(&Nbt::Compound(vec![("RabbitType".into(), Nbt::Int(42))]));
    assert_eq!(rabbit::get_variant(&unknown.entity), Variant::Brown);
}

#[test]
fn the_rabbit_variant_component_sets_the_type() {
    let mut mob = Rabbit::create();
    let mut egg = ItemStack::new(items::RABBIT_SPAWN_EGG, 1);
    egg.set(dc::RABBIT_VARIANT, Some(Nbt::String("evil".into())));
    mob.apply_components_from_item_stack(&egg);
    assert_eq!(rabbit::get_variant(&mob.entity), Variant::Evil);
}

#[test]
fn rabbit_groups_share_a_variant_and_bring_babies() {
    let mut level = testing::level();
    let difficulty = level.current_difficulty_at(wafer_core::BlockPos::ZERO);
    let mut first = Rabbit::create();
    let group_data = mob::finalize_spawn(&mut level, &mut first, &difficulty, crate::world::entity::entity_spawn_reason::EntitySpawnReason::ChunkGeneration, None);
    let Some(SpawnGroupData::Rabbit(rabbit_group_data)) = group_data else { panic!("rabbit group data") };
    assert!(!first.is_baby());
    assert_eq!(rabbit::get_variant(&first.entity), rabbit_group_data.variant);
    assert!(matches!(rabbit::get_variant(&first.entity), Variant::Brown | Variant::Salt | Variant::Black));
    let mut second = Rabbit::create();
    mob::finalize_spawn(&mut level, &mut second, &difficulty, crate::world::entity::entity_spawn_reason::EntitySpawnReason::ChunkGeneration, group_data);
    assert!(second.is_baby());
    assert_eq!(rabbit::get_variant(&second.entity), rabbit_group_data.variant);
}

#[test]
fn carrots_put_rabbits_in_love() {
    let mut level = testing::level();
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    let rabbit = add(&mut level, Rabbit::create(), Vec3::new(0.5, -60.0, 2.0));
    level.player_mut(player).set_item_in_hand(InteractionHand::MainHand, Some(ItemStack::new(items::GOLDEN_CARROT, 2)));
    let location = level.entity(rabbit).position();
    assert_eq!(level.interact_on(player, rabbit, InteractionHand::MainHand, location), InteractionResult::SUCCESS_SERVER);
    assert!(level.is_in_love(rabbit));
}

#[test]
fn rabbits_hop_instead_of_walking() {
    let mut level = testing::level();
    let _player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    let mut mob = Rabbit::create();
    mob.entity.random = wafer_core::random::LegacyRandomSource::new(1);
    let rabbit = add(&mut level, mob, Vec3::new(0.5, -60.0, 30.5));
    let mut jumps = 0;
    let mut was_on_ground = true;
    for _ in 0..600 {
        level.tick();
        let on_ground = level.entity(rabbit).on_ground;
        if was_on_ground && !on_ground {
            jumps += 1;
        }
        was_on_ground = on_ground;
    }
    assert!(jumps > 0, "the rabbit never hopped");
    assert!(level.entity(rabbit).position().distance_to(Vec3::new(0.5, -60.0, 30.5)) > 0.5, "the rabbit stayed put");
}
