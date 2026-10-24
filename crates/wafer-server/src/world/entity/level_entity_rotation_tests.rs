use super::LevelEntity;
use super::decoration::armor_stand::ArmorStand;
use super::monster::phantom::Phantom;
use super::monster::silverfish::Silverfish;

#[test]
fn body_and_head_rotation_follow_vanilla() {
    let mut phantom = LevelEntity::Mob(Box::new(Phantom::create()));
    let y_rot = phantom.entity().y_rot;
    phantom.set_y_body_rot(30.0);
    phantom.set_y_head_rot(40.0);
    let LevelEntity::Mob(phantom) = phantom else { unreachable!() };
    assert_eq!((phantom.living.y_body_rot, phantom.entity.y_head_rot, phantom.entity.y_rot), (30.0, 40.0, y_rot));

    let mut silverfish = LevelEntity::Mob(Box::new(Silverfish::create()));
    silverfish.set_y_body_rot(50.0);
    let LevelEntity::Mob(silverfish) = silverfish else { unreachable!() };
    assert_eq!((silverfish.living.y_body_rot, silverfish.entity.y_rot), (50.0, 50.0));

    let mut stand = LevelEntity::ArmorStand(Box::new(ArmorStand::of_type()));
    stand.set_y_body_rot(60.0);
    stand.set_y_head_rot(70.0);
    let LevelEntity::ArmorStand(stand) = stand else { unreachable!() };
    assert_eq!((stand.entity.y_head_rot, stand.living.y_head_rot_o), (70.0, 70.0));
}
