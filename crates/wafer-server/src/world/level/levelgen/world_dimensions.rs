use wafer_protocol::nbt::Nbt;

use crate::util::codec::{self, DecodeResult};
use crate::util::java_throwable::JavaThrowable;
use crate::util::lifecycle::Lifecycle;
use crate::world::level::chunk::unseeded_chunk_generator::UnseededChunkGenerator;
use crate::world::level::dimension::builtin_dimension_types;
use crate::world::level::dimension::level_stem::{END, LevelStem, NETHER, OVERWORLD};

const BUILTIN_ORDER: [&str; 3] = [OVERWORLD, NETHER, END];

const OVERWORLD_SETTINGS_MISSING: &str = "java.lang.IllegalStateException: Overworld settings missing";
const CONSTRUCTOR_FRAME: &str = "net.minecraft.world.level.levelgen.WorldDimensions.<init>(WorldDimensions.java:56)";
const REGISTRY_CONSTRUCTOR_FRAME: &str = "net.minecraft.world.level.levelgen.WorldDimensions.<init>(WorldDimensions.java:61)";
const CODEC_HEAD: [&str; 3] = [
    "com.mojang.serialization.codecs.RecordCodecBuilder$Instance$1.lambda$decode$0(RecordCodecBuilder.java:185)",
    "com.mojang.serialization.DataResult$Success.map(DataResult.java:175)",
    "com.mojang.serialization.codecs.RecordCodecBuilder$Instance$1.lambda$decode$1(RecordCodecBuilder.java:184)",
];
const CODEC_SUCCESS_FLAT_MAP: [&str; 1] = ["com.mojang.serialization.DataResult$Success.flatMap(DataResult.java:201)"];
const CODEC_ERROR_FLAT_MAP: [&str; 2] = ["com.mojang.serialization.DataResult$Error.flatMap(DataResult.java:335)", "com.mojang.serialization.DataResult$Error.flatMap(DataResult.java:250)"];
const CODEC_TAIL: [&str; 9] = [
    "com.mojang.serialization.codecs.RecordCodecBuilder$Instance$1.decode(RecordCodecBuilder.java:183)",
    "com.mojang.serialization.codecs.RecordCodecBuilder$2.decode(RecordCodecBuilder.java:107)",
    "com.mojang.serialization.codecs.RecordCodecBuilder$Instance$3.decode(RecordCodecBuilder.java:249)",
    "com.mojang.serialization.codecs.RecordCodecBuilder$2.decode(RecordCodecBuilder.java:107)",
    "com.mojang.serialization.MapDecoder.lambda$compressedDecode$1(MapDecoder.java:52)",
    "com.mojang.serialization.DataResult$Success.flatMap(DataResult.java:201)",
    "com.mojang.serialization.MapDecoder.compressedDecode(MapDecoder.java:52)",
    "com.mojang.serialization.MapCodec$MapCodecCodec.decode(MapCodec.java:159)",
    "com.mojang.serialization.Decoder.parse(Decoder.java:18)",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpecialWorldProperty {
    None,
    Flat,
    Debug,
}

#[derive(Clone)]
pub struct WorldDimensions {
    dimensions: Vec<(String, LevelStem)>,
}

pub struct Complete {
    pub dimensions: Vec<(String, LevelStem)>,
    pub special_world_property: SpecialWorldProperty,
    lifecycle: Lifecycle,
}

impl Complete {
    pub fn lifecycle(&self) -> Lifecycle {
        self.lifecycle
    }

    pub fn levels_in_creation_order(self) -> Vec<(String, LevelStem)> {
        let rank = |key: &str| match key {
            OVERWORLD => 0,
            END => 1,
            NETHER => 2,
            _ => 3,
        };
        let mut dimensions = self.dimensions;
        dimensions.sort_by_key(|(key, _)| rank(key));
        dimensions
    }
}

fn overworld_settings_missing(frames: Vec<&'static str>) -> JavaThrowable {
    JavaThrowable::new(OVERWORLD_SETTINGS_MISSING, frames)
}

impl WorldDimensions {
    pub fn new(dimensions: Vec<(String, LevelStem)>, caller_frames: &[&'static str]) -> Result<WorldDimensions, JavaThrowable> {
        if !dimensions.iter().any(|(key, _)| key == OVERWORLD) {
            let mut frames = vec![CONSTRUCTOR_FRAME];
            frames.extend_from_slice(caller_frames);
            return Err(overworld_settings_missing(frames));
        }
        Ok(WorldDimensions { dimensions })
    }

    pub fn from_registry(registry: &[(String, LevelStem)], caller_frames: &[&'static str]) -> Result<WorldDimensions, JavaThrowable> {
        let mut frames = vec![REGISTRY_CONSTRUCTOR_FRAME];
        frames.extend_from_slice(caller_frames);
        WorldDimensions::new(registry.to_vec(), &frames)
    }

    pub fn keys_in_order<'a>(known_keys: impl Iterator<Item = &'a str> + Clone) -> Vec<&'a str> {
        let mut keys: Vec<&str> = BUILTIN_ORDER.iter().copied().filter(|builtin| known_keys.clone().any(|key| key == *builtin)).collect();
        keys.extend(known_keys.filter(|key| !BUILTIN_ORDER.contains(key)));
        keys
    }

    pub fn replace_overworld_generator(&self, generator: UnseededChunkGenerator) -> WorldDimensions {
        let dimension_type = match self.get(OVERWORLD) {
            Some(stem) => stem.dimension_type.clone(),
            None => crate::world::level::dimension::level_stem::DimensionTypeHolder::Reference(builtin_dimension_types::OVERWORLD.to_string()),
        };
        WorldDimensions { dimensions: WorldDimensions::with_overworld(&self.dimensions, LevelStem::new(dimension_type, generator)) }
    }

    pub fn with_overworld(dimensions: &[(String, LevelStem)], overworld: LevelStem) -> Vec<(String, LevelStem)> {
        let mut result = dimensions.to_vec();
        match result.iter_mut().find(|(key, _)| key == OVERWORLD) {
            Some(entry) => entry.1 = overworld,
            None => result.push((OVERWORLD.to_string(), overworld)),
        }
        result
    }

    pub fn dimensions(&self) -> &[(String, LevelStem)] {
        &self.dimensions
    }

    pub fn overworld(&self) -> &UnseededChunkGenerator {
        &self.get(OVERWORLD).expect("overworld settings").generator
    }

    pub fn get(&self, key: &str) -> Option<&LevelStem> {
        self.dimensions.iter().find(|(stem_key, _)| stem_key == key).map(|(_, stem)| stem)
    }

    pub fn is_debug(&self) -> bool {
        self.overworld().is_debug()
    }

    fn special_world_property(registry: &[(String, LevelStem)]) -> SpecialWorldProperty {
        match registry.iter().find(|(key, _)| key == OVERWORLD) {
            Some((_, overworld)) if overworld.generator.is_debug() => SpecialWorldProperty::Debug,
            Some((_, overworld)) if overworld.generator.is_flat() => SpecialWorldProperty::Flat,
            _ => SpecialWorldProperty::None,
        }
    }

    fn check_stability(key: &str, dimension: &LevelStem) -> Lifecycle {
        if WorldDimensions::is_vanilla_like(key, dimension) { Lifecycle::Stable } else { Lifecycle::Experimental }
    }

    fn is_vanilla_like(key: &str, dimension: &LevelStem) -> bool {
        match key {
            OVERWORLD => WorldDimensions::is_stable_overworld(dimension),
            NETHER => WorldDimensions::is_stable_nether(dimension),
            END => WorldDimensions::is_stable_end(dimension),
            _ => false,
        }
    }

    fn is_stable_overworld(dimension: &LevelStem) -> bool {
        let dimension_type = &dimension.dimension_type;
        if !dimension_type.is(builtin_dimension_types::OVERWORLD) && !dimension_type.is(builtin_dimension_types::OVERWORLD_CAVES) {
            return false;
        }
        match &dimension.generator {
            UnseededChunkGenerator::Noise(noise) => noise.multi_noise_biome_source_stable("minecraft:overworld").unwrap_or(true),
            _ => true,
        }
    }

    fn is_stable_nether(dimension: &LevelStem) -> bool {
        dimension.dimension_type.is(builtin_dimension_types::NETHER)
            && matches!(&dimension.generator, UnseededChunkGenerator::Noise(noise) if noise.stable("minecraft:nether") && noise.multi_noise_biome_source_stable("minecraft:nether") == Some(true))
    }

    fn is_stable_end(dimension: &LevelStem) -> bool {
        dimension.dimension_type.is(builtin_dimension_types::END)
            && matches!(&dimension.generator, UnseededChunkGenerator::Noise(noise) if noise.stable("minecraft:end") && noise.is_the_end_biome_source())
    }

    pub fn bake(&self, base_dimensions: &[(String, LevelStem)]) -> Complete {
        let mut known_dimensions: Vec<&str> = base_dimensions.iter().map(|(key, _)| key.as_str()).collect();
        for (key, _) in &self.dimensions {
            if !known_dimensions.contains(&key.as_str()) {
                known_dimensions.push(key);
            }
        }
        let mut results: Vec<(String, LevelStem)> = Vec::new();
        for key in WorldDimensions::keys_in_order(known_dimensions.iter().copied()) {
            let stem = base_dimensions.iter().find(|(base_key, _)| base_key == key).or_else(|| self.dimensions.iter().find(|(stem_key, _)| stem_key == key));
            if let Some((_, stem)) = stem {
                results.push((key.to_string(), stem.clone()));
            }
        }
        let mut lifecycle = if BUILTIN_ORDER.iter().all(|builtin| known_dimensions.contains(builtin)) { Lifecycle::Stable } else { Lifecycle::Experimental };
        for (key, stem) in &results {
            lifecycle = lifecycle.add(WorldDimensions::check_stability(key, stem));
        }
        let special_world_property = WorldDimensions::special_world_property(&results);
        Complete { dimensions: results, special_world_property, lifecycle }
    }

    pub fn decode(tag: &Nbt, caller_frames: &[&'static str]) -> Result<DecodeResult<WorldDimensions>, JavaThrowable> {
        let dimensions = match codec::field(tag, "dimensions") {
            Ok(dimensions) => codec::base_map_partial(dimensions, |key| codec::identifier(&Nbt::String(key.to_string())), LevelStem::decode),
            Err(error) => return Ok(DecodeResult::error(error)),
        };
        let DecodeResult { value, error } = dimensions;
        let Some(entries) = value else { return Ok(DecodeResult { value: None, error }) };
        let flat_map: &[&'static str] = if error.is_some() { &CODEC_ERROR_FLAT_MAP } else { &CODEC_SUCCESS_FLAT_MAP };
        let mut frames: Vec<&'static str> = CODEC_HEAD.to_vec();
        frames.extend_from_slice(flat_map);
        frames.extend_from_slice(&CODEC_TAIL);
        frames.extend_from_slice(caller_frames);
        let world_dimensions = WorldDimensions::new(entries, &frames)?;
        Ok(DecodeResult { value: Some(world_dimensions), error })
    }

    pub fn encode(&self) -> Nbt {
        Nbt::Compound(self.dimensions.iter().map(|(key, stem)| (key.clone(), stem.encode())).collect())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;
    use crate::world::item::component::json_to_nbt;
    use crate::world::level::levelgen::presets::world_preset::WorldPreset;
    use crate::world::level::levelgen::world_gen_settings::WorldGenSettings;
    use crate::world::level::levelgen::world_options::WorldOptions;

    fn stem_error(json: &str) -> String {
        let value: Value = serde_json::from_str(json).unwrap();
        crate::nbt::with_json_source(|| LevelStem::decode(&json_to_nbt(&value)).error.expect("a decode error"))
    }

    fn preset_error(json: &str) -> String {
        let value: Value = serde_json::from_str(json).unwrap();
        crate::nbt::with_json_source(|| WorldPreset::decode(&json_to_nbt(&value)).error.expect("a decode error"))
    }

    fn preset(name: &str) -> WorldDimensions {
        crate::world::level::levelgen::presets::world_presets::get(name).expect("a world preset").create_world_dimensions()
    }

    fn decode_settings(snbt: &str) -> Result<DecodeResult<WorldGenSettings>, JavaThrowable> {
        let tag = crate::nbt::parse_fully(snbt).expect("valid snbt");
        codec::with_nbt_ops(|| WorldGenSettings::decode(&tag, &["caller"]))
    }

    #[test]
    fn a_stem_reports_its_fields_in_vanilla_order() {
        assert_eq!(stem_error("{}"), "No key generator in MapLike[{}]; No key type in MapLike[{}]");
        assert_eq!(
            stem_error(r#"{"type":"ta:nope","generator":{"type":"minecraft:nope"}}"#),
            "Unknown registry key in ResourceKey[minecraft:root / minecraft:worldgen/chunk_generator]: minecraft:nope; Failed to get element ta:nope from registry minecraft:dimension_type"
        );
        assert_eq!(
            stem_error(r#"{"type":"minecraft:overworld","generator":{"type":"minecraft:noise"}}"#),
            r#"No key settings in MapLike[{"type":"minecraft:noise"}]; No key biome_source in MapLike[{"type":"minecraft:noise"}]"#
        );
    }

    #[test]
    fn biome_sources_report_dispatch_and_field_errors() {
        let noise = |source: &str| format!(r#"{{"type":"minecraft:overworld","generator":{{"type":"minecraft:noise","settings":"minecraft:overworld","biome_source":{source}}}}}"#);
        assert!(stem_error(&noise(r#"{}"#)).ends_with("No key type in MapLike[{}]"));
        assert!(stem_error(&noise(r#"{"type":"minecraft:fixed","biome":"minecraft:nope"}"#)).ends_with("Failed to get element minecraft:nope from registry minecraft:worldgen/biome"));
        assert!(stem_error(&noise(r#"{"type":"minecraft:checkerboard","biomes":"minecraft:plains","scale":99}"#)).ends_with("Value 99 outside of range [0:62]"));
        assert!(stem_error(&noise(r#"{"type":"minecraft:multi_noise"}"#))
            .ends_with(r#"No key preset in MapLike[{"type":"minecraft:multi_noise"}]; No key biomes in MapLike[{"type":"minecraft:multi_noise"}]"#));
    }

    #[test]
    fn a_world_preset_needs_an_overworld() {
        assert_eq!(preset_error(r#"{"dimensions":{}}"#), "Missing overworld dimension");
        assert_eq!(preset_error("{}"), "No key dimensions in MapLike[{}]");
        assert_eq!(
            preset_error(r#"{"dimensions":{"ta:x":{"type":"minecraft:overworld"}}}"#),
            r#"No key generator in MapLike[{"type":"minecraft:overworld"}] missed input: {"ta:x":{"type":"minecraft:overworld"}}; Missing overworld dimension"#
        );
    }

    #[test]
    fn dimensions_without_an_overworld_throw_the_vanilla_exception() {
        let thrown = WorldDimensions::new(Vec::new(), &["caller"]).err().expect("an exception");
        assert_eq!(thrown.description, OVERWORLD_SETTINGS_MISSING);
        assert_eq!(thrown.stack_trace, [CONSTRUCTOR_FRAME, "caller"]);
        let thrown = WorldDimensions::from_registry(&[], &["caller"]).err().expect("an exception");
        assert_eq!(thrown.stack_trace, [CONSTRUCTOR_FRAME, REGISTRY_CONSTRUCTOR_FRAME, "caller"]);
    }

    #[test]
    fn decoding_dimensions_throws_while_the_codec_applies_the_constructor() {
        let thrown = decode_settings(r#"{seed:5L,dimensions:{}}"#).err().expect("an exception");
        assert_eq!(thrown.description, OVERWORLD_SETTINGS_MISSING);
        assert_eq!(thrown.stack_trace[0], CONSTRUCTOR_FRAME);
        assert_eq!(thrown.stack_trace[4], "com.mojang.serialization.DataResult$Success.flatMap(DataResult.java:201)");
        assert_eq!(thrown.stack_trace.last(), Some(&"caller"));
        let thrown = decode_settings(r#"{seed:5L,dimensions:{"minecraft:overworld":{type:"minecraft:overworld",generator:{type:"minecraft:nope"}}}}"#).err().expect("an exception");
        assert_eq!(thrown.stack_trace[4], "com.mojang.serialization.DataResult$Error.flatMap(DataResult.java:335)");
        assert_eq!(thrown.stack_trace[5], "com.mojang.serialization.DataResult$Error.flatMap(DataResult.java:250)");
    }

    #[test]
    fn a_broken_dimension_besides_the_overworld_is_an_error_without_an_exception() {
        let dimensions = preset("minecraft:normal").encode();
        let Nbt::Compound(entries) = dimensions else { panic!("a compound") };
        let nether = entries.iter().find(|(key, _)| key == NETHER).unwrap().1.clone();
        let broken_nether = Nbt::Compound(vec![("generator".into(), nether.get("generator").unwrap().clone()), ("type".into(), Nbt::String("minecraft:nope".into()))].into());
        let overworld = entries.iter().find(|(key, _)| key == OVERWORLD).unwrap().1.clone();
        let tag = Nbt::Compound(vec![
            ("seed".into(), Nbt::Long(5)),
            ("dimensions".into(), Nbt::Compound(vec![(OVERWORLD.into(), overworld), (NETHER.into(), broken_nether)].into())),
        ].into());
        let decoded = codec::with_nbt_ops(|| WorldGenSettings::decode(&tag, &[])).unwrap();
        assert!(decoded.value.is_some());
        assert!(decoded.error.expect("an error").starts_with("Failed to get element minecraft:nope from registry minecraft:dimension_type missed input: "));
    }

    #[test]
    fn world_options_report_their_fields_in_vanilla_order() {
        let decoded = decode_settings(r#"{generate_structures:"x",bonus_chest:"y",dimensions:{}}"#);
        let thrown = decoded.err().expect("the constructor throws first");
        assert_eq!(thrown.description, OVERWORLD_SETTINGS_MISSING);
        let tag = crate::nbt::parse_fully(r#"{generate_structures:"x",bonus_chest:"y"}"#).unwrap();
        let options = codec::with_nbt_ops(|| WorldOptions::decode(&tag));
        assert_eq!(options.error.as_deref(), Some("Not a number; Not a number; No key seed in MapLike[{bonus_chest:\"y\",generate_structures:\"x\"}]"));
        assert!(options.value.is_none());
    }

    #[test]
    fn world_gen_settings_round_trip_through_their_codec() {
        for name in ["minecraft:normal", "minecraft:large_biomes", "minecraft:amplified", "minecraft:single_biome_surface", "minecraft:flat", "minecraft:flat_all_dimensions", "minecraft:debug_all_block_states"] {
            let settings = WorldGenSettings::new(WorldOptions::new(7, false, true), preset(name));
            let encoded = settings.encode();
            let decoded = codec::with_nbt_ops(|| WorldGenSettings::decode(&encoded, &[])).unwrap();
            assert_eq!(decoded.error, None, "{name}");
            assert_eq!(decoded.value.expect("settings").encode(), encoded, "{name}");
        }
    }

    #[test]
    fn baking_derives_the_lifecycle_and_the_special_world_property() {
        let normal = preset("minecraft:normal").bake(&[]);
        assert_eq!((normal.lifecycle(), normal.special_world_property), (Lifecycle::Stable, SpecialWorldProperty::None));
        let flat = preset("minecraft:flat").bake(&[]);
        assert_eq!((flat.lifecycle(), flat.special_world_property), (Lifecycle::Stable, SpecialWorldProperty::Flat));
        let debug = preset("minecraft:debug_all_block_states").bake(&[]);
        assert_eq!((debug.lifecycle(), debug.special_world_property), (Lifecycle::Stable, SpecialWorldProperty::Debug));
        assert_eq!(preset("minecraft:flat_all_dimensions").bake(&[]).lifecycle(), Lifecycle::Experimental);
        let overworld_only = WorldDimensions::new(vec![preset("minecraft:normal").get(OVERWORLD).cloned().map(|stem| (OVERWORLD.to_string(), stem)).unwrap()], &[]).unwrap();
        assert_eq!(overworld_only.bake(&[]).lifecycle(), Lifecycle::Experimental);
        let extra = preset("minecraft:normal").get(NETHER).cloned().unwrap();
        assert_eq!(preset("minecraft:normal").bake(&[("ta:extra".to_string(), extra)]).lifecycle(), Lifecycle::Experimental);
    }

    #[test]
    fn baked_dimensions_prefer_the_data_pack_and_follow_the_builtin_order() {
        let flat = preset("minecraft:flat");
        let replacement = flat.get(OVERWORLD).cloned().unwrap();
        let baked = preset("minecraft:normal").bake(&[("ta:extra".to_string(), replacement.clone()), (OVERWORLD.to_string(), replacement)]);
        assert_eq!(baked.dimensions.iter().map(|(key, _)| key.as_str()).collect::<Vec<_>>(), [OVERWORLD, NETHER, END, "ta:extra"]);
        assert_eq!(baked.special_world_property, SpecialWorldProperty::Flat);
        assert_eq!(baked.levels_in_creation_order().iter().map(|(key, _)| key.as_str()).collect::<Vec<_>>(), [OVERWORLD, END, NETHER, "ta:extra"]);
    }

    #[test]
    fn replacing_the_overworld_generator_keeps_its_position_and_type() {
        let replaced = preset("minecraft:normal").replace_overworld_generator(UnseededChunkGenerator::Debug);
        assert!(replaced.is_debug());
        assert_eq!(replaced.dimensions().iter().map(|(key, _)| key.as_str()).collect::<Vec<_>>(), [OVERWORLD, NETHER, END]);
        assert!(replaced.get(OVERWORLD).unwrap().dimension_type.is("minecraft:overworld"));
    }
}
