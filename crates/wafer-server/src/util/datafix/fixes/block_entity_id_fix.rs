use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::value::Value;
use crate::util::datafix::fixes::references::{BLOCK_ENTITY, ITEM_STACK};

pub static ID_MAP: [(&str, &str); 23] = [
    ("Airportal", "minecraft:end_portal"),
    ("Banner", "minecraft:banner"),
    ("Beacon", "minecraft:beacon"),
    ("Cauldron", "minecraft:brewing_stand"),
    ("Chest", "minecraft:chest"),
    ("Comparator", "minecraft:comparator"),
    ("Control", "minecraft:command_block"),
    ("DLDetector", "minecraft:daylight_detector"),
    ("Dropper", "minecraft:dropper"),
    ("EnchantTable", "minecraft:enchanting_table"),
    ("EndGateway", "minecraft:end_gateway"),
    ("EnderChest", "minecraft:ender_chest"),
    ("FlowerPot", "minecraft:flower_pot"),
    ("Furnace", "minecraft:furnace"),
    ("Hopper", "minecraft:hopper"),
    ("MobSpawner", "minecraft:mob_spawner"),
    ("Music", "minecraft:noteblock"),
    ("Piston", "minecraft:piston"),
    ("RecordPlayer", "minecraft:jukebox"),
    ("Sign", "minecraft:sign"),
    ("Skull", "minecraft:skull"),
    ("Structure", "minecraft:structure_block"),
    ("Trap", "minecraft:dispenser"),
];

pub struct BlockEntityIdFix {
    base: DataFixBase,
}

impl BlockEntityIdFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> BlockEntityIdFix {
        BlockEntityIdFix { base: DataFixBase::new(output_schema, changes_type) }
    }
}

impl DataFix for BlockEntityIdFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let old_item_stack_type = self.get_input_schema().get_type(ITEM_STACK);
        let new_item_stack_type = self.get_output_schema().get_type(ITEM_STACK);
        let old_type = self.get_input_schema().find_choice_type(BLOCK_ENTITY);
        let new_type = self.get_output_schema().find_choice_type(BLOCK_ENTITY);
        TypeRewriteRule::seq(vec![
            self.convert_unchecked("item stack block entity name hook converter", old_item_stack_type, new_item_stack_type),
            self.fix_type_everywhere_to("BlockEntityIdFix", old_type, new_type, |input| match input {
                Value::Choice(id, value) => Value::Choice(ID_MAP.iter().find(|(old, _)| *old == id).map_or(id, |(_, new)| new.to_string()), value),
                other => other,
            }),
        ])
    }
}
