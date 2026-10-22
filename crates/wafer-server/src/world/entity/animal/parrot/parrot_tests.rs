use wafer_core::Vec3;
use wafer_data::damage_types;

use super::parrot::{self, Parrot, Variant};
use crate::server::level::server_player::is_empty_compound;
use crate::server::level::{ServerLevel, testing};
use crate::world::damagesource::DamageSource;
use crate::world::entity::data::{DataValue, ids};
use crate::world::entity::entity_reference::EntityReference;
use crate::world::entity::tamable_animal;
use crate::world::entity::{EntityId, LevelEntity};

fn add_parrot(level: &mut ServerLevel, pos: Vec3, owner: Option<EntityId>) -> EntityId {
    let mut mob = Parrot::create();
    mob.entity.snap_to(pos.x, pos.y, pos.z, 0.0, 0.0);
    mob.persistence_required = true;
    parrot::set_variant(&mut mob.entity, Variant::YellowBlue);
    if let Some(owner) = owner {
        tamable_animal::set_owner_reference(&mut mob.entity, Some(EntityReference::of(level.entity(owner).uuid)));
        tamable_animal::set_tame(&mut mob, true, false);
    }
    let id = mob.entity.id;
    level.add_fresh_entity(LevelEntity::Mob(Box::new(mob)));
    id
}

#[test]
fn tamed_parrots_land_on_their_owners_shoulder_and_jump_off_when_hurt() {
    let mut level = testing::level();
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    level.entity_mut(player).on_ground = true;
    let parrot = add_parrot(&mut level, Vec3::new(0.5, -60.0, 0.5), Some(player));
    let mut landed = false;
    for _ in 0..4000 {
        level.entity_mut(player).on_ground = true;
        level.tick();
        if level.entity_ref(parrot).is_none_or(|e| e.is_removed()) {
            landed = true;
            break;
        }
    }
    assert!(landed, "the parrot never landed on its owner");
    let p = level.player(player);
    assert_eq!(p.shoulder_entity_left.get("id").and_then(wafer_protocol::nbt::Nbt::as_str), Some("minecraft:parrot"));
    assert_eq!(*p.entity.data.get(ids::PLAYER_SHOULDER_PARROT_LEFT), DataValue::OptionalUnsignedInt(Some(Variant::YellowBlue as u32)));
    for _ in 0..25 {
        level.entity_mut(player).on_ground = true;
        level.tick();
    }
    level.hurt(player, DamageSource::new(damage_types::GENERIC), 1.0);
    assert!(is_empty_compound(&level.player(player).shoulder_entity_left));
    assert_eq!(*level.player(player).entity.data.get(ids::PLAYER_SHOULDER_PARROT_LEFT), DataValue::OptionalUnsignedInt(None));
    assert!(level.entities.values().any(|e| matches!(e, LevelEntity::Mob(mob) if level.is_parrot(mob.entity.id) && parrot::get_variant(&mob.entity) == Variant::YellowBlue)));
}

#[test]
fn parrots_save_their_variant_but_no_age() {
    let mut level = testing::level();
    let _player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    let parrot = add_parrot(&mut level, Vec3::new(4.5, -60.0, 4.5), None);
    let mut out = Vec::new();
    level.save_without_id(parrot, &mut out);
    assert!(out.iter().any(|(key, value)| key == "Variant" && *value == wafer_protocol::nbt::Nbt::Int(3)));
    assert!(!out.iter().any(|(key, _)| key == "Age" || key == "AgeLocked" || key == "ForcedAge"));
}

#[test]
fn parrots_fly_around() {
    let mut level = testing::level();
    let _player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    let parrot = add_parrot(&mut level, Vec3::new(4.5, -60.0, 4.5), None);
    let mut flew = false;
    for _ in 0..600 {
        level.tick();
        flew |= level.entity(parrot).y() > -59.0;
    }
    assert!(flew, "the parrot never took off");
}
