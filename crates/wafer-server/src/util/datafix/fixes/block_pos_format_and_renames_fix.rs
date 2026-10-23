use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dsl::TypeReference;
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::{Typed, named_choice};
use crate::util::datafix::extra_data_fix_utils;
use crate::util::datafix::fixes::item_stack_tag_fix::ItemStackTagFix;
use crate::util::datafix::fixes::references::{BLOCK_ENTITY, ENTITY, ITEM_STACK, SAVED_DATA_MAP_DATA};

const PATROLLING_MOBS: [&str; 6] = ["minecraft:witch", "minecraft:ravager", "minecraft:pillager", "minecraft:illusioner", "minecraft:evoker", "minecraft:vindicator"];

pub struct BlockPosFormatAndRenamesFix {
    base: DataFixBase,
}

impl BlockPosFormatAndRenamesFix {
    pub fn new(output_schema: &Arc<Schema>) -> BlockPosFormatAndRenamesFix {
        BlockPosFormatAndRenamesFix { base: DataFixBase::new(output_schema, true) }
    }

    fn add_entity_rules(&self, rules: &mut Vec<TypeRewriteRule>) {
        rules.push(self.create_entity_fixer(ENTITY, "minecraft:bee", &[("HivePos", "hive_pos"), ("FlowerPos", "flower_pos")]));
        rules.push(self.create_entity_fixer(ENTITY, "minecraft:end_crystal", &[("BeamTarget", "beam_target")]));
        rules.push(self.create_entity_fixer(ENTITY, "minecraft:wandering_trader", &[("WanderTarget", "wander_target")]));
        for patrolling_mob in PATROLLING_MOBS {
            rules.push(self.create_entity_fixer(ENTITY, patrolling_mob, &[("PatrolTarget", "patrol_target")]));
        }
        rules.push(self.fix_type_everywhere_typed("BlockPos format in Leash for mobs", self.get_input_schema().get_type(ENTITY), |input| {
            input.update_remainder(|tag| tag.rename_and_fix_field("Leash", "leash", extra_data_fix_utils::fix_block_pos))
        }));
    }

    fn add_block_entity_rules(&self, rules: &mut Vec<TypeRewriteRule>) {
        rules.push(self.create_entity_fixer(BLOCK_ENTITY, "minecraft:beehive", &[("FlowerPos", "flower_pos")]));
        rules.push(self.create_entity_fixer(BLOCK_ENTITY, "minecraft:end_gateway", &[("ExitPortal", "exit_portal")]));
    }

    fn create_entity_fixer(&self, ty: TypeReference, entity_name: &str, fields: &[(&'static str, &'static str)]) -> TypeRewriteRule {
        let keys: Vec<&str> = fields.iter().map(|(from, _)| *from).collect();
        let name = format!("BlockPos format in [{}] for {} ({})", keys.join(", "), entity_name, ty.type_name());
        let entity_f = named_choice(entity_name, self.get_input_schema().get_choice_type(ty, entity_name));
        let fields = fields.to_vec();
        self.fix_type_everywhere_typed(&name, self.get_input_schema().get_type(ty), move |input| input.update_typed(&entity_f, None, |entity| fix_fields(entity, &fields)))
    }
}

fn fix_fields(typed: Typed, fields: &[(&str, &str)]) -> Typed {
    typed.update_remainder(|mut tag| {
        for (from, to) in fields {
            tag = tag.rename_and_fix_field(from, to, extra_data_fix_utils::fix_block_pos);
        }
        tag
    })
}

fn fix_map_saved_data(data: Dynamic) -> Dynamic {
    data.update("frames", |frames| {
        let entries = frames.as_stream();
        frames.create_list(entries.into_iter().map(|mut frame| {
            frame = frame.rename_and_fix_field("Pos", "pos", extra_data_fix_utils::fix_block_pos);
            frame = frame.rename_field("Rotation", "rotation");
            frame.rename_field("EntityId", "entity_id")
        }))
    })
    .update("banners", |banners| {
        let entries = banners.as_stream();
        banners.create_list(entries.into_iter().map(|mut banner| {
            banner = banner.rename_field("Pos", "pos");
            banner = banner.rename_field("Color", "color");
            banner.rename_field("Name", "name")
        }))
    })
}

impl DataFix for BlockPosFormatAndRenamesFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let mut rules = Vec::new();
        self.add_entity_rules(&mut rules);
        self.add_block_entity_rules(&mut rules);
        rules.push(self.write_fix_and_read(
            "BlockPos format for map frames",
            self.get_input_schema().get_type(SAVED_DATA_MAP_DATA),
            self.get_output_schema().get_type(SAVED_DATA_MAP_DATA),
            |input| input.update("data", fix_map_saved_data),
        ));
        let item_stack_type = self.get_input_schema().get_type(ITEM_STACK);
        rules.push(self.fix_type_everywhere_typed(
            "BlockPos format for compass target",
            item_stack_type.clone(),
            ItemStackTagFix::create_fixer(
                &item_stack_type,
                |id| id == "minecraft:compass",
                |typed| typed.update_remainder(|tag| tag.update("LodestonePos", extra_data_fix_utils::fix_block_pos)),
            ),
        ));
        TypeRewriteRule::seq(rules)
    }
}
