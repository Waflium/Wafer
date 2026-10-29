use std::sync::LazyLock;

use wafer_core::BlockPos;
use wafer_core::random::RandomSource;
use serde_json::Value;

use super::abstract_huge_mushroom_feature::{AbstractHugeMushroomFeature, HugeMushroomType};
use super::bamboo_feature::BambooFeature;
use super::fill_layer_feature::FillLayerFeature;
use super::block_blob_feature::BlockBlobFeature;
use super::coral_claw_feature::CoralClawFeature;
use super::coral_tree_feature::CoralTreeFeature;
use super::disk_feature::DiskFeature;
use super::lake_feature::LakeFeature;
use super::monster_room_feature;
use super::fossil_feature::FossilFeature;
use super::overlay_feature::OverlayFeature;
use super::template_feature::TemplateFeature;
use super::root_system_feature::RootSystemFeature;
use super::sculk_patch_feature::SculkPatchFeature;
use super::sequence_feature::SequenceFeature;
use super::underwater_magma_feature::UnderwaterMagmaFeature;
use super::geode_feature::GeodeFeature;
use super::large_dripstone_feature::LargeDripstoneFeature;
use super::speleothem_cluster_feature::SpeleothemClusterFeature;
use super::speleothem_feature::SpeleothemFeature;
use super::iceberg_feature::IcebergFeature;
use super::spike_feature::SpikeFeature;
use super::spring_feature::SpringFeature;
use super::{blue_ice_feature, snow_and_freeze_feature};
use super::block_column_feature::BlockColumnFeature;
use super::block_pile_feature::BlockPileFeature;
use super::fallen_tree_feature::FallenTreeFeature;
use super::multiface_growth_feature::MultifaceGrowthFeature;
use super::ore_feature::OreFeature;
use super::random_boolean_selector_feature::RandomBooleanSelectorFeature;
use super::random_selector_feature::RandomSelectorFeature;
use super::scattered_ore_feature::ScatteredOreFeature;
use super::simple_block_feature::SimpleBlockFeature;
use super::simple_random_selector_feature::SimpleRandomSelectorFeature;
use super::tree_feature::TreeFeature;
use super::vegetation_patch_feature::VegetationPatchFeature;
use super::vines_feature;
use super::weighted_random_selector_feature::WeightedRandomSelectorFeature;
use super::delta_feature::DeltaFeature;
use super::end_gateway_feature::EndGatewayFeature;
use super::end_podium_feature::EndPodiumFeature;
use super::end_spike_feature::EndSpikeFeature;
use super::huge_fungus_feature::HugeFungusFeature;
use super::projected_random_patchy_square::ProjectedRandomPatchySquare;
use super::random_neighbor_spread_feature::RandomNeighborSpreadFeature;
use super::replace_blobs_feature::ReplaceBlobsFeature;
use super::replace_block_feature::ReplaceBlockFeature;
use super::single_block_pillar_feature::SingleBlockPillarFeature;
use super::stepped_column_cluster_feature::SteppedColumnClusterFeature;
use super::{bonus_chest_feature, chorus_plant_feature, end_island_feature, end_platform_feature, void_start_platform_feature};
use crate::world::level::world_gen_level::WorldGenLevel;
use crate::world::level::levelgen::blockpredicates::block_predicate::resource;

