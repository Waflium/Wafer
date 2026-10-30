use std::sync::{Arc, LazyLock, OnceLock};

use wafer_core::BlockPos;
use wafer_core::random::RandomSource;
use serde_json::Value;

use wafer_protocol::nbt::Nbt;

use super::structure_pool_element::{self, StructurePoolElement};
use crate::util::codec::DecodeResult;
use crate::world::item::component::codec::{Codec, RangeError, Stream};
use crate::world::item::component::types::{self, Pair, req};
use crate::world::level::block::rotation::Rotation;
use crate::world::level::levelgen::blockpredicates::block_predicate::resource;
use crate::world::level::levelgen::structure::templatesystem::structure_processor::StructureProcessor;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Projection {
    TerrainMatching,
    Rigid,
}

static TERRAIN_MATCHING_PROCESSORS: LazyLock<[StructureProcessor; 1]> =
    LazyLock::new(|| [StructureProcessor::Gravity { heightmap: crate::world::level::chunk::HeightmapType::WorldSurfaceWg, offset: -1 }]);

impl Projection {
    pub fn by_name(name: &str) -> Option<Projection> {
        match name {
            "terrain_matching" => Some(Projection::TerrainMatching),
            "rigid" => Some(Projection::Rigid),
            _ => None,
        }
    }

    pub fn get_name(self) -> &'static str {
        match self {
            Projection::TerrainMatching => "terrain_matching",
            Projection::Rigid => "rigid",
        }
    }

    pub fn get_processors(self) -> &'static [StructureProcessor] {
        match self {
            Projection::TerrainMatching => &*TERRAIN_MATCHING_PROCESSORS,
            Projection::Rigid => &[],
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct TemplatePoolHolder(pub u16);

pub const EMPTY: &str = "minecraft:empty";

impl TemplatePoolHolder {
    pub fn by_name(name: &str) -> Option<TemplatePoolHolder> {
        let name = resource(name);
        crate::core::data_registries::json_entries("minecraft:worldgen/template_pool").iter().position(|(n, _)| *n == name.as_str()).map(|i| TemplatePoolHolder(i as u16))
    }

    pub fn key(self) -> &'static str {
        crate::core::data_registries::json_entries("minecraft:worldgen/template_pool")[self.0 as usize].0
    }

    pub fn value(self) -> &'static StructureTemplatePool {
        POOLS[self.0 as usize].get_or_init(|| {
            let (name, source) = crate::core::data_registries::json_entries("minecraft:worldgen/template_pool")[self.0 as usize];
            let json: Value = serde_json::from_str(source).expect("template pool json");
            StructureTemplatePool::parse(&json).unwrap_or_else(|e| panic!("template pool {name}: {e}"))
        })
    }

    pub fn is(self, name: &str) -> bool {
        self.key() == name
    }
}

static POOLS: LazyLock<Vec<OnceLock<StructureTemplatePool>>> = LazyLock::new(|| (0..crate::core::data_registries::json_entries("minecraft:worldgen/template_pool").len()).map(|_| OnceLock::new()).collect());

pub struct StructureTemplatePool {
    templates: Vec<Arc<StructurePoolElement>>,
    fallback: TemplatePoolHolder,
    max_size: OnceLock<i32>,
}

impl StructureTemplatePool {
    pub fn parse(json: &Value) -> Result<StructureTemplatePool, String> {
        let fallback_name = json.get("fallback").and_then(Value::as_str).ok_or("missing fallback")?;
        let fallback = TemplatePoolHolder::by_name(fallback_name).ok_or_else(|| format!("unknown pool {fallback_name}"))?;
        let mut templates = Vec::new();
        for entry in json.get("elements").and_then(Value::as_array).ok_or("missing elements")? {
            let element = Arc::new(StructurePoolElement::parse(entry.get("element").ok_or("missing element")?, None)?);
            let weight = entry.get("weight").and_then(Value::as_i64).ok_or("missing weight")?;
            for _ in 0..weight {
                templates.push(element.clone());
            }
        }
        Ok(StructureTemplatePool { templates, fallback, max_size: OnceLock::new() })
    }

    pub fn get_max_size(&self) -> i32 {
        *self.max_size.get_or_init(|| {
            self.templates
                .iter()
                .filter(|t| !t.is_empty())
                .map(|t| t.get_bounding_box(BlockPos::ZERO, Rotation::None).y_span())
                .max()
                .unwrap_or(0)
        })
    }

    pub fn get_fallback(&self) -> TemplatePoolHolder {
        self.fallback
    }

    pub fn get_random_template(&self, random: &mut impl RandomSource) -> Arc<StructurePoolElement> {
        if self.templates.is_empty() {
            Arc::new(StructurePoolElement::Empty)
        } else {
            self.templates[random.next_int_bounded(self.templates.len() as i32) as usize].clone()
        }
    }

    pub fn get_shuffled_templates(&self, random: &mut impl RandomSource) -> Vec<Arc<StructurePoolElement>> {
        let mut copy = self.templates.clone();
        shuffle(&mut copy, random);
        copy
    }

    pub fn size(&self) -> usize {
        self.templates.len()
    }
}

pub fn shuffle<T>(list: &mut [T], random: &mut impl RandomSource) {
    for i in (2..=list.len()).rev() {
        let swap_to = random.next_int_bounded(i as i32) as usize;
        list.swap(i - 1, swap_to);
    }
}

fn direct_codec_lazy() -> &'static Pair {
    static CODEC: LazyLock<Pair> = LazyLock::new(direct_codec);
    &CODEC
}

fn element_with_weight(tag: &Nbt) -> DecodeResult<Nbt> {
    static ELEMENT: LazyLock<Codec> = LazyLock::new(|| Codec::Record(vec![req("element", (Codec::Lazy(structure_pool_element::codec_lazy), Stream::Lazy))]));
    static WEIGHT: LazyLock<Codec> = LazyLock::new(|| Codec::Record(vec![req("weight", (Codec::Int(1, 150, RangeError::Outside), Stream::Int))]));
    ELEMENT.decode_partial(tag).flat_map(|element| WEIGHT.decode_partial(tag).map(|weight| Nbt::Compound(vec![("element".into(), element), ("weight".into(), weight)].into())))
}

pub fn direct_codec() -> Pair {
    types::record(vec![req("fallback", holder_codec()), req("elements", types::list((Codec::Func(element_with_weight), Stream::Nbt)))])
}

pub fn holder_codec() -> Pair {
    (Codec::Holder("minecraft:worldgen/template_pool", Some(Box::new(Codec::Lazy(direct_codec_lazy)))), Stream::Nbt)
}
