use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::BLOCK_STATE;

pub struct RedstoneWireConnectionsFix {
    base: DataFixBase,
}

impl RedstoneWireConnectionsFix {
    pub fn new(output_schema: &Arc<Schema>) -> RedstoneWireConnectionsFix {
        RedstoneWireConnectionsFix { base: DataFixBase::new(output_schema, false) }
    }
}

impl DataFix for RedstoneWireConnectionsFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let input_schema = self.get_input_schema();
        self.fix_type_everywhere_typed("RedstoneConnectionsFix", input_schema.get_type(BLOCK_STATE), |input| input.update_remainder(update_redstone_connections))
    }
}

fn update_redstone_connections(state: Dynamic) -> Dynamic {
    let is_redstone = state.get("Name").as_string().into_result().is_ok_and(|name| name == "minecraft:redstone_wire");
    if !is_redstone {
        return state;
    }
    state.update("Properties", |props| {
        let east = props.get("east").as_string_or("none");
        let west = props.get("west").as_string_or("none");
        let north = props.get("north").as_string_or("none");
        let south = props.get("south").as_string_or("none");
        let eastwest = is_connected(&east) || is_connected(&west);
        let northsouth = is_connected(&north) || is_connected(&south);
        let new_east = if !is_connected(&east) && !northsouth { "side" } else { east.as_str() };
        let new_west = if !is_connected(&west) && !northsouth { "side" } else { west.as_str() };
        let new_north = if !is_connected(&north) && !eastwest { "side" } else { north.as_str() };
        let new_south = if !is_connected(&south) && !eastwest { "side" } else { south.as_str() };
        props
            .update("east", |value| value.create_string(new_east))
            .update("west", |value| value.create_string(new_west))
            .update("north", |value| value.create_string(new_north))
            .update("south", |value| value.create_string(new_south))
    })
}

fn is_connected(connection_type: &str) -> bool {
    connection_type != "none"
}