pub enum Feature {
    Tree(Box<TreeFeature>),
    FallenTree(FallenTreeFeature),
    SimpleBlock(SimpleBlockFeature),
    BlockColumn(BlockColumnFeature),
    BlockPile(BlockPileFeature),
    VegetationPatch(Box<VegetationPatchFeature>),
    Vines,
    Disk(DiskFeature),
    Spring(SpringFeature),
    FreezeTopLayer,
    Iceberg(IcebergFeature),
    Spike(Box<SpikeFeature>),
    BlueIce,
    Speleothem(Box<SpeleothemFeature>),
    SpeleothemCluster(Box<SpeleothemClusterFeature>),
    LargeDripstone(Box<LargeDripstoneFeature>),
    Geode(Box<GeodeFeature>),
    Lake(Box<LakeFeature>),
    MonsterRoom,
    UnderwaterMagma(UnderwaterMagmaFeature),
    BlockBlob(Box<BlockBlobFeature>),
    CoralTree(CoralTreeFeature),
    CoralClaw(CoralClawFeature),
    Sequence(SequenceFeature),
    Overlay(OverlayFeature),
    RootSystem(Box<RootSystemFeature>),
    Template(TemplateFeature),
    Fossil(Box<FossilFeature>),
    NoOp,
    FillLayer(FillLayerFeature),
    SculkPatch(SculkPatchFeature),
    MultifaceGrowth(MultifaceGrowthFeature),
    Bamboo(BambooFeature),
    HugeMushroom(Box<AbstractHugeMushroomFeature>),
    Ore(OreFeature),
    ScatteredOre(ScatteredOreFeature),
    RandomSelector(RandomSelectorFeature),
    SimpleRandomSelector(SimpleRandomSelectorFeature),
    RandomBooleanSelector(Box<RandomBooleanSelectorFeature>),
    WeightedRandomSelector(WeightedRandomSelectorFeature),
    BonusChest,
    ChorusPlant,
    Delta(DeltaFeature),
    EndGateway(EndGatewayFeature),
    EndIsland,
    EndPlatform,
    EndPodium(EndPodiumFeature),
    EndSpike(Box<EndSpikeFeature>),
    HugeFungus(Box<HugeFungusFeature>),
    ReplaceBlobs(ReplaceBlobsFeature),
    ProjectedRandomPatchySquare(ProjectedRandomPatchySquare),
    RandomNeighborSpread(Box<RandomNeighborSpreadFeature>),
    ReplaceBlock(ReplaceBlockFeature),
    SingleBlockPillar(Box<SingleBlockPillarFeature>),
    SteppedColumnCluster(Box<SteppedColumnClusterFeature>),
    VoidStartPlatform,
    NotImplemented,
}

static FEATURES: LazyLock<Vec<Feature>> = LazyLock::new(|| {
    crate::core::data_registries::json_entries("minecraft:worldgen/feature")
        .iter()
        .map(|(name, json)| {
            let json: Value = serde_json::from_str(json).expect("feature json");
            Feature::parse(&json).unwrap_or_else(|e| panic!("feature {name}: {e}"))
        })
        .collect()
});

pub fn feature_index(name: &str) -> Option<u32> {
    let name = resource(name);
    crate::core::data_registries::json_entries("minecraft:worldgen/feature").iter().position(|(n, _)| *n == name).map(|i| i as u32)
}

pub fn feature(name: &str) -> Option<&'static Feature> {
    feature_index(name).map(|index| &FEATURES[index as usize])
}

fn nbt_to_json(nbt: &wafer_protocol::nbt::Nbt) -> Value {
    use wafer_protocol::nbt::Nbt;
    match nbt {
        Nbt::End => Value::Null,
        Nbt::Byte(b) => Value::Bool(*b != 0),
        Nbt::Short(v) => Value::from(*v),
        Nbt::Int(v) => Value::from(*v),
        Nbt::Long(v) => Value::from(*v),
        Nbt::Float(v) => Value::from(*v),
        Nbt::Double(v) => Value::from(*v),
        Nbt::String(s) => Value::String(s.clone()),
        Nbt::ByteArray(a) => Value::Array(a.iter().map(|&v| Value::from(v)).collect()),
        Nbt::IntArray(a) => Value::Array(a.iter().map(|&v| Value::from(v)).collect()),
        Nbt::LongArray(a) => Value::Array(a.iter().map(|&v| Value::from(v)).collect()),
        Nbt::LazilyParsedNumber(text) => crate::util::gson::number_value(text),
        Nbt::List(items) => Value::Array(items.iter().map(nbt_to_json).collect()),
        Nbt::Compound(fields) => Value::Object(fields.iter().map(|(k, v)| (k.clone(), nbt_to_json(v))).collect()),
    }
}

impl Feature {
    pub fn parse_nbt(tag: &wafer_protocol::nbt::Nbt) -> Result<Feature, String> {
        Feature::parse(&nbt_to_json(tag))
    }

