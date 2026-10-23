use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::named_choice;
use crate::util::datafix::fixes::references::ENTITY;

const DIRECTIONS: [[i32; 3]; 4] = [[0, 0, 1], [-1, 0, 0], [0, 0, -1], [1, 0, 0]];

pub struct EntityPaintingItemFrameDirectionFix {
    base: DataFixBase,
}

impl EntityPaintingItemFrameDirectionFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> EntityPaintingItemFrameDirectionFix {
        EntityPaintingItemFrameDirectionFix { base: DataFixBase::new(output_schema, changes_type) }
    }
}

fn do_fix(mut input: Dynamic, is_painting: bool, is_item_frame: bool) -> Dynamic {
    if (is_painting || is_item_frame) && !input.get("Facing").as_number().is_success() {
        let direction;
        if input.get("Direction").as_number().is_success() {
            direction = input.get("Direction").as_byte(0) as i32 % DIRECTIONS.len() as i32;
            let steps = DIRECTIONS[direction as usize];
            let tile_x = input.create_int(input.get("TileX").as_int(0) + steps[0]);
            input = input.set("TileX", tile_x);
            let tile_y = input.create_int(input.get("TileY").as_int(0) + steps[1]);
            input = input.set("TileY", tile_y);
            let tile_z = input.create_int(input.get("TileZ").as_int(0) + steps[2]);
            input = input.set("TileZ", tile_z);
            input = input.remove("Direction");
            if is_item_frame && input.get("ItemRotation").as_number().is_success() {
                let rotation = input.create_byte((input.get("ItemRotation").as_byte(0) as i32 * 2) as i8);
                input = input.set("ItemRotation", rotation);
            }
        } else {
            direction = input.get("Dir").as_byte(0) as i32 % DIRECTIONS.len() as i32;
            input = input.remove("Dir");
        }
        let facing = input.create_byte(direction as i8);
        input = input.set("Facing", facing);
    }
    input
}

impl DataFix for EntityPaintingItemFrameDirectionFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let painting_type = self.get_input_schema().get_choice_type(ENTITY, "Painting");
        let painting_finder = named_choice("Painting", painting_type.clone());
        let item_frame_type = self.get_input_schema().get_choice_type(ENTITY, "ItemFrame");
        let item_frame_finder = named_choice("ItemFrame", item_frame_type.clone());
        let entity_type = self.get_input_schema().get_type(ENTITY);
        let painting_rule = self.fix_type_everywhere_typed("EntityPaintingFix", entity_type.clone(), move |input| {
            input.update_typed(&painting_finder, Some(&painting_type), |entity| entity.update_remainder(|tag| do_fix(tag, true, false)))
        });
        let item_frame_rule = self.fix_type_everywhere_typed("EntityItemFrameFix", entity_type, move |input| {
            input.update_typed(&item_frame_finder, Some(&item_frame_type), |entity| entity.update_remainder(|tag| do_fix(tag, false, true)))
        });
        TypeRewriteRule::seq(vec![painting_rule, item_frame_rule])
    }
}
