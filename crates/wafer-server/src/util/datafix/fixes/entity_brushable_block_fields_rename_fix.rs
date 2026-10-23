use std::sync::Arc;

use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::Typed;
use crate::util::datafix::fixes::named_entity_fix::NamedEntityFix;
use crate::util::datafix::fixes::references::BLOCK_ENTITY;

pub struct EntityBrushableBlockFieldsRenameFix;

impl EntityBrushableBlockFieldsRenameFix {
    pub fn new(output_schema: &Arc<Schema>) -> NamedEntityFix {
        NamedEntityFix::new(output_schema, false, "EntityBrushableBlockFieldsRenameFix", BLOCK_ENTITY, "minecraft:brushable_block", fix)
    }
}

fn fix_tag(input: Dynamic) -> Dynamic {
    input.rename_field("loot_table", "LootTable").rename_field("loot_table_seed", "LootTableSeed")
}

fn fix(entity: Typed) -> Typed {
    entity.update_remainder(fix_tag)
}
