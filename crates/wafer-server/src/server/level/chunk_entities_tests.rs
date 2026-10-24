use wafer_core::ChunkPos;
use wafer_protocol::nbt::Nbt;

use super::testing;

fn entity(id: &str, x: f64) -> Nbt {
    Nbt::Compound(vec![
        ("id".into(), Nbt::String(id.into())),
        ("Pos".into(), Nbt::List(vec![Nbt::Double(x), Nbt::Double(-60.0), Nbt::Double(0.5)])),
    ])
}

#[test]
fn legacy_chunk_entities_are_loaded_into_the_level() {
    let mut level = testing::level();
    level.add_legacy_chunk_entities(ChunkPos::new(0, 0), &[entity("minecraft:cow", 0.5), entity("minecraft:nope", 1.5), entity("minecraft:pig", 2.5)]);
    assert_eq!(level.entities.len(), 2);
}

#[test]
fn world_gen_chunk_entities_are_loaded_into_the_level() {
    let mut level = testing::level();
    level.add_world_gen_chunk_entities(ChunkPos::new(0, 0), &[entity("minecraft:sheep", 0.5), entity("minecraft:nope", 1.5)]);
    assert_eq!(level.entities.len(), 1);
}

#[test]
fn a_full_chunk_keeps_its_legacy_entities_until_it_is_loaded() {
    use crate::world::level::chunk::FlatLevelSource;
    use crate::world::level::chunk::serialization;
    use crate::server::level::chunk_level::ChunkStatus;

    let dimension = &wafer_data::DIMENSION_TYPES[0];
    let chunk = FlatLevelSource::default().generate(ChunkPos::new(0, 0), dimension);
    let Nbt::Compound(mut fields) = serialization::write(&chunk, ChunkStatus::Full, Vec::new(), 0, &[], &[], &[]) else { panic!("a compound") };
    fields.retain(|(name, _)| name != "entities");
    fields.push(("entities".into(), Nbt::List(vec![entity("minecraft:cow", 0.5)])));
    let read = serialization::read(&Nbt::Compound(fields), dimension, 0).unwrap();
    assert_eq!(read.chunk.legacy_entities.len(), 1);
}