    pub fn parse(json: &Value) -> Result<Feature, String> {
        let ty = json.get("type").and_then(Value::as_str).ok_or("feature without type")?;
        Ok(match ty.strip_prefix("minecraft:").unwrap_or(ty) {
            "tree" => Feature::Tree(Box::new(TreeFeature::parse(json)?)),
            "fallen_tree" => Feature::FallenTree(FallenTreeFeature::parse(json)?),
            "simple_block" => Feature::SimpleBlock(SimpleBlockFeature::parse(json)?),
            "block_column" => Feature::BlockColumn(BlockColumnFeature::parse(json)?),
            "block_pile" => Feature::BlockPile(BlockPileFeature::parse(json)?),
            "vegetation_patch" => Feature::VegetationPatch(Box::new(VegetationPatchFeature::parse(json, false)?)),
            "waterlogged_vegetation_patch" => Feature::VegetationPatch(Box::new(VegetationPatchFeature::parse(json, true)?)),
            "vines" => Feature::Vines,
            "disk" => Feature::Disk(DiskFeature::parse(json)?),
            "spring_feature" => Feature::Spring(SpringFeature::parse(json)?),
            "freeze_top_layer" => Feature::FreezeTopLayer,
            "iceberg" => Feature::Iceberg(IcebergFeature::parse(json)?),
            "spike" => Feature::Spike(Box::new(SpikeFeature::parse(json)?)),
            "blue_ice" => Feature::BlueIce,
            "speleothem" => Feature::Speleothem(Box::new(SpeleothemFeature::parse(json)?)),
            "speleothem_cluster" => Feature::SpeleothemCluster(Box::new(SpeleothemClusterFeature::parse(json)?)),
            "large_dripstone" => Feature::LargeDripstone(Box::new(LargeDripstoneFeature::parse(json)?)),
            "geode" => Feature::Geode(Box::new(GeodeFeature::parse(json)?)),
            "lake" => Feature::Lake(Box::new(LakeFeature::parse(json)?)),
            "monster_room" => Feature::MonsterRoom,
            "underwater_magma" => Feature::UnderwaterMagma(UnderwaterMagmaFeature::parse(json)?),
            "block_blob" => Feature::BlockBlob(Box::new(BlockBlobFeature::parse(json)?)),
            "coral_tree" => Feature::CoralTree(CoralTreeFeature::parse(json)?),
            "coral_claw" => Feature::CoralClaw(CoralClawFeature::parse(json)?),
            "sequence" => Feature::Sequence(SequenceFeature::parse(json)?),
            "overlay" => Feature::Overlay(OverlayFeature::parse(json)?),
            "root_system" => Feature::RootSystem(Box::new(RootSystemFeature::parse(json)?)),
            "template" => Feature::Template(TemplateFeature::parse(json)?),
            "fossil" => Feature::Fossil(Box::new(FossilFeature::parse(json)?)),
            "no_op" => Feature::NoOp,
            "fill_layer" => Feature::FillLayer(FillLayerFeature::parse(json)?),
            "sculk_patch" => Feature::SculkPatch(SculkPatchFeature::parse(json)?),
            "multiface_growth" => Feature::MultifaceGrowth(MultifaceGrowthFeature::parse(json)?),
            "bamboo" => Feature::Bamboo(BambooFeature::parse(json)?),
            "huge_brown_mushroom" => Feature::HugeMushroom(Box::new(AbstractHugeMushroomFeature::parse(json, HugeMushroomType::Brown)?)),
            "huge_red_mushroom" => Feature::HugeMushroom(Box::new(AbstractHugeMushroomFeature::parse(json, HugeMushroomType::Red)?)),
            "ore" => Feature::Ore(OreFeature::parse(json)?),
            "scattered_ore" => Feature::ScatteredOre(ScatteredOreFeature::parse(json)?),
            "random_selector" => Feature::RandomSelector(RandomSelectorFeature::parse(json)?),
            "simple_random_selector" => Feature::SimpleRandomSelector(SimpleRandomSelectorFeature::parse(json)?),
            "random_boolean_selector" => Feature::RandomBooleanSelector(Box::new(RandomBooleanSelectorFeature::parse(json)?)),
            "weighted_random_selector" => Feature::WeightedRandomSelector(WeightedRandomSelectorFeature::parse(json)?),
            "bonus_chest" => Feature::BonusChest,
            "chorus_plant" => Feature::ChorusPlant,
            "delta_feature" => Feature::Delta(DeltaFeature::parse(json)?),
            "end_gateway" => Feature::EndGateway(EndGatewayFeature::parse(json)?),
            "end_island" => Feature::EndIsland,
            "end_platform" => Feature::EndPlatform,
            "end_podium" => Feature::EndPodium(EndPodiumFeature::parse(json)?),
            "end_spike" => Feature::EndSpike(Box::new(EndSpikeFeature::parse(json)?)),
            "huge_fungus" => Feature::HugeFungus(Box::new(HugeFungusFeature::parse(json)?)),
            "netherrack_replace_blobs" => Feature::ReplaceBlobs(ReplaceBlobsFeature::parse(json)?),
            "projected_random_patchy_square" => Feature::ProjectedRandomPatchySquare(ProjectedRandomPatchySquare::parse(json)?),
            "random_neighbor_spread" => Feature::RandomNeighborSpread(Box::new(RandomNeighborSpreadFeature::parse(json)?)),
            "replace_single_block" => Feature::ReplaceBlock(ReplaceBlockFeature::parse(json)?),
            "single_block_pillar" => Feature::SingleBlockPillar(Box::new(SingleBlockPillarFeature::parse(json)?)),
            "stepped_column_cluster" => Feature::SteppedColumnCluster(Box::new(SteppedColumnClusterFeature::parse(json)?)),
            "void_start_platform" => Feature::VoidStartPlatform,
            _ => Feature::NotImplemented,
        })
    }

