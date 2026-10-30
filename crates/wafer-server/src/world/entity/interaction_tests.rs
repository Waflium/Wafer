use wafer_core::Vec3;
use wafer_protocol::nbt::Nbt;

use crate::server::level::testing;
use crate::world::entity::LevelEntity;
use crate::world::entity::interaction::Interaction;

#[test]
fn interactions_record_attacks_and_interactions() {
    let mut level = testing::level();
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    let mut interaction = Interaction::create();
    interaction.read_additional_save_data(&Nbt::Compound(vec![("width".into(), Nbt::Float(2.0)), ("height".into(), Nbt::Float(3.0))].into()));
    interaction.entity.snap_to(0.5, -60.0, 2.5, 0.0, 0.0);
    let id = interaction.entity.id;
    level.add_fresh_entity(LevelEntity::Interaction(interaction));
    assert_eq!(level.entity(id).bounding_box().max_y - level.entity(id).bounding_box().min_y, 3.0);
    assert_eq!(level.get_last_attacker(id), None);
    assert!(level.skip_attack_interaction(id, player));
    assert_eq!(level.get_last_attacker(id), Some(player));
    assert_eq!(level.targeting_get_target(id), None);
    assert!(level.interaction_interact(id, player).consumes_action());
    assert_eq!(level.targeting_get_target(id), Some(player));
    let Some(LevelEntity::Interaction(interaction)) = level.entities.get_mut(&id) else { unreachable!() };
    let mut out = Vec::new();
    interaction.add_additional_save_data(&mut out);
    assert!(out.iter().any(|(k, v)| k == "width" && *v == Nbt::Float(2.0)));
    assert!(out.iter().any(|(k, _)| k == "attack"));
    assert!(out.iter().any(|(k, _)| k == "interaction"));
    interaction.read_additional_save_data(&Nbt::Compound(vec![("response".into(), Nbt::Byte(1))].into()));
    assert_eq!(level.get_last_attacker(id), None);
    assert!(!level.skip_attack_interaction(id, player));
    assert_eq!(level.get_last_attacker(id), Some(player));
}
