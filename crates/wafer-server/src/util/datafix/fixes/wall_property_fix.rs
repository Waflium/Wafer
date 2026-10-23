use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::BLOCK_STATE;

const WALL_BLOCKS: [&str; 14] = [
    "minecraft:andesite_wall",
    "minecraft:brick_wall",
    "minecraft:cobblestone_wall",
    "minecraft:diorite_wall",
    "minecraft:end_stone_brick_wall",
    "minecraft:granite_wall",
    "minecraft:mossy_cobblestone_wall",
    "minecraft:mossy_stone_brick_wall",
    "minecraft:nether_brick_wall",
    "minecraft:prismarine_wall",
    "minecraft:red_nether_brick_wall",
    "minecraft:red_sandstone_wall",
    "minecraft:sandstone_wall",
    "minecraft:stone_brick_wall",
];

pub struct WallPropertyFix {
    base: DataFixBase,
}

impl WallPropertyFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> WallPropertyFix {
        WallPropertyFix { base: DataFixBase::new(output_schema, changes_type) }
    }
}

impl DataFix for WallPropertyFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        self.fix_type_everywhere_typed("WallPropertyFix", self.get_input_schema().get_type(BLOCK_STATE), |input| input.update_remainder(upgrade_block_state_tag))
    }
}

fn map_property(value: &str) -> &'static str {
    if value == "true" { "low" } else { "none" }
}

fn fix_wall_property(state: Dynamic, property: &str) -> Dynamic {
    state.update(property, |value| match value.as_string().into_result() {
        Ok(string) => value.create_string(map_property(&string)),
        Err(_) => value,
    })
}

fn upgrade_block_state_tag(state: Dynamic) -> Dynamic {
    let is_wall = state.get("Name").as_string().into_result().is_ok_and(|name| WALL_BLOCKS.contains(&name.as_str()));
    if !is_wall {
        return state;
    }
    state.update("Properties", |properties| {
        let new_state = fix_wall_property(properties, "east");
        let new_state = fix_wall_property(new_state, "west");
        let new_state = fix_wall_property(new_state, "north");
        fix_wall_property(new_state, "south")
    })
}