    pub fn get_sub_features(&self, out: &mut Vec<u32>) {
        match self {
            Feature::Sequence(feature) => feature.get_sub_features(out),
            Feature::Overlay(feature) => feature.get_sub_features(out),
            Feature::RandomSelector(feature) => feature.get_sub_features(out),
            Feature::SimpleRandomSelector(feature) => feature.get_sub_features(out),
            Feature::RandomBooleanSelector(feature) => feature.get_sub_features(out),
            Feature::WeightedRandomSelector(feature) => feature.get_sub_features(out),
            _ => {}
        }
    }

    pub fn place<R: RandomSource>(&self, level: &mut impl WorldGenLevel, random: &mut R, origin: BlockPos) -> bool {
        match self {
            Feature::Tree(feature) => feature.place(level, random, origin),
            Feature::FallenTree(feature) => feature.place(level, random, origin),
            Feature::SimpleBlock(feature) => feature.place(level, random, origin),
            Feature::BlockColumn(feature) => feature.place(level, random, origin),
            Feature::BlockPile(feature) => feature.place(level, random, origin),
            Feature::VegetationPatch(feature) => feature.place(level, random, origin),
            Feature::Vines => vines_feature::place(level, origin),
            Feature::Disk(feature) => feature.place(level, random, origin),
            Feature::Spring(feature) => feature.place(level, origin),
            Feature::FreezeTopLayer => snow_and_freeze_feature::place(level, origin),
            Feature::Iceberg(feature) => feature.place(level, random, origin),
            Feature::Spike(feature) => feature.place(level, random, origin),
            Feature::BlueIce => blue_ice_feature::place(level, random, origin),
            Feature::Speleothem(feature) => feature.place(level, random, origin),
            Feature::SpeleothemCluster(feature) => feature.place(level, random, origin),
            Feature::LargeDripstone(feature) => feature.place(level, random, origin),
            Feature::Geode(feature) => feature.place(level, random, origin),
            Feature::Lake(feature) => feature.place(level, random, origin),
            Feature::MonsterRoom => monster_room_feature::place(level, random, origin),
            Feature::UnderwaterMagma(feature) => feature.place(level, random, origin),
            Feature::BlockBlob(feature) => feature.place(level, random, origin),
            Feature::CoralTree(feature) => feature.place(level, random, origin),
            Feature::CoralClaw(feature) => feature.place(level, random, origin),
            Feature::Sequence(feature) => feature.place(level, random, origin),
            Feature::Overlay(feature) => feature.place(level, random, origin),
            Feature::RootSystem(feature) => feature.place(level, random, origin),
            Feature::Template(feature) => feature.place(level, random, origin),
            Feature::Fossil(feature) => feature.place(level, random, origin),
            Feature::NoOp => true,
            Feature::FillLayer(feature) => feature.place(level, origin),
            Feature::SculkPatch(feature) => feature.place(level, random, origin),
            Feature::MultifaceGrowth(feature) => feature.place(level, random, origin),
            Feature::Bamboo(feature) => feature.place(level, random, origin),
            Feature::HugeMushroom(feature) => feature.place(level, random, origin),
            Feature::Ore(feature) => feature.place(level, random, origin),
            Feature::ScatteredOre(feature) => feature.place(level, random, origin),
            Feature::RandomSelector(feature) => feature.place(level, random, origin),
            Feature::SimpleRandomSelector(feature) => feature.place(level, random, origin),
            Feature::RandomBooleanSelector(feature) => feature.place(level, random, origin),
            Feature::WeightedRandomSelector(feature) => feature.place(level, random, origin),
            Feature::BonusChest => bonus_chest_feature::place(level, random, origin),
            Feature::ChorusPlant => chorus_plant_feature::place(level, random, origin),
            Feature::Delta(feature) => feature.place(level, random, origin),
            Feature::EndGateway(feature) => feature.place(level, origin),
            Feature::EndIsland => end_island_feature::place(level, random, origin),
            Feature::EndPlatform => end_platform_feature::place(level, origin),
            Feature::EndPodium(feature) => feature.place(level, origin),
            Feature::EndSpike(feature) => feature.place(level, random, origin),
            Feature::HugeFungus(feature) => feature.place(level, random, origin),
            Feature::ReplaceBlobs(feature) => feature.place(level, random, origin),
            Feature::ProjectedRandomPatchySquare(feature) => feature.place(level, random, origin),
            Feature::RandomNeighborSpread(feature) => feature.place(level, random, origin),
            Feature::ReplaceBlock(feature) => feature.place(level, random, origin),
            Feature::SingleBlockPillar(feature) => feature.place(level, random, origin),
            Feature::SteppedColumnCluster(feature) => feature.place(level, random, origin),
            Feature::VoidStartPlatform => void_start_platform_feature::place(level, origin),
            Feature::NotImplemented => false,
        }
    }
}

