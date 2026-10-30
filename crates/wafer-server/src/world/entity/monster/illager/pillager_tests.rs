use wafer_core::Vec3;
use wafer_data::{EquipmentSlot, items};
use wafer_protocol::nbt::Nbt;

use super::pillager::{self, Pillager};
use crate::server::level::{ServerLevel, testing};
use crate::world::difficulty::Difficulty;
use crate::world::entity::ai::goal::goal::Goal;
use crate::world::entity::attributes::Attribute;
use crate::world::entity::data::DataValue;
use crate::world::entity::mob::Mob;
use crate::world::entity::projectile::projectile::ProjectileKind;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;

fn add(level: &mut ServerLevel, mut mob: Mob, pos: Vec3) -> EntityId {
    mob.entity.snap_to(pos.x, pos.y, pos.z, 0.0, 0.0);
    let id = mob.entity.id;
    level.add_fresh_entity(LevelEntity::Mob(Box::new(mob)));
    id
}

fn arrows(level: &ServerLevel) -> Vec<EntityId> {
    level.entities.iter().filter(|(_, e)| matches!(e, LevelEntity::Projectile(p) if matches!(p.kind, ProjectileKind::Arrow(_)))).map(|(&id, _)| id).collect()
}

#[test]
fn pillagers_have_vanillas_attributes_and_goals() {
    let mob = Pillager::create();
    let attributes = &mob.living.attributes;
    assert_eq!(attributes.value(Attribute::MaxHealth), 24.0);
    assert_eq!(attributes.value(Attribute::MovementSpeed), 0.35f32 as f64);
    assert_eq!(attributes.value(Attribute::FollowRange), 32.0);
    assert_eq!(attributes.value(Attribute::AttackDamage), 5.0);
    assert_eq!(mob.get_max_spawn_cluster_size(), 1);
    let priorities: Vec<i32> = mob.ai.goal_selector.get_available_goals().iter().map(|wrapped| wrapped.get_priority()).collect();
    assert_eq!(priorities, vec![4, 1, 3, 4, 5, 0, 1, 2, 3, 8, 9, 10]);
    assert!(matches!(mob.ai.goal_selector.get_available_goals()[8].get_goal(), Goal::RangedCrossbowAttack(_)));
}

#[test]
fn pillagers_charge_their_crossbow_and_shoot_players() {
    let mut level = testing::level();
    level.difficulty = Difficulty::Normal;
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    level.player_mut(player).movement.client_loaded_timeout_timer = 0;
    let mut mob = Pillager::create();
    mob.equipment[EquipmentSlot::Mainhand as usize] = Some(ItemStack::new(items::CROSSBOW, 1));
    let pillager = add(&mut level, mob, Vec3::new(6.5, -60.0, 0.5));
    let mut charging_seen = false;
    for _ in 0..200 {
        level.tick();
        charging_seen |= matches!(level.entity(pillager).data.get(pillager::IS_CHARGING_CROSSBOW), DataValue::Boolean(true));
        let found = arrows(&level);
        if !found.is_empty() {
            assert!(charging_seen);
            assert_eq!(level.projectile_get_owner(found[0]), Some(pillager));
            let crossbow = level.item_by_slot(pillager, EquipmentSlot::Mainhand).unwrap();
            assert!(crossbow.get(dc::CHARGED_PROJECTILES).is_none_or(|charged| charged.as_list().is_empty()));
            return;
        }
    }
    panic!("the pillager never shot");
}

#[test]
fn pillager_inventories_are_saved() {
    let mut mob = Pillager::create();
    pillager::pillager_mut(&mut mob).inventory.items[0] = ItemStack::new(items::DIAMOND, 3);
    let mut out = Vec::new();
    mob.save(&mut out);
    let mut loaded = Pillager::create();
    loaded.load(&Nbt::Compound(out.into()));
    assert_eq!(pillager::pillager(&loaded).inventory.items[0], ItemStack::new(items::DIAMOND, 3));
    assert!(loaded.can_pick_up_loot);
}
