use wafer_core::random::RandomSource;
use wafer_core::{BlockPos, Direction};
use wafer_data::properties::DISTANCE;
use wafer_data::{BlockState, block_tags, blocks};
use serde_json::Value;

use super::featuresize::feature_size::{self, FeatureSize};
use super::foliageplacers::foliage_placer::{self, FoliagePlacer};
use super::rootplacers::root_placer::{self, RootPlacer};
use super::stateproviders::block_state_provider::{self, BlockStateProviderHolder};
use super::treedecorators::tree_decorator::{self, TreeDecorator};
use super::trunkplacers::trunk_placer::{self, TrunkPlacer};
use crate::world::item::component::codec::Field;
use crate::world::item::component::types::{self, lenient, opt, req};
use crate::world::level::world_gen_level::WorldGenLevel;
use crate::util::java_collections::JavaObjectHashSet;
use crate::world::level::block::leaves_block;
use crate::world::level::levelgen::structure::BoundingBox;
use crate::world::level::levelgen::structure::templatesystem::structure_template;
use wafer_core::shapes::BitSetDiscreteVoxelShape;

const BLOCK_UPDATE_FLAGS: i32 = 19;

pub struct TreeSetter<'s> {
    positions: &'s mut JavaObjectHashSet<BlockPos>,
}

impl<'s> TreeSetter<'s> {
    pub fn new(positions: &'s mut JavaObjectHashSet<BlockPos>) -> TreeSetter<'s> {
        TreeSetter { positions }
    }

    pub fn accept(&mut self, level: &mut impl WorldGenLevel, pos: BlockPos, state: BlockState) {
        self.positions.insert(pos);
        level.set_block(pos, state, BLOCK_UPDATE_FLAGS);
    }

    pub fn is_set(&self, pos: BlockPos) -> bool {
        self.positions.contains(&pos)
    }
}

pub struct TreeFeature {
    pub trunk_provider: BlockStateProviderHolder,
    pub trunk_placer: TrunkPlacer,
    pub foliage_provider: BlockStateProviderHolder,
    pub foliage_placer: FoliagePlacer,
    pub root_placer: Option<RootPlacer>,
    pub minimum_size: FeatureSize,
    pub decorators: Vec<TreeDecorator>,
    pub ignore_vines: bool,
    pub below_trunk_provider: BlockStateProviderHolder,
}

pub fn is_vine(level: &mut impl WorldGenLevel, pos: BlockPos) -> bool {
    level.get_block_state(pos).is_block(blocks::VINE)
}

pub fn is_air_or_leaves(level: &mut impl WorldGenLevel, pos: BlockPos) -> bool {
    let state = level.get_block_state(pos);
    state.is_air() || state.is(&block_tags::LEAVES)
}

fn set_block_known_shape(level: &mut impl WorldGenLevel, pos: BlockPos, state: BlockState) {
    level.set_block(pos, state, BLOCK_UPDATE_FLAGS);
}

pub fn valid_tree_pos(level: &mut impl WorldGenLevel, pos: BlockPos) -> bool {
    let state = level.get_block_state(pos);
    state.is_air() || state.is(&block_tags::REPLACEABLE_BY_TREES)
}

impl TreeFeature {
    pub fn parse(json: &Value) -> Result<TreeFeature, String> {
        let field = |key: &str| json.get(key).ok_or_else(|| crate::util::codec::no_key(key, json));
        Ok(TreeFeature {
            trunk_provider: BlockStateProviderHolder::parse(field("trunk_provider")?)?,
            trunk_placer: TrunkPlacer::parse(field("trunk_placer")?)?,
            foliage_provider: BlockStateProviderHolder::parse(field("foliage_provider")?)?,
            foliage_placer: FoliagePlacer::parse(field("foliage_placer")?)?,
            root_placer: json.get("root_placer").map(RootPlacer::parse).transpose()?,
            minimum_size: FeatureSize::parse(field("minimum_size")?)?,
            decorators: field("decorators")?.as_array().ok_or("decorators is a list")?.iter().map(TreeDecorator::parse).collect::<Result<_, _>>()?,
            ignore_vines: json.get("ignore_vines").and_then(Value::as_bool).unwrap_or(false),
            below_trunk_provider: BlockStateProviderHolder::parse(field("below_trunk_provider")?)?,
        })
    }

