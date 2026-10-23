use std::sync::Arc;

use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::typed::{OpticFinder, Typed};
use crate::datafixers::types::{self, Type};

pub fn fix_block_pos(pos: Dynamic) -> Dynamic {
    let x = pos.get("X").as_number().into_result();
    let y = pos.get("Y").as_number().into_result();
    let z = pos.get("Z").as_number().into_result();
    match (x, y, z) {
        (Ok(x), Ok(y), Ok(z)) => create_block_pos(&pos, x.int_value(), y.int_value(), z.int_value()),
        _ => pos,
    }
}

pub fn fix_inline_block_pos(input: Dynamic, field_x: &str, field_y: &str, field_z: &str, new_field: &str) -> Dynamic {
    let x = input.get(field_x).as_number().into_result();
    let y = input.get(field_y).as_number().into_result();
    let z = input.get(field_z).as_number().into_result();
    match (x, y, z) {
        (Ok(x), Ok(y), Ok(z)) => {
            let pos = create_block_pos(&input, x.int_value(), y.int_value(), z.int_value());
            input.remove(field_x).remove(field_y).remove(field_z).set(new_field, pos)
        }
        _ => input,
    }
}

pub fn create_block_pos(dynamic: &Dynamic, x: i32, y: i32, z: i32) -> Dynamic {
    dynamic.create_int_list([x, y, z])
}

pub fn cast(ty: &Arc<Type>, typed: Typed) -> Typed {
    typed.with_type(ty.clone())
}

pub fn patch_sub_type(ty: &Arc<Type>, find: &Arc<Type>, replace: &Arc<Type>) -> Arc<Type> {
    types::patch_sub_type(ty, find, replace)
}

pub fn fix_string_field(dynamic: Dynamic, field_name: &str, fix: impl FnOnce(&str) -> String) -> Dynamic {
    let created = |value: String| Dynamic::new(wafer_protocol::nbt::Nbt::String(value));
    dynamic.update(field_name, |field| match field.as_string().into_result() {
        Ok(value) => created(fix(&value)),
        Err(_) => field,
    })
}

pub fn get_plain_translation_key(text_component: &Dynamic) -> Option<String> {
    let fields = text_component.get_map_values();
    if !fields.is_success() || fields.value.as_ref().is_none_or(|fields| fields.len() != 1) {
        return None;
    }
    text_component.get("translate").as_string().into_result().ok()
}

pub fn dye_color_id_to_name(id: i32) -> &'static str {
    match id {
        1 => "orange",
        2 => "magenta",
        3 => "light_blue",
        4 => "yellow",
        5 => "lime",
        6 => "pink",
        7 => "gray",
        8 => "light_gray",
        9 => "cyan",
        10 => "purple",
        11 => "blue",
        12 => "brown",
        13 => "green",
        14 => "red",
        15 => "black",
        _ => "white",
    }
}

pub fn read_and_set(target: Typed, optic: &OpticFinder, value: Dynamic) -> Typed {
    let read = crate::util::read_typed_or_throw(&optic.ty(), value, true);
    target.set(optic, read)
}
