use wafer_core::Vec3;
use wafer_data::{EquipmentSlot, damage_types, items, mob_effects};
use wafer_protocol::nbt::Nbt;

use crate::server::level::{ServerLevel, testing};
use crate::world::damagesource::DamageSource;
use crate::world::effect::mob_effect_instance::MobEffectInstance;
use crate::world::entity::data::Pose;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::item::ItemStack;

fn add_mannequin(level: &mut ServerLevel, pos: Vec3, nbt: Nbt) -> EntityId {
    let mut entity = ServerLevel::create_entity("minecraft:mannequin").expect("a mannequin");
    entity.load(&nbt).expect("loads");
    entity.entity_mut().set_pos(pos);
    let id = entity.entity().id;
    level.add_fresh_entity(entity);
    id
}

fn saved(level: &ServerLevel, id: EntityId) -> Vec<(String, Nbt)> {
    let mut out = Vec::new();
    level.save_without_id(id, &mut out);
    out
}

#[test]
fn mannequins_fall_take_damage_and_die() {
    let mut level = testing::level();
    let _player = testing::player(&mut level, Vec3::new(0.5, -60.0, 8.5), 180.0, 0.0);
    let id = add_mannequin(&mut level, Vec3::new(0.5, -55.0, 0.5), Nbt::Compound(Vec::new()));
    for _ in 0..40 {
        level.tick();
    }
    assert_eq!(level.entity(id).y(), -60.0);
    level.set_item_slot(id, EquipmentSlot::Head, ItemStack::new(items::IRON_HELMET, 1));
    assert!(level.add_effect(id, MobEffectInstance::new(mob_effects::SPEED, 100, 0), None));
    assert!(level.hurt_server(id, DamageSource::new(damage_types::GENERIC), 5.0));
    assert_eq!(level.health(id), 13.0);
    assert!(level.hurt_server(id, DamageSource::new(damage_types::GENERIC_KILL), f32::MAX));
    assert_eq!(level.entity(id).pose(), Pose::Dying);
    assert_eq!(level.entity(id).dimensions.height, 0.2);
    for _ in 0..25 {
        level.tick();
    }
    assert!(level.entity_ref(id).is_none_or(|e| e.is_removed()));
    assert!(!level.entities.values().any(|e| matches!(e, LevelEntity::Item(_))));
}

#[test]
fn immovable_mannequins_do_not_fall() {
    let mut level = testing::level();
    let _player = testing::player(&mut level, Vec3::new(0.5, -60.0, 8.5), 180.0, 0.0);
    let id = add_mannequin(&mut level, Vec3::new(0.5, -55.0, 0.5), Nbt::Compound(vec![("immovable".into(), Nbt::Byte(1))]));
    for _ in 0..20 {
        level.tick();
    }
    assert_eq!(level.entity(id).y(), -55.0);
}

#[test]
fn mannequins_save_their_avatar_fields() {
    let mut level = testing::level();
    let fields = vec![
        ("hidden_layers".into(), Nbt::List(vec![Nbt::String("cape".into()), Nbt::String("hat".into())])),
        ("main_hand".into(), Nbt::String("left".into())),
        ("pose".into(), Nbt::String("crouching".into())),
        ("hide_description".into(), Nbt::Byte(1)),
    ];
    let id = add_mannequin(&mut level, Vec3::new(0.5, -60.0, 0.5), Nbt::Compound(fields));
    let out = saved(&level, id);
    let get = |key: &str| out.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone());
    assert_eq!(get("hidden_layers"), Some(Nbt::List(vec![Nbt::String("cape".into()), Nbt::String("hat".into())])));
    assert_eq!(get("main_hand"), Some(Nbt::String("left".into())));
    assert_eq!(get("pose"), Some(Nbt::String("crouching".into())));
    assert_eq!(get("hide_description"), Some(Nbt::Byte(1)));
    assert_eq!(get("profile"), Some(Nbt::Compound(Vec::new())));
    assert!(get("description").is_none());
    assert_eq!(level.entity(id).dimensions.height, 1.5);
}
