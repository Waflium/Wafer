use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::STRUCTURE_FEATURE;

const ENDCITY_TEMPLATES: &[(&str, &str)] = &[
    ("second_floor", "second_floor_1"),
    ("third_floor", "third_floor_1"),
    ("third_floor_c", "third_floor_2"),
];

const MANSION_TEMPLATES: &[(&str, &str)] = &[
    ("carpet_south", "carpet_south_1"),
    ("carpet_west", "carpet_west_1"),
    ("indoors_door", "indoors_door_1"),
    ("indoors_wall", "indoors_wall_1"),
];

const IGLOO_TEMPLATES: &[(&str, &str)] = &[
    ("minecraft:igloo/igloo_bottom", "minecraft:igloo/bottom"),
    ("minecraft:igloo/igloo_middle", "minecraft:igloo/middle"),
    ("minecraft:igloo/igloo_top", "minecraft:igloo/top"),
];

const OCEAN_RUIN_TEMPLATES: &[(&str, &str)] = &[
    ("minecraft:ruin/big_ruin1_brick", "minecraft:underwater_ruin/big_brick_1"),
    ("minecraft:ruin/big_ruin2_brick", "minecraft:underwater_ruin/big_brick_2"),
    ("minecraft:ruin/big_ruin3_brick", "minecraft:underwater_ruin/big_brick_3"),
    ("minecraft:ruin/big_ruin8_brick", "minecraft:underwater_ruin/big_brick_8"),
    ("minecraft:ruin/big_ruin1_cracked", "minecraft:underwater_ruin/big_cracked_1"),
    ("minecraft:ruin/big_ruin2_cracked", "minecraft:underwater_ruin/big_cracked_2"),
    ("minecraft:ruin/big_ruin3_cracked", "minecraft:underwater_ruin/big_cracked_3"),
    ("minecraft:ruin/big_ruin8_cracked", "minecraft:underwater_ruin/big_cracked_8"),
    ("minecraft:ruin/big_ruin1_mossy", "minecraft:underwater_ruin/big_mossy_1"),
    ("minecraft:ruin/big_ruin2_mossy", "minecraft:underwater_ruin/big_mossy_2"),
    ("minecraft:ruin/big_ruin3_mossy", "minecraft:underwater_ruin/big_mossy_3"),
    ("minecraft:ruin/big_ruin8_mossy", "minecraft:underwater_ruin/big_mossy_8"),
    ("minecraft:ruin/big_ruin_warm4", "minecraft:underwater_ruin/big_warm_4"),
    ("minecraft:ruin/big_ruin_warm5", "minecraft:underwater_ruin/big_warm_5"),
    ("minecraft:ruin/big_ruin_warm6", "minecraft:underwater_ruin/big_warm_6"),
    ("minecraft:ruin/big_ruin_warm7", "minecraft:underwater_ruin/big_warm_7"),
    ("minecraft:ruin/ruin1_brick", "minecraft:underwater_ruin/brick_1"),
    ("minecraft:ruin/ruin2_brick", "minecraft:underwater_ruin/brick_2"),
    ("minecraft:ruin/ruin3_brick", "minecraft:underwater_ruin/brick_3"),
    ("minecraft:ruin/ruin4_brick", "minecraft:underwater_ruin/brick_4"),
    ("minecraft:ruin/ruin5_brick", "minecraft:underwater_ruin/brick_5"),
    ("minecraft:ruin/ruin6_brick", "minecraft:underwater_ruin/brick_6"),
    ("minecraft:ruin/ruin7_brick", "minecraft:underwater_ruin/brick_7"),
    ("minecraft:ruin/ruin8_brick", "minecraft:underwater_ruin/brick_8"),
    ("minecraft:ruin/ruin1_cracked", "minecraft:underwater_ruin/cracked_1"),
    ("minecraft:ruin/ruin2_cracked", "minecraft:underwater_ruin/cracked_2"),
    ("minecraft:ruin/ruin3_cracked", "minecraft:underwater_ruin/cracked_3"),
    ("minecraft:ruin/ruin4_cracked", "minecraft:underwater_ruin/cracked_4"),
    ("minecraft:ruin/ruin5_cracked", "minecraft:underwater_ruin/cracked_5"),
    ("minecraft:ruin/ruin6_cracked", "minecraft:underwater_ruin/cracked_6"),
    ("minecraft:ruin/ruin7_cracked", "minecraft:underwater_ruin/cracked_7"),
    ("minecraft:ruin/ruin8_cracked", "minecraft:underwater_ruin/cracked_8"),
    ("minecraft:ruin/ruin1_mossy", "minecraft:underwater_ruin/mossy_1"),
    ("minecraft:ruin/ruin2_mossy", "minecraft:underwater_ruin/mossy_2"),
    ("minecraft:ruin/ruin3_mossy", "minecraft:underwater_ruin/mossy_3"),
    ("minecraft:ruin/ruin4_mossy", "minecraft:underwater_ruin/mossy_4"),
    ("minecraft:ruin/ruin5_mossy", "minecraft:underwater_ruin/mossy_5"),
    ("minecraft:ruin/ruin6_mossy", "minecraft:underwater_ruin/mossy_6"),
    ("minecraft:ruin/ruin7_mossy", "minecraft:underwater_ruin/mossy_7"),
    ("minecraft:ruin/ruin8_mossy", "minecraft:underwater_ruin/mossy_8"),
    ("minecraft:ruin/ruin_warm1", "minecraft:underwater_ruin/warm_1"),
    ("minecraft:ruin/ruin_warm2", "minecraft:underwater_ruin/warm_2"),
    ("minecraft:ruin/ruin_warm3", "minecraft:underwater_ruin/warm_3"),
    ("minecraft:ruin/ruin_warm4", "minecraft:underwater_ruin/warm_4"),
    ("minecraft:ruin/ruin_warm5", "minecraft:underwater_ruin/warm_5"),
    ("minecraft:ruin/ruin_warm6", "minecraft:underwater_ruin/warm_6"),
    ("minecraft:ruin/ruin_warm7", "minecraft:underwater_ruin/warm_7"),
    ("minecraft:ruin/ruin_warm8", "minecraft:underwater_ruin/warm_8"),
    ("minecraft:ruin/big_brick_1", "minecraft:underwater_ruin/big_brick_1"),
    ("minecraft:ruin/big_brick_2", "minecraft:underwater_ruin/big_brick_2"),
    ("minecraft:ruin/big_brick_3", "minecraft:underwater_ruin/big_brick_3"),
    ("minecraft:ruin/big_brick_8", "minecraft:underwater_ruin/big_brick_8"),
    ("minecraft:ruin/big_mossy_1", "minecraft:underwater_ruin/big_mossy_1"),
    ("minecraft:ruin/big_mossy_2", "minecraft:underwater_ruin/big_mossy_2"),
    ("minecraft:ruin/big_mossy_3", "minecraft:underwater_ruin/big_mossy_3"),
    ("minecraft:ruin/big_mossy_8", "minecraft:underwater_ruin/big_mossy_8"),
    ("minecraft:ruin/big_cracked_1", "minecraft:underwater_ruin/big_cracked_1"),
    ("minecraft:ruin/big_cracked_2", "minecraft:underwater_ruin/big_cracked_2"),
    ("minecraft:ruin/big_cracked_3", "minecraft:underwater_ruin/big_cracked_3"),
    ("minecraft:ruin/big_cracked_8", "minecraft:underwater_ruin/big_cracked_8"),
    ("minecraft:ruin/big_warm_4", "minecraft:underwater_ruin/big_warm_4"),
    ("minecraft:ruin/big_warm_5", "minecraft:underwater_ruin/big_warm_5"),
    ("minecraft:ruin/big_warm_6", "minecraft:underwater_ruin/big_warm_6"),
    ("minecraft:ruin/big_warm_7", "minecraft:underwater_ruin/big_warm_7"),
];

