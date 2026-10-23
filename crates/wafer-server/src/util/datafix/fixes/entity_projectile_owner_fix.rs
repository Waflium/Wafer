use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::abstract_uuid_fix::NamedChoiceUpdate;
use crate::util::datafix::fixes::references::ENTITY;

pub struct EntityProjectileOwnerFix {
    base: DataFixBase,
}

impl EntityProjectileOwnerFix {
    pub fn new(output_schema: &Arc<Schema>) -> EntityProjectileOwnerFix {
        EntityProjectileOwnerFix { base: DataFixBase::new(output_schema, false) }
    }
}

impl DataFix for EntityProjectileOwnerFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let input_schema = self.get_input_schema();
        let functions: [(&str, fn(Dynamic) -> Dynamic); 9] = [
            ("minecraft:egg", update_owner_throwable),
            ("minecraft:ender_pearl", update_owner_throwable),
            ("minecraft:experience_bottle", update_owner_throwable),
            ("minecraft:snowball", update_owner_throwable),
            ("minecraft:potion", update_owner_throwable),
            ("minecraft:llama_spit", update_owner_llama_spit),
            ("minecraft:arrow", update_owner_arrow),
            ("minecraft:spectral_arrow", update_owner_arrow),
            ("minecraft:trident", update_owner_arrow),
        ];
        let updates: Vec<(NamedChoiceUpdate, fn(Dynamic) -> Dynamic)> =
            functions.iter().map(|(name, function)| (NamedChoiceUpdate::new(input_schema, self.get_output_schema(), ENTITY, name), *function)).collect();
        self.fix_type_everywhere_typed("EntityProjectileOwner", input_schema.get_type(ENTITY), move |input| {
            updates.iter().fold(input, |input, (update, function)| update.update(input, function))
        })
    }
}

fn update_owner_arrow(tag: Dynamic) -> Dynamic {
    let most_significant_bits = tag.get("OwnerUUIDMost").as_long(0);
    let least_significant_bits = tag.get("OwnerUUIDLeast").as_long(0);
    set_uuid(tag, most_significant_bits, least_significant_bits).remove("OwnerUUIDMost").remove("OwnerUUIDLeast")
}

fn update_owner_llama_spit(tag: Dynamic) -> Dynamic {
    let most_significant_bits = tag.get("Owner").get("OwnerUUIDMost").as_long(0);
    let least_significant_bits = tag.get("Owner").get("OwnerUUIDLeast").as_long(0);
    set_uuid(tag, most_significant_bits, least_significant_bits).remove("Owner")
}

fn update_owner_throwable(tag: Dynamic) -> Dynamic {
    let most_significant_bits = tag.get("owner").get("M").as_long(0);
    let least_significant_bits = tag.get("owner").get("L").as_long(0);
    set_uuid(tag, most_significant_bits, least_significant_bits).remove("owner")
}

fn set_uuid(tag: Dynamic, most_significant_bits: i64, least_significant_bits: i64) -> Dynamic {
    if most_significant_bits != 0 && least_significant_bits != 0 {
        let uuid = tag.create_int_list(create_uuid_array(most_significant_bits, least_significant_bits));
        tag.set("OwnerUUID", uuid)
    } else {
        tag
    }
}

fn create_uuid_array(most_significant_bits: i64, least_significant_bits: i64) -> [i32; 4] {
    [(most_significant_bits >> 32) as i32, most_significant_bits as i32, (least_significant_bits >> 32) as i32, least_significant_bits as i32]
}
