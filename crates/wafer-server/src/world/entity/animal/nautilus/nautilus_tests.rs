use wafer_core::Vec3;
use wafer_data::{blocks, mob_effects};

use crate::server::level::{ServerLevel, testing};
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type;

fn add(level: &mut ServerLevel, name: &str, pos: Vec3) -> EntityId {
    let mut entity = ServerLevel::create_entity(name).expect("a nautilus");
    entity.entity_mut().set_pos(pos);
    let id = entity.entity().id;
    level.add_fresh_entity(entity);
    id
}

fn flood(level: &mut ServerLevel) {
    for x in -4..=4 {
        for z in -4..=4 {
            for y in -60..=-56 {
                level.set_block(wafer_core::BlockPos::new(x, y, z), blocks::WATER.default_state(), 3);
            }
        }
    }
}

#[test]
fn nautiluses_dry_out_on_land() {
    let mut level = testing::level();
    let _player = testing::player(&mut level, Vec3::new(0.5, -60.0, 8.5), 180.0, 0.0);
    let id = add(&mut level, "minecraft:nautilus", Vec3::new(0.5, -60.0, 0.5));
    for _ in 0..321 {
        level.tick();
    }
    assert!(level.health(id) < 15.0);
}

#[test]
fn nautiluses_breathe_under_water() {
    let mut level = testing::level();
    let _player = testing::player(&mut level, Vec3::new(0.5, -60.0, 8.5), 180.0, 0.0);
    flood(&mut level);
    let id = add(&mut level, "minecraft:nautilus", Vec3::new(0.5, -58.0, 0.5));
    for _ in 0..400 {
        level.tick();
    }
    assert_eq!(level.health(id), 15.0);
    assert_eq!(level.entity(id).air_supply(), 300);
}

#[test]
fn zombie_nautiluses_save_their_variant_and_ignore_poison() {
    let mut level = testing::level();
    let id = add(&mut level, "minecraft:zombie_nautilus", Vec3::new(0.5, -60.0, 0.5));
    let mut out = Vec::new();
    level.save_without_id(id, &mut out);
    let get = |key: &str| out.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone());
    assert_eq!(get("variant"), Some(wafer_protocol::nbt::Nbt::String("minecraft:temperate".into())));
    assert!(!level.add_effect(id, crate::world::effect::mob_effect_instance::MobEffectInstance::new(mob_effects::POISON, 100, 0), None));
    level.mob_mut(id).unwrap().set_baby(true);
    assert!(!level.is_baby(id));
}

#[test]
fn nautiluses_get_angry_at_attackers() {
    let mut level = testing::level();
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 2.5), 180.0, 0.0);
    let id = add(&mut level, "minecraft:nautilus", Vec3::new(0.5, -60.0, 0.5));
    level.tick();
    let source = crate::world::damagesource::DamageSource { causing_entity: Some(player), direct_entity: Some(player), ..crate::world::damagesource::DamageSource::new(wafer_data::damage_types::PLAYER_ATTACK) };
    assert!(level.hurt_server(id, source, 1.0));
    assert!(level.get_brain(id).has_memory_value(memory_module_type::ANGRY_AT));
}

#[test]
fn nautiluses_are_tamed_with_pufferfish_and_ridden() {
    use crate::world::InteractionHand;
    use crate::world::item::ItemStack;
    use wafer_data::items;
    let mut level = testing::level();
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    let id = add(&mut level, "minecraft:nautilus", Vec3::new(0.5, -60.0, 2.5));
    let mut tries = 0;
    while !level.is_tame(id) {
        level.player_mut(player).set_item_in_hand(InteractionHand::MainHand, Some(ItemStack::new(items::PUFFERFISH_BUCKET, 1)));
        let location = level.entity(id).position();
        level.interact_on(player, id, InteractionHand::MainHand, location);
        assert!(level.player(player).item_in_hand(InteractionHand::MainHand).is_some_and(|stack| stack.is(items::WATER_BUCKET)));
        tries += 1;
        assert!(tries < 100);
    }
    assert!(level.is_owned_by(id, player));
    level.player_mut(player).set_item_in_hand(InteractionHand::MainHand, None);
    let location = level.entity(id).position();
    level.interact_on(player, id, InteractionHand::MainHand, location);
    assert_eq!(level.vehicle(player), Some(id));
    level.tick();
    assert!(level.living(player).has_effect(mob_effects::BREATH_OF_THE_NAUTILUS));
}
