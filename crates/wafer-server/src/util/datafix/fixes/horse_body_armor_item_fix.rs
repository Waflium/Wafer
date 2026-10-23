use std::sync::Arc;

use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::named_entity_write_read_fix::NamedEntityWriteReadFix;
use crate::util::datafix::fixes::references::ENTITY;

pub struct HorseBodyArmorItemFix;

impl HorseBodyArmorItemFix {
    pub fn new(output_schema: &Arc<Schema>, entity_name: &str, previous_body_armor_tag: &str, clear_armor_items: bool) -> NamedEntityWriteReadFix {
        let previous_body_armor_tag = previous_body_armor_tag.to_string();
        NamedEntityWriteReadFix::new(output_schema, true, &format!("Horse armor fix for {entity_name}"), ENTITY, entity_name, move |input| {
            fix(input, &previous_body_armor_tag, clear_armor_items)
        })
    }
}

fn fix(input: Dynamic, previous_body_armor_tag: &str, clear_armor_items: bool) -> Dynamic {
    let Some(body_armor_item) = input.get(previous_body_armor_tag).result() else { return input };
    let body_armor_drop_chance = input.create_float(2.0);
    let mut output = input.remove(previous_body_armor_tag);
    if clear_armor_items {
        output = output.update("ArmorItems", |armor_items| {
            let entries = armor_items.as_stream();
            armor_items.create_list(entries.into_iter().enumerate().map(|(index, entry)| if index == 2 { entry.empty_map() } else { entry }))
        });
        output = output.update("ArmorDropChances", |armor_drop_chances| {
            let entries = armor_drop_chances.as_stream();
            armor_drop_chances.create_list(entries.into_iter().enumerate().map(|(index, entry)| if index == 2 { entry.create_float(0.085) } else { entry }))
        });
    }
    output = output.set("body_armor_item", body_armor_item);
    output.set("body_armor_drop_chance", body_armor_drop_chance)
}
