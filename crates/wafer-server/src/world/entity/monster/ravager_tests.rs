use wafer_protocol::nbt::Nbt;

use super::ravager::Ravager;
use crate::world::entity::attributes::Attribute;

#[test]
fn ravagers_have_vanillas_attributes_and_save_their_ticks() {
    let mut mob = Ravager::create();
    let attributes = &mob.living.attributes;
    assert_eq!(attributes.value(Attribute::MaxHealth), 100.0);
    assert_eq!(attributes.value(Attribute::KnockbackResistance), 0.75);
    assert_eq!(attributes.value(Attribute::AttackDamage), 12.0);
    assert_eq!(attributes.value(Attribute::AttackKnockback), 1.5);
    assert_eq!(attributes.value(Attribute::StepHeight), 1.0);
    assert_eq!(mob.xp_reward, 20);
    assert_eq!(mob.max_head_y_rot(), 45);
    mob.load(&Nbt::Compound(vec![("StunTick".into(), Nbt::Int(12)), ("RoarTick".into(), Nbt::Int(3))]));
    assert!(Ravager::is_immobile(&mob));
    assert!(!Ravager::has_line_of_sight(&mob));
    let mut out = Vec::new();
    mob.save(&mut out);
    assert!(out.iter().any(|(k, v)| k == "StunTick" && *v == Nbt::Int(12)));
    assert!(out.iter().any(|(k, v)| k == "AttackTick" && *v == Nbt::Int(0)));
}
