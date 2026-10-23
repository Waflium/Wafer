use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::find_field;
use crate::util::datafix::fixes::references::{PLAYER, SAVED_DATA_MAP_DATA};

pub struct LegacyDimensionIdFix {
    base: DataFixBase,
}

impl LegacyDimensionIdFix {
    pub fn new(output_schema: &Arc<Schema>) -> LegacyDimensionIdFix {
        LegacyDimensionIdFix { base: DataFixBase::new(output_schema, false) }
    }
}

impl DataFix for LegacyDimensionIdFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let player_rule = self.fix_type_everywhere_typed("PlayerLegacyDimensionFix", self.get_input_schema().get_type(PLAYER), |input| input.update_remainder(fix_player));
        let data_type = self.get_input_schema().get_type(SAVED_DATA_MAP_DATA);
        let map_data_finder = find_field(&data_type, "data");
        let map_rule = self.fix_type_everywhere_typed("MapLegacyDimensionFix", data_type, move |input| {
            input.update_typed(&map_data_finder, None, |data| data.update_remainder(fix_map))
        });
        TypeRewriteRule::seq(vec![player_rule, map_rule])
    }
}

fn fix_map(remainder: Dynamic) -> Dynamic {
    remainder.update("dimension", fix_dimension_id)
}

fn fix_player(remainder: Dynamic) -> Dynamic {
    remainder.update("Dimension", fix_dimension_id)
}

fn fix_dimension_id(id: Dynamic) -> Dynamic {
    match id.as_number().into_result() {
        Ok(legacy_id) => match legacy_id.int_value() {
            -1 => id.create_string("minecraft:the_nether"),
            1 => id.create_string("minecraft:the_end"),
            _ => id.create_string("minecraft:overworld"),
        },
        Err(_) => id,
    }
}