    fn do_place<R: RandomSource>(
        &self,
        level: &mut impl WorldGenLevel,
        random: &mut R,
        origin: BlockPos,
        root_setter: &mut TreeSetter,
        trunk_setter: &mut TreeSetter,
        foliage_setter: &mut TreeSetter,
    ) -> bool {
        let tree_height = self.trunk_placer.get_tree_height(random);
        let foliage_height = self.foliage_placer.foliage_height(random, tree_height, self);
        let trunk_height = tree_height - foliage_height;
        let leaf_radius = self.foliage_placer.foliage_radius(random, trunk_height);
        let trunk_origin = match &self.root_placer {
            Some(root_placer) => root_placer.get_trunk_origin(origin, random),
            None => origin,
        };
        let min_y = origin.y.min(trunk_origin.y);
        let max_y = origin.y.max(trunk_origin.y) + tree_height + 1;
        if min_y < level.get_min_y() + 1 || max_y > level.get_max_y() + 1 {
            return false;
        }
        let min_clipped_height = self.minimum_size.min_clipped_height();
        let clipped_tree_height = self.get_max_free_tree_height(level, tree_height, trunk_origin);
        if clipped_tree_height < tree_height && min_clipped_height.is_none_or(|min| clipped_tree_height < min) {
            return false;
        }
        if let Some(root_placer) = &self.root_placer
            && !root_placer.place_roots(level, root_setter, random, origin, trunk_origin, self)
        {
            return false;
        }
        let foliage_attachments = self.trunk_placer.place_trunk(level, trunk_setter, random, clipped_tree_height, trunk_origin, self);
        for foliage_attachment in &foliage_attachments {
            self.foliage_placer.create_foliage(level, foliage_setter, random, self, clipped_tree_height, foliage_attachment, foliage_height, leaf_radius);
        }
        true
    }

    fn get_max_free_tree_height(&self, level: &mut impl WorldGenLevel, max_tree_height: i32, tree_pos: BlockPos) -> i32 {
        for y in 0..=max_tree_height + 1 {
            let r = self.minimum_size.get_size_at_height(max_tree_height, y);
            for x in -r..=r {
                for z in -r..=r {
                    let block_pos = tree_pos.offset(x, y, z);
                    if !self.trunk_placer.is_free(level, block_pos) || !self.ignore_vines && is_vine(level, block_pos) {
                        return y - 2;
                    }
                }
            }
        }
        max_tree_height
    }

