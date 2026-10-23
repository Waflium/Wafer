use std::sync::Arc;

use crate::datafixers::data_fix::DataFixBase;
use crate::datafixers::dsl::TypeReference;
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::{Typed, named_choice};

pub struct AbstractUUIDFix {
    pub base: DataFixBase,
    pub type_reference: TypeReference,
}

impl AbstractUUIDFix {
    pub fn new(output_schema: &Arc<Schema>, type_reference: TypeReference) -> AbstractUUIDFix {
        AbstractUUIDFix { base: DataFixBase::new(output_schema, false), type_reference }
    }

    pub fn update_named_choice(input_schema: &Schema, output_schema: &Schema, type_reference: TypeReference, input: Typed, name: &str, function: impl Fn(Dynamic) -> Dynamic) -> Typed {
        let old_type = input_schema.get_choice_type(type_reference, name);
        let new_type = output_schema.get_choice_type(type_reference, name);
        input.update_typed(&named_choice(name, old_type), Some(&new_type), |typed_tag| typed_tag.update_remainder(&function))
    }
}

pub fn replace_uuid_string(tag: &Dynamic, old_key: &str, new_key: &str) -> Option<Dynamic> {
    create_uuid_from_string(tag, old_key).map(|uuid_tag| tag.clone().remove(old_key).set(new_key, uuid_tag))
}

pub fn replace_uuid_ml_tag(tag: &Dynamic, old_key: &str, new_key: &str) -> Option<Dynamic> {
    tag.get(old_key).result().and_then(|value| create_uuid_from_ml(&value)).map(|uuid_tag| tag.clone().remove(old_key).set(new_key, uuid_tag))
}

pub fn replace_uuid_least_most(tag: &Dynamic, old_key: &str, new_key: &str) -> Option<Dynamic> {
    let most_key = format!("{old_key}Most");
    let least_key = format!("{old_key}Least");
    create_uuid_from_longs(tag, &most_key, &least_key).map(|uuid_tag| tag.clone().remove(&most_key).remove(&least_key).set(new_key, uuid_tag))
}

pub fn create_uuid_from_string(tag: &Dynamic, old_key: &str) -> Option<Dynamic> {
    let uuid_string_tag = tag.get(old_key).result()?;
    let uuid_string = uuid_string_tag.as_string().into_result().ok()?;
    let uuid = crate::commands::arguments::java_uuid_from_string(&uuid_string)?;
    create_uuid_tag(tag, (uuid >> 64) as i64, uuid as i64)
}

pub fn create_uuid_from_ml(tag: &Dynamic) -> Option<Dynamic> {
    create_uuid_from_longs(tag, "M", "L")
}

pub fn create_uuid_from_longs(tag: &Dynamic, most_key: &str, least_key: &str) -> Option<Dynamic> {
    let most_significant_bits = tag.get(most_key).as_long(0);
    let least_significant_bits = tag.get(least_key).as_long(0);
    if most_significant_bits != 0 && least_significant_bits != 0 { create_uuid_tag(tag, most_significant_bits, least_significant_bits) } else { None }
}

pub fn create_uuid_tag(tag: &Dynamic, most_significant_bits: i64, least_significant_bits: i64) -> Option<Dynamic> {
    Some(tag.create_int_list([
        (most_significant_bits >> 32) as i32,
        most_significant_bits as i32,
        (least_significant_bits >> 32) as i32,
        least_significant_bits as i32,
    ]))
}
