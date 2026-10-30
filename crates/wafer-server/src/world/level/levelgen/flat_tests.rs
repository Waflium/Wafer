use wafer_core::ChunkPos;
use wafer_protocol::nbt::Nbt;

use super::flat::decode_settings;
use crate::server::level::chunk_level::ChunkStatus;
use crate::world::item::component::element_to_nbt;
use crate::world::level::biome::inline_biome;
use crate::world::level::chunk::unseeded_chunk_generator::UnseededChunkGenerator;
use crate::world::level::chunk::{Biome, FlatLevelSource, serialization};

fn biome_tag(name: &str) -> Nbt {
    let source = crate::core::data_registries::json_entries("minecraft:worldgen/biome").iter().find(|(entry, _)| *entry == name).expect("a biome").1;
    element_to_nbt(&crate::util::gson::parse_strict(source).expect("biome json"))
}

fn settings(biome: Nbt) -> Nbt {
    let layer = |block: &str, height: i32| Nbt::Compound(vec![("block".into(), Nbt::String(block.into())), ("height".into(), Nbt::Int(height))].into());
    Nbt::Compound(vec![("layers".into(), Nbt::List(vec![layer("minecraft:bedrock", 1), layer("minecraft:grass_block", 1)])), ("biome".into(), biome)].into())
}

fn decode(biome: Nbt) -> FlatLevelSource {
    crate::nbt::with_json_source(|| decode_settings(&settings(biome))).value.expect("flat settings")
}

#[test]
fn an_inline_biome_object_is_accepted_like_a_biome_id() {
    let flat = decode(biome_tag("minecraft:snowy_plains"));
    assert!(flat.biome.is_inline());
    assert_ne!(flat.biome, Biome::by_name("minecraft:snowy_plains").unwrap());
    assert_eq!(flat.biome.name(), "[unregistered]");
    assert!(!flat.biome.is("minecraft:is_overworld"));
    assert_eq!(decode(biome_tag("minecraft:snowy_plains")).biome, flat.biome);
    assert_ne!(decode(biome_tag("minecraft:plains")).biome, flat.biome);
}

#[test]
fn an_inline_biome_keeps_its_own_climate_and_features() {
    let snowy = decode(biome_tag("minecraft:snowy_plains")).biome;
    let pos = wafer_core::BlockPos::new(0, 64, 0);
    assert_eq!(snowy.precipitation_at(pos, 63), Biome::by_name("minecraft:snowy_plains").unwrap().precipitation_at(pos, 63));
    assert_eq!(
        crate::world::level::biome::biome_generation_settings::get(snowy).features(),
        crate::world::level::biome::biome_generation_settings::get(Biome::by_name("minecraft:snowy_plains").unwrap()).features()
    );
    assert_eq!(inline_biome::get(snowy).tag, biome_tag("minecraft:snowy_plains"));
}

#[test]
fn an_invalid_inline_biome_falls_back_to_plains() {
    let broken = Nbt::Compound(vec![("temperature".into(), Nbt::Float(1.0))].into());
    assert_eq!(decode(broken).biome, Biome::by_name("minecraft:plains").unwrap());
}

#[test]
fn an_inline_biome_is_encoded_as_an_object() {
    let encoded = UnseededChunkGenerator::Flat(decode(biome_tag("minecraft:desert"))).encode();
    assert_eq!(encoded.get("settings").and_then(|settings| settings.get("biome")), Some(&biome_tag("minecraft:desert")));
}

#[test]
fn chunks_with_an_inline_biome_cannot_be_saved() {
    let flat = decode(biome_tag("minecraft:plains"));
    let chunk = flat.generate(ChunkPos::new(0, 0), &wafer_data::DIMENSION_TYPES[0]);
    let thrown = crate::util::catch_throwable(|| serialization::write(&chunk, ChunkStatus::Full, Vec::new(), 0, &[], &[], &[])).err().expect("an exception");
    assert!(thrown.description.starts_with("java.lang.IllegalStateException: Unregistered holder in ResourceKey[minecraft:root / minecraft:worldgen/biome]: Direct{net.minecraft.world.level.biome.Biome@"));
    assert_eq!(thrown.stack_trace.len(), 11);
    assert_eq!(thrown.stack_trace[4], "net.minecraft.world.level.chunk.storage.SerializableChunkData.write(SerializableChunkData.java:432)");
}

#[test]
fn inline_biomes_are_sent_with_the_id_minus_one() {
    use crate::world::level::chunk::palette::PaletteValue;
    assert_eq!(decode(biome_tag("minecraft:plains")).biome.global_id() as i32, -1);
}

#[test]
fn a_fixed_biome_keeps_the_plains_default_in_the_network_palette() {
    let chunk_biomes = |biome: Biome| {
        let section = crate::world::level::chunk::LevelChunkSection::with_fixed_biome(biome);
        let mut w = wafer_protocol::Writer::new();
        section.biomes.write(&mut w);
        w.into_inner()
    };
    let plains = Biome::by_name("minecraft:plains").unwrap();
    let desert = Biome::by_name("minecraft:desert").unwrap();
    assert_eq!(chunk_biomes(plains), [0, plains.0 as u8]);
    let written = chunk_biomes(desert);
    assert_eq!(&written[..4], [1, 2, plains.0 as u8, desert.0 as u8]);
    assert_eq!(written.len(), 4 + 8);
    let inline = chunk_biomes(decode(biome_tag("minecraft:desert")).biome);
    assert_eq!(inline[..3], [1, 2, plains.0 as u8]);
    assert_eq!(inline[3..8], [0xff, 0xff, 0xff, 0xff, 0x0f]);
}
