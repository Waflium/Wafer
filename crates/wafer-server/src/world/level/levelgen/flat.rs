use wafer_data::Block;
use wafer_protocol::nbt::Nbt;

use crate::world::level::chunk::Biome;
use crate::util::codec::{self, DecodeResult, Record};
use crate::world::item::component::values::decode_item;
use crate::world::level::chunk::FlatLevelSource;

const Y_SIZE: i64 = 4064;
const STRUCTURE_SET: &str = "minecraft:worldgen/structure_set";
const BIOME: &str = "minecraft:worldgen/biome";

fn allows_unbound_references() -> bool {
    !codec::is_nbt_ops() && crate::core::data_registries::is_loading()
}

#[derive(Clone, Debug, PartialEq)]
pub enum StructureOverrides {
    Tag(String),
    Direct(Vec<String>),
}

impl StructureOverrides {
    pub fn to_nbt(&self) -> Nbt {
        match self {
            StructureOverrides::Tag(tag) => Nbt::String(format!("#{tag}")),
            StructureOverrides::Direct(ids) if ids.len() == 1 => Nbt::String(ids[0].clone()),
            StructureOverrides::Direct(ids) => Nbt::List(ids.iter().map(|id| Nbt::String(id.clone())).collect()),
        }
    }

    fn from_strings<'a>(single: Option<&'a str>, list: Option<Vec<&'a str>>) -> Option<StructureOverrides> {
        let structure_set = |id: &str| {
            let id = crate::resources::identifier::parse(id).ok()?;
            if crate::core::data_registries::present_json_entries(STRUCTURE_SET).any(|(name, _)| *name == id) {
                return Some(id);
            }
            if allows_unbound_references() {
                crate::core::data_registries::unknown_reference(STRUCTURE_SET, &id);
                return Some(id);
            }
            None
        };
        let tag = |tag: &str| {
            let key = crate::resources::identifier::read(tag).ok()?;
            if !crate::world::item::component::codec::tag_exists(STRUCTURE_SET, &key) && crate::core::data_registries::missing_tag(STRUCTURE_SET, &key).is_some() {
                return None;
            }
            Some(StructureOverrides::Tag(key))
        };
        match (single, list) {
            (Some(name), _) if name.starts_with('#') => tag(&name[1..]),
            (Some(id), _) => structure_set(id).map(|id| StructureOverrides::Direct(vec![id])),
            (None, Some(ids)) => ids.into_iter().map(structure_set).collect::<Option<Vec<_>>>().map(StructureOverrides::Direct),
            (None, None) => None,
        }
    }

    pub fn from_nbt(nbt: &Nbt) -> Option<StructureOverrides> {
        match nbt {
            Nbt::String(s) => Self::from_strings(Some(s), None),
            Nbt::List(items) => Self::from_strings(None, items.iter().map(Nbt::as_str).collect()),
            _ => None,
        }
    }
}

pub fn decode_preset(tag: &Nbt) -> DecodeResult<()> {
    if let Err(error) = codec::map(tag) {
        return DecodeResult::error(error);
    }
    let mut record = Record::default();
    let display = record.required(tag, "display", |display| DecodeResult::from(decode_item(display)));
    let settings = record.required(tag, "settings", |settings| {
        let settings = decode_settings(settings);
        DecodeResult { value: settings.value.map(drop), error: settings.error }
    });
    record.build(|| display.and(settings))
}

fn decode_biome(tag: &Nbt) -> Option<Biome> {
    if let Nbt::Compound(_) = tag {
        return crate::world::level::biome::validate(tag).ok().map(|()| crate::world::level::biome::inline_biome::register(tag));
    }
    codec::string(tag).ok().and_then(|id| crate::resources::identifier::parse(id).ok()).and_then(|id| {
        Biome::by_name(&id).or_else(|| {
            allows_unbound_references().then(|| {
                crate::core::data_registries::unknown_reference(BIOME, &id);
                Biome::by_name("minecraft:plains").expect("plains biome")
            })
        })
    })
}

pub fn decode_settings(tag: &Nbt) -> DecodeResult<FlatLevelSource> {
    if let Err(error) = codec::map(tag) {
        return DecodeResult::error(error);
    }
    let mut record = Record::default();
    let layers = record.required(tag, "layers", |layers| codec::list_partial(layers, decode_layer));
    let lakes = record.with_default(tag, "lakes", false, codec::bool_value);
    let features = record.with_default(tag, "features", false, codec::bool_value);
    let biome = tag.get("biome").and_then(decode_biome);
    let structure_overrides = tag.get("structure_overrides").and_then(StructureOverrides::from_nbt);
    let mut settings = record.build(|| {
        let layers = layers?;
        let biome = biome.unwrap_or_else(|| {
            error!("Unknown biome, defaulting to plains");
            Biome::by_name("minecraft:plains").expect("plains biome")
        });
        Some(FlatLevelSource { layers, biome, structure_overrides, lakes, features, generation: Default::default() })
    });
    if let Some(value) = &settings.value
        && value.layers.iter().map(|&(_, height)| height as i64).sum::<i64>() > Y_SIZE
    {
        let error = format!("Sum of layer heights is > {Y_SIZE}");
        settings.error = Some(match settings.error.take() {
            Some(previous) => format!("{previous}; {error}"),
            None => error,
        });
    }
    settings
}

fn decode_layer(tag: &Nbt) -> DecodeResult<(wafer_data::BlockState, u32)> {
    if let Err(error) = codec::map(tag) {
        return DecodeResult::error(error);
    }
    let mut record = Record::default();
    let height = record.required(tag, "height", |height| match codec::int(height) {
        Ok(height) if (0..=Y_SIZE as i32).contains(&height) => DecodeResult::success(height),
        Ok(height) => DecodeResult::error(format!("Value {height} outside of range [0:{Y_SIZE}]")),
        Err(error) => DecodeResult::error(error),
    });
    let block = record.required(tag, "block", |block| {
        DecodeResult::from(codec::identifier(block).and_then(|id| Block::by_name(&id).ok_or_else(|| format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:block]: {id}"))))
    });
    record.build(|| Some((block?.default_state(), height? as u32)))
}
