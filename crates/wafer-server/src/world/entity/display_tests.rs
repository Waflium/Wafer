use wafer_protocol::nbt::Nbt;

use crate::world::entity::display::Display;

fn floats(values: &[f32]) -> Nbt {
    Nbt::List(values.iter().map(|&v| Nbt::Float(v)).collect())
}

fn saved(display: &Display, key: &str) -> Nbt {
    let mut out = Vec::new();
    display.add_additional_save_data(&mut out);
    out.into_iter().find(|(k, _)| k == key).map(|(_, v)| v).unwrap()
}

#[test]
fn display_transformations_decompose_like_vanilla() {
    let mut display = Display::create_item_display();
    let matrix = [1.2, 0.3, -0.7, 0.1, 0.4, 0.9, 0.2, -1.0, -0.6, 0.1, 1.5, 0.25, 0.0, 0.0, 0.0, 2.0];
    display.read_additional_save_data(&Nbt::Compound(vec![("transformation".into(), floats(&matrix))].into()));
    let transformation = saved(&display, "transformation");
    assert_eq!(transformation.get("translation"), Some(&floats(&[0.05, -0.5, 0.125])));
    assert_eq!(transformation.get("left_rotation"), Some(&floats(&[-0.122892976, -0.32860938, 0.45001304, 0.8212196])));
    assert_eq!(transformation.get("scale"), Some(&floats(&[0.58528256, 0.20518348, 1.0138212])));
    assert_eq!(transformation.get("right_rotation"), Some(&floats(&[0.098285004, 0.32231143, -0.41410065, 0.84556264])));

    let mut display = Display::create_block_display();
    let decomposed = Nbt::Compound(vec![
        ("translation".into(), floats(&[0.5, 0.0, -0.5])),
        ("left_rotation".into(), Nbt::Compound(vec![("angle".into(), Nbt::Float(0.7854)), ("axis".into(), floats(&[0.0, 1.0, 0.0]))].into())),
        ("scale".into(), floats(&[2.0, 1.0, 1.0])),
        ("right_rotation".into(), floats(&[0.0, 0.0, 1.0, 1.0])),
    ].into());
    display.read_additional_save_data(&Nbt::Compound(vec![("transformation".into(), decomposed), ("teleport_duration".into(), Nbt::Int(100))].into()));
    let transformation = saved(&display, "transformation");
    assert_eq!(transformation.get("left_rotation"), Some(&floats(&[0.0, 0.38268426, 0.0, 0.9238792])));
    assert_eq!(transformation.get("right_rotation"), Some(&floats(&[0.0, 0.0, 0.70710677, 0.70710677])));
    assert_eq!(saved(&display, "teleport_duration"), Nbt::Int(59));
}

#[test]
fn text_displays_resolve_their_text_while_loading() {
    use crate::network::chat::Component;
    use crate::server::level::testing;
    use crate::world::entity::LevelEntity;
    use crate::world::item::component::custom::parse_snbt;
    use crate::world::scores::criteria::{ObjectiveCriteria, RenderType};
    use crate::world::scores::score_holder::ScoreHolder;

    let mut level = testing::level();
    let objective = level.scoreboard.borrow_mut().add_objective("o", ObjectiveCriteria::DUMMY, Component::literal("O"), RenderType::Integer, false, None);
    let mut score = level.scoreboard.borrow_mut().get_or_create_player_score(&ScoreHolder::for_name_only("Fred"), objective, false);
    score.set(&mut level, 7).unwrap();

    let nbt = parse_snbt(r#"{id:"minecraft:text_display",text:{score:{name:"Fred",objective:"o"}}}"#).unwrap();
    let LevelEntity::Display(display) = level.load_entity(&nbt).unwrap() else { unreachable!() };
    assert_eq!(saved(&display, "text"), Nbt::String("7".into()));
}
