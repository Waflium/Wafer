use serde_json::Value;

use super::structure_processor::StructureProcessorListHolder;
use super::structure_template_manager;

#[test]
fn every_processor_list_parses() {
    for (name, _) in wafer_data::PROCESSOR_LISTS {
        StructureProcessorListHolder::parse(&Value::String((*name).to_owned())).unwrap().value();
    }
}

#[test]
fn every_structure_template_loads() {
    let templates = structure_template_manager::list_templates();
    assert_eq!(templates.len(), crate::world::item::crafting::tests::vanilla_data_files_with_extension("structure", ".nbt"));
    for name in templates {
        let size = structure_template_manager::get_or_create(&name).get_size();
        assert!(size.x >= 1 && size.y >= 1 && size.z >= 1, "{name} has size {size:?}");
    }
}

mod template_entities {
    use wafer_core::ChunkPos;
    use wafer_protocol::nbt::Nbt;

    use crate::server::level::testing;
    use crate::world::entity::{LevelEntity, sound_event};
    use crate::world::level::levelgen::structure::templatesystem::structure_template::create_entity_ignore_exception;

    const WOLF_TARGET: u128 = 0x0000_0001_0000_0002_0000_0003_0000_0004;

    fn wolf_tag() -> Nbt {
        Nbt::Compound(vec![
            ("id".into(), Nbt::String("minecraft:wolf".into())),
            ("AngerTime".into(), Nbt::Int(100)),
            ("angry_at".into(), Nbt::IntArray(vec![1, 2, 3, 4])),
        ])
    }

    fn item_frame_tag() -> Nbt {
        Nbt::Compound(vec![
            ("id".into(), Nbt::String("minecraft:item_frame".into())),
            ("Item".into(), Nbt::Compound(vec![("id".into(), Nbt::String("minecraft:stone".into())), ("count".into(), Nbt::Int(1))])),
        ])
    }

    fn anger(entity: &LevelEntity) -> (i64, Option<u128>) {
        let LevelEntity::Mob(mob) = entity else { unreachable!("a wolf") };
        let neutral_mob = mob.neutral_mob().expect("a neutral mob");
        (neutral_mob.get_persistent_anger_end_time(&mob.entity), neutral_mob.get_persistent_anger_target().map(|target| target.get_uuid()))
    }

    #[test]
    fn legacy_anger_time_and_target_are_applied_in_a_level() {
        let mut level = testing::level();
        level.game_time = 40;
        let entity = create_entity_ignore_exception(&mut level, &wolf_tag()).expect("a wolf");
        assert_eq!(anger(&entity), (140, Some(WOLF_TARGET)));
    }

    #[test]
    fn legacy_anger_time_and_target_are_applied_in_a_region() {
        let mut level = testing::level();
        level.chunks.region_level_data.game_time = 40;
        let entity = level.chunks.with_region(ChunkPos::new(0, 0), |region, _| create_entity_ignore_exception(region, &wolf_tag())).expect("a wolf");
        assert_eq!(anger(&entity), (140, Some(WOLF_TARGET)));
    }

    #[test]
    fn framed_items_play_their_sound_in_a_region() {
        let mut level = testing::level();
        let entity = level.chunks.with_region(ChunkPos::new(0, 0), |region, _| create_entity_ignore_exception(region, &item_frame_tag()));
        assert!(entity.is_some());
        let sounds = level.chunks.take_world_gen_entity_sounds();
        assert_eq!(sounds.len(), 1);
        assert_eq!(sounds[0].1, sound_event("minecraft:entity.item_frame.add_item"));
    }
}
