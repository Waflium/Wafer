use std::sync::Arc;

use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::Typed;
use crate::util::datafix::fixes::item_stack_componentization_fix;
use crate::util::datafix::fixes::named_entity_fix::NamedEntityFix;
use crate::util::datafix::fixes::references::BLOCK_ENTITY;

pub struct PlayerHeadBlockProfileFix;

impl PlayerHeadBlockProfileFix {
    pub fn new(output_schema: &Arc<Schema>) -> NamedEntityFix {
        NamedEntityFix::new(output_schema, false, "PlayerHeadBlockProfileFix", BLOCK_ENTITY, "minecraft:skull", fix)
    }
}

fn fix(entity: Typed) -> Typed {
    entity.update_remainder(fix_tag)
}

fn fix_tag(mut entity: Dynamic) -> Dynamic {
    let skull_owner = entity.get("SkullOwner").result();
    let extra_type = entity.get("ExtraType").result();
    let Some(profile) = skull_owner.or(extra_type) else { return entity };
    entity = entity.remove("SkullOwner").remove("ExtraType");
    entity.set("profile", item_stack_componentization_fix::fix_profile(profile))
}
