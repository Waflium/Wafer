use wafer_core::random::RandomSource;

use super::foliage_placer::{FoliageAttachment, FoliagePlacer};
use crate::world::level::world_gen_level::WorldGenLevel;
use crate::world::level::levelgen::feature::tree_feature::{TreeFeature, TreeSetter};

pub fn blob_parts() -> Vec<crate::world::item::component::codec::Field> {
    use crate::world::item::component::codec::{Codec, RangeError, Stream};
    use crate::world::item::component::types::req;
    let mut fields = super::foliage_placer::foliage_placer_parts();
    fields.push(req("height", (Codec::Int(0, 16, RangeError::Outside), Stream::Int)));
    fields
}

pub fn fields() -> Vec<crate::world::item::component::codec::Field> {
    blob_parts()
}

#[allow(clippy::too_many_arguments)]
pub fn create_foliage<R: RandomSource>(
    placer: &FoliagePlacer,
    level: &mut impl WorldGenLevel,
    foliage_setter: &mut TreeSetter,
    random: &mut R,
    tree: &TreeFeature,
    foliage_attachment: &FoliageAttachment,
    foliage_height: i32,
    leaf_radius: i32,
    offset: i32,
) {
    let foliage_height_with_offset = foliage_height + foliage_attachment.foliage_height_offset;
    let mut yo = offset;
    while yo >= offset - foliage_height_with_offset {
        let current_radius = (leaf_radius + foliage_attachment.radius_offset_xz - 1 - yo / 2).max(0);
        placer.place_leaves_row(level, foliage_setter, random, tree, foliage_attachment.pos, current_radius, yo, foliage_attachment.double_trunk());
        yo -= 1;
    }
}

pub fn should_skip_location(random: &mut impl RandomSource, dx: i32, y: i32, dz: i32, current_radius: i32) -> bool {
    dx == current_radius && dz == current_radius && (random.next_int_bounded(2) == 0 || y == 0)
}