const RENAMES: [(&str, &str, &[(&str, &str)]); 4] = [
    ("EndCity", "ECP", ENDCITY_TEMPLATES),
    ("Mansion", "WMP", MANSION_TEMPLATES),
    ("Igloo", "Iglu", IGLOO_TEMPLATES),
    ("Ocean_Ruin", "ORP", OCEAN_RUIN_TEMPLATES),
];

pub struct ChunkStructuresTemplateRenameFix {
    base: DataFixBase,
}

impl ChunkStructuresTemplateRenameFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> ChunkStructuresTemplateRenameFix {
        ChunkStructuresTemplateRenameFix { base: DataFixBase::new(output_schema, changes_type) }
    }
}

impl DataFix for ChunkStructuresTemplateRenameFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let ty = self.get_input_schema().get_type(STRUCTURE_FEATURE);
        self.fix_type_everywhere_typed("ChunkStructuresTemplateRenameFix", ty, |input| input.update_remainder(fix_children))
    }
}

fn fix_children(structure: Dynamic) -> Dynamic {
    let creator = structure.clone();
    structure.update("Children", |children| creator.create_list(children.as_stream().into_iter().map(|child| fix_tag(&creator, child))))
}

fn fix_tag(structure: &Dynamic, child: Dynamic) -> Dynamic {
    let id = structure.get("id").as_string_or("");
    match RENAMES.iter().find(|(structure_id, _, _)| *structure_id == id) {
        Some((_, child_id, templates)) if *child_id == child.get("id").as_string_or("") => {
            let template = child.get("Template").as_string_or("");
            let renamed = templates.iter().find(|(from, _)| *from == template).map_or(template.as_str(), |(_, to)| to);
            let renamed = child.create_string(renamed);
            child.set("Template", renamed)
        }
        _ => child,
    }
}