pub fn mark_above_for_post_processing(level: &mut impl WorldGenLevel, place_pos: BlockPos) {
    let mut pos = place_pos;
    for _ in 0..2 {
        pos = pos.above();
        if level.get_block_state(pos).is_air() {
            return;
        }
        level.mark_pos_for_post_processing(pos);
    }
}

pub enum FeatureHolder {
    Reference(u32),
    Direct(Box<Feature>),
}

impl FeatureHolder {
    pub fn parse(json: &Value) -> Result<FeatureHolder, String> {
        match json {
            Value::String(name) => feature_index(name).map(FeatureHolder::Reference).ok_or_else(|| crate::core::data_registries::unknown_reference("minecraft:worldgen/feature", &resource(name))),
            other => Ok(FeatureHolder::Direct(Box::new(Feature::parse(other)?))),
        }
    }

    pub fn get_features(&self, out: &mut Vec<u32>) {
        if let FeatureHolder::Reference(index) = self {
            out.push(*index);
        }
        self.value().get_sub_features(out);
    }

    pub fn value(&self) -> &Feature {
        match self {
            FeatureHolder::Reference(index) => &FEATURES[*index as usize],
            FeatureHolder::Direct(feature) => feature,
        }
    }
}

fn direct_codec_lazy() -> &'static crate::world::item::component::types::Pair {
    static CODEC: LazyLock<crate::world::item::component::types::Pair> = LazyLock::new(direct_codec);
    &CODEC
}