    pub fn place<R: RandomSource>(&self, level: &mut impl WorldGenLevel, random: &mut R, origin: BlockPos) -> bool {
        let mut root_positions = JavaObjectHashSet::default();
        let mut trunks = JavaObjectHashSet::default();
        let mut foliage = JavaObjectHashSet::default();
        let mut decorations = JavaObjectHashSet::default();
        let result = self.do_place(
            level,
            random,
            origin,
            &mut TreeSetter::new(&mut root_positions),
            &mut TreeSetter::new(&mut trunks),
            &mut TreeSetter::new(&mut foliage),
        );
        if !result || trunks.is_empty() && foliage.is_empty() {
            return false;
        }
        if !self.decorators.is_empty() {
            let mut context = tree_decorator::Context::new(level, TreeSetter::new(&mut decorations), random, &trunks, &foliage, &root_positions);
            for decorator in &self.decorators {
                decorator.place(&mut context);
            }
        }
        let Some(bounds) = encapsulating_positions(root_positions.iter().chain(trunks.iter()).chain(foliage.iter()).chain(decorations.iter())) else {
            return false;
        };
        let shape = update_leaves(level, &bounds, &trunks, &decorations, &root_positions);
        structure_template::update_shape_at_edge(level, 3, &shape, bounds.min.x, bounds.min.y, bounds.min.z);
        true
    }
}

fn encapsulating_positions(mut positions: impl Iterator<Item = BlockPos>) -> Option<BoundingBox> {
    let first = positions.next()?;
    let mut result = BoundingBox { min: first, max: first };
    for pos in positions {
        result.min = BlockPos::new(result.min.x.min(pos.x), result.min.y.min(pos.y), result.min.z.min(pos.z));
        result.max = BlockPos::new(result.max.x.max(pos.x), result.max.y.max(pos.y), result.max.z.max(pos.z));
    }
    Some(result)
}

fn update_leaves(
    level: &mut impl WorldGenLevel,
    bounds: &BoundingBox,
    logs: &JavaObjectHashSet<BlockPos>,
    decoration_set: &JavaObjectHashSet<BlockPos>,
    root_positions: &JavaObjectHashSet<BlockPos>,
) -> BitSetDiscreteVoxelShape {
    let mut shape = BitSetDiscreteVoxelShape::new(bounds.x_span(), bounds.y_span(), bounds.z_span());
    let mut to_check: Vec<JavaObjectHashSet<BlockPos>> = (0..7).map(|_| JavaObjectHashSet::default()).collect();
    for pos in decoration_set.iter().chain(root_positions.iter()) {
        if bounds.is_inside(pos) {
            shape.fill(pos.x - bounds.min.x, pos.y - bounds.min.y, pos.z - bounds.min.z);
        }
    }
    let mut smallest_distance = 0;
    for pos in logs.iter() {
        to_check[0].insert(pos);
    }
    loop {
        while smallest_distance >= 7 || !to_check[smallest_distance].is_empty() {
            if smallest_distance >= 7 {
                return shape;
            }
            let pos = to_check[smallest_distance].first().expect("a position to check");
            to_check[smallest_distance].remove(&pos);
            if bounds.is_inside(pos) {
                if smallest_distance != 0 {
                    let state = level.get_block_state(pos);
                    set_block_known_shape(level, pos, state.set(DISTANCE, smallest_distance as i32));
                }
                shape.fill(pos.x - bounds.min.x, pos.y - bounds.min.y, pos.z - bounds.min.z);
                for direction in Direction::VALUES {
                    let neighbor_pos = pos.relative(direction);
                    if bounds.is_inside(neighbor_pos) {
                        let x_in_shape = neighbor_pos.x - bounds.min.x;
                        let y_in_shape = neighbor_pos.y - bounds.min.y;
                        let z_in_shape = neighbor_pos.z - bounds.min.z;
                        if !shape.is_full(x_in_shape, y_in_shape, z_in_shape) {
                            let current_state = level.get_block_state(neighbor_pos);
                            if let Some(distance) = leaves_block::optional_distance_at(current_state) {
                                let new_distance = distance.min(smallest_distance as i32 + 1);
                                if new_distance < 7 {
                                    to_check[new_distance as usize].insert(neighbor_pos);
                                    smallest_distance = smallest_distance.min(new_distance as usize);
                                }
                            }
                        }
                    }
                }
            }
        }
        smallest_distance += 1;
    }
}

pub fn get_lowest_trunk_or_root_of_tree(roots: &[BlockPos], logs: &[BlockPos]) -> Vec<BlockPos> {
    let mut block_positions = Vec::new();
    if roots.is_empty() {
        block_positions.extend_from_slice(logs);
    } else if !logs.is_empty() && roots[0].y == logs[0].y {
        block_positions.extend_from_slice(logs);
        block_positions.extend_from_slice(roots);
    } else {
        block_positions.extend_from_slice(roots);
    }
    block_positions
}

pub fn fields() -> Vec<Field> {
    vec![
        req("trunk_provider", block_state_provider::holder_codec()),
        req("trunk_placer", trunk_placer::codec()),
        req("foliage_provider", block_state_provider::holder_codec()),
        req("foliage_placer", foliage_placer::codec()),
        opt("root_placer", root_placer::codec()),
        req("minimum_size", feature_size::codec()),
        req("decorators", tree_decorator::list_codec()),
        lenient("ignore_vines", types::bool(), types::b(false)),
        req("below_trunk_provider", block_state_provider::holder_codec()),
    ]
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::world::item::component::json_to_nbt;

    fn tree() -> serde_json::Value {
        json!({
            "type": "minecraft:tree",
            "trunk_provider": {"id": "minecraft:oak_log"},
            "trunk_placer": {"type": "minecraft:straight_trunk_placer", "base_height": 5, "height_rand_a": 2, "height_rand_b": 0},
            "foliage_provider": {"id": "minecraft:oak_leaves"},
            "foliage_placer": {"type": "minecraft:blob_foliage_placer", "radius": 2, "offset": 0, "height": 3},
            "minimum_size": {"type": "minecraft:two_layers_feature_size"},
            "decorators": [],
            "below_trunk_provider": {"type": "minecraft:simple", "state": {"id": "minecraft:dirt"}},
        })
    }

    fn decode(json: serde_json::Value) -> Result<(), String> {
        crate::nbt::with_json_source(|| super::super::feature::direct_codec().0.decode(&json_to_nbt(&json))).map(drop)
    }

    #[test]
    fn a_tree_with_every_optional_part_decodes() {
        let mut tree = tree();
        tree["ignore_vines"] = json!(true);
        tree["root_placer"] = json!({
            "type": "minecraft:mangrove_root_placer",
            "trunk_offset_y": {"type": "minecraft:uniform", "min_inclusive": 1, "max_inclusive": 3},
            "root_provider": {"id": "minecraft:mangrove_roots"},
            "above_root_placement": {"above_root_provider": {"id": "minecraft:moss_carpet"}, "above_root_placement_chance": 0.5},
            "mangrove_root_placement": {
                "can_grow_through": "minecraft:mud",
                "muddy_roots_in": ["minecraft:mud"],
                "muddy_roots_provider": {"id": "minecraft:muddy_mangrove_roots"},
                "max_root_width": 8,
                "max_root_length": 15,
                "random_skew_chance": 0.2,
            },
        });
        tree["decorators"] = json!([{"type": "minecraft:trunk_vine"}, {"type": "minecraft:beehive", "probability": 0.1}]);
        tree["trunk_placer"] = json!({
            "type": "minecraft:cherry_trunk_placer", "base_height": 7, "height_rand_a": 1, "height_rand_b": 0,
            "branch_count": {"type": "minecraft:uniform", "min_inclusive": 1, "max_inclusive": 3},
            "branch_horizontal_length": {"type": "minecraft:uniform", "min_inclusive": 2, "max_inclusive": 4},
            "branch_start_offset_from_top": {"min_inclusive": -4, "max_inclusive": -3},
            "branch_end_offset_from_top": 2,
        });
        assert_eq!(decode(tree), Ok(()));
    }

    #[test]
    fn errors_of_every_part_are_joined_in_reverse_field_order() {
        let mut tree = tree();
        tree["trunk_placer"]["base_height"] = json!(40);
        tree["foliage_placer"]["radius"] = json!(40);
        tree["minimum_size"]["limit"] = json!(100);
        tree["decorators"] = json!([{"type": "minecraft:cocoa", "probability": 5}]);
        assert_eq!(
            decode(tree).unwrap_err(),
            "Value 5.0 outside of range [0.0:1.0]; Value 100 outside of range [0:81]; Value provider too high: 16 [40-40]; Value 40 outside of range [0:32]"
        );
    }

    #[test]
    fn unknown_part_types_name_their_registries() {
        let mut tree = tree();
        tree["trunk_placer"] = json!({"type": "bogus"});
        tree["foliage_placer"] = json!({"type": "bogus"});
        tree["minimum_size"] = json!({"type": "bogus"});
        tree["decorators"] = json!([{"type": "bogus"}]);
        assert_eq!(
            decode(tree).unwrap_err(),
            "Unknown registry key in ResourceKey[minecraft:root / minecraft:worldgen/tree_decorator_type]: minecraft:bogus; Unknown registry key in ResourceKey[minecraft:root / minecraft:worldgen/feature_size_type]: minecraft:bogus; Unknown registry key in ResourceKey[minecraft:root / minecraft:worldgen/foliage_placer_type]: minecraft:bogus; Unknown registry key in ResourceKey[minecraft:root / minecraft:worldgen/trunk_placer_type]: minecraft:bogus"
        );
    }

    #[test]
    fn ignore_vines_falls_back_to_false_whatever_it_holds() {
        let mut tree = tree();
        tree["ignore_vines"] = json!("x");
        assert_eq!(decode(tree), Ok(()));
    }

    #[test]
    fn cherry_branch_starts_are_validated_in_three_steps() {
        let start = |start: serde_json::Value| {
            let mut tree = tree();
            tree["trunk_placer"] = json!({
                "type": "minecraft:cherry_trunk_placer", "base_height": 7, "height_rand_a": 1, "height_rand_b": 0,
                "branch_count": 1, "branch_horizontal_length": 2, "branch_start_offset_from_top": start, "branch_end_offset_from_top": 2,
            });
            decode(tree).unwrap_err()
        };
        assert_eq!(start(json!({"min_inclusive": -1, "max_inclusive": -4})), "Max must be at least min, min_inclusive: -1, max_inclusive: -4");
        assert_eq!(start(json!({"min_inclusive": -4, "max_inclusive": -4})), "Need at least 2 blocks variation for the branch starts to fit both branches");
        assert_eq!(start(json!({"min_inclusive": -17, "max_inclusive": -15})), "Value provider too low: -16 [-17--15]");
        assert_eq!(start(json!({"min_inclusive": -4, "max_inclusive": 1})), "Value provider too high: 0 [-4-1]");
        assert_eq!(start(json!(-3)), "Not a JSON object: -3");
    }
}
