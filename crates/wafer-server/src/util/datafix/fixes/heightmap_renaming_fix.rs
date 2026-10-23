use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::find_field;
use crate::util::datafix::fixes::references::CHUNK;

pub struct HeightmapRenamingFix {
    base: DataFixBase,
}

impl HeightmapRenamingFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> HeightmapRenamingFix {
        HeightmapRenamingFix { base: DataFixBase::new(output_schema, changes_type) }
    }
}

impl DataFix for HeightmapRenamingFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let input_type = self.get_input_schema().get_type(CHUNK);
        let level_f = find_field(&input_type, "Level");
        self.fix_type_everywhere_typed("HeightmapRenamingFix", input_type, move |input| input.update_typed(&level_f, None, |level| level.update_remainder(fix)))
    }
}

fn fix(tag: Dynamic) -> Dynamic {
    let Some(heightmaps) = tag.get("Heightmaps").result() else { return tag };
    let mut heightmaps_tag = heightmaps;
    if let Some(liquid) = heightmaps_tag.get("LIQUID").result() {
        heightmaps_tag = heightmaps_tag.remove("LIQUID");
        heightmaps_tag = heightmaps_tag.set("WORLD_SURFACE_WG", liquid);
    }
    if let Some(solid) = heightmaps_tag.get("SOLID").result() {
        heightmaps_tag = heightmaps_tag.remove("SOLID");
        heightmaps_tag = heightmaps_tag.set("OCEAN_FLOOR_WG", solid.clone());
        heightmaps_tag = heightmaps_tag.set("OCEAN_FLOOR", solid);
    }
    if let Some(light) = heightmaps_tag.get("LIGHT").result() {
        heightmaps_tag = heightmaps_tag.remove("LIGHT");
        heightmaps_tag = heightmaps_tag.set("LIGHT_BLOCKING", light);
    }
    if let Some(rain) = heightmaps_tag.get("RAIN").result() {
        heightmaps_tag = heightmaps_tag.remove("RAIN");
        heightmaps_tag = heightmaps_tag.set("MOTION_BLOCKING", rain.clone());
        heightmaps_tag = heightmaps_tag.set("MOTION_BLOCKING_NO_LEAVES", rain);
    }
    tag.set("Heightmaps", heightmaps_tag)
}