pub fn direct_codec() -> crate::world::item::component::types::Pair {
    use crate::world::item::component::codec::{Codec, Dispatch, Field, Selector, Stream};
    let no_fields = || -> Vec<Field> { Vec::new() };
    let cases = vec![
        ("minecraft:bamboo", super::bamboo_feature::fields()),
        ("minecraft:block_blob", super::block_blob_feature::fields()),
        ("minecraft:block_column", super::block_column_feature::fields()),
        ("minecraft:block_pile", super::block_pile_feature::fields()),
        ("minecraft:blue_ice", no_fields()),
        ("minecraft:bonus_chest", no_fields()),
        ("minecraft:chorus_plant", no_fields()),
        ("minecraft:coral_claw", super::coral_claw_feature::fields()),
        ("minecraft:coral_tree", super::coral_tree_feature::fields()),
        ("minecraft:delta_feature", super::delta_feature::fields()),
        ("minecraft:disk", super::disk_feature::fields()),
        ("minecraft:end_gateway", super::end_gateway_feature::fields()),
        ("minecraft:end_island", no_fields()),
        ("minecraft:end_platform", no_fields()),
        ("minecraft:end_podium", super::end_podium_feature::fields()),
        ("minecraft:end_spike", super::end_spike_feature::fields()),
        ("minecraft:fallen_tree", super::fallen_tree_feature::fields()),
        ("minecraft:fill_layer", super::fill_layer_feature::fields()),
        ("minecraft:fossil", super::fossil_feature::fields()),
        ("minecraft:freeze_top_layer", no_fields()),
        ("minecraft:geode", super::geode_feature::fields()),
        ("minecraft:huge_brown_mushroom", super::huge_brown_mushroom_feature::fields()),
        ("minecraft:huge_fungus", super::huge_fungus_feature::fields()),
        ("minecraft:huge_red_mushroom", super::huge_red_mushroom_feature::fields()),
        ("minecraft:iceberg", super::iceberg_feature::fields()),
        ("minecraft:lake", super::lake_feature::fields()),
        ("minecraft:large_dripstone", super::large_dripstone_feature::fields()),
        ("minecraft:monster_room", no_fields()),
        ("minecraft:multiface_growth", super::multiface_growth_feature::fields()),
        ("minecraft:netherrack_replace_blobs", super::replace_blobs_feature::fields()),
        ("minecraft:no_op", no_fields()),
        ("minecraft:ore", super::ore_feature::fields()),
        ("minecraft:overlay", super::overlay_feature::fields()),
        ("minecraft:projected_random_patchy_square", super::projected_random_patchy_square::fields()),
        ("minecraft:random_boolean_selector", super::random_boolean_selector_feature::fields()),
        ("minecraft:random_neighbor_spread", super::random_neighbor_spread_feature::fields()),
        ("minecraft:random_selector", super::random_selector_feature::fields()),
        ("minecraft:replace_single_block", super::replace_block_feature::fields()),
        ("minecraft:root_system", super::root_system_feature::fields()),
        ("minecraft:scattered_ore", super::scattered_ore_feature::fields()),
        ("minecraft:sculk_patch", super::sculk_patch_feature::fields()),
        ("minecraft:sequence", super::sequence_feature::fields()),
        ("minecraft:simple_block", super::simple_block_feature::fields()),
        ("minecraft:simple_random_selector", super::simple_random_selector_feature::fields()),
        ("minecraft:single_block_pillar", super::single_block_pillar_feature::fields()),
        ("minecraft:speleothem", super::speleothem_feature::fields()),
        ("minecraft:speleothem_cluster", super::speleothem_cluster_feature::fields()),
        ("minecraft:spike", super::spike_feature::fields()),
        ("minecraft:spring_feature", super::spring_feature::fields()),
        ("minecraft:stepped_column_cluster", super::stepped_column_cluster_feature::fields()),
        ("minecraft:template", super::template_feature::fields()),
        ("minecraft:tree", super::tree_feature::fields()),
        ("minecraft:underwater_magma", super::underwater_magma_feature::fields()),
        ("minecraft:vegetation_patch", super::vegetation_patch_feature::fields()),
        ("minecraft:vines", no_fields()),
        ("minecraft:void_start_platform", no_fields()),
        ("minecraft:waterlogged_vegetation_patch", super::vegetation_patch_feature::fields()),
        ("minecraft:weighted_random_selector", super::weighted_random_selector_feature::fields()),
    ];
    let dispatch = Codec::Dispatch(Box::new(Dispatch { key: "type", selector: Selector::Registry("minecraft:worldgen/feature_type"), cases }));
    (Codec::Normalize(Box::new(dispatch), validate), Stream::Dispatch)
}

fn validate(feature: wafer_protocol::nbt::Nbt) -> Result<wafer_protocol::nbt::Nbt, String> {
    match feature.get("type").and_then(wafer_protocol::nbt::Nbt::as_str) {
        Some("minecraft:fossil") => super::fossil_feature::validate(feature),
        _ => Ok(feature),
    }
}

pub fn holder_codec() -> crate::world::item::component::types::Pair {
    use crate::world::item::component::codec::{Codec, Stream};
    (Codec::Holder("minecraft:worldgen/feature", Some(Box::new(Codec::Lazy(direct_codec_lazy)))), Stream::Nbt)
}
