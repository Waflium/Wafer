use std::sync::Arc;

use crate::commands::brigadier::{CommandSyntaxException, StringReader};
use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::PARTICLE;
use crate::util::datafix::schemas::namespaced_schema::ensure_namespaced;

pub struct ParticleUnflatteningFix {
    base: DataFixBase,
}

impl ParticleUnflatteningFix {
    pub fn new(output_schema: &Arc<Schema>) -> ParticleUnflatteningFix {
        ParticleUnflatteningFix { base: DataFixBase::new(output_schema, true) }
    }
}

fn fix(input: Dynamic) -> Dynamic {
    let Ok(particle_description) = input.as_string().into_result() else { return input };
    let parts: Vec<&str> = particle_description.splitn(2, ' ').collect();
    let id = ensure_namespaced(parts[0]);
    let result = input.create_map([(input.create_string("type"), input.create_string(id.clone()))]);
    let Some(contents) = parts.get(1).copied() else { return result };
    match id.as_str() {
        "minecraft:item" => update_item(result, contents),
        "minecraft:block" | "minecraft:block_marker" | "minecraft:falling_dust" | "minecraft:dust_pillar" => update_block(result, contents),
        "minecraft:dust" => update_dust(result, contents),
        "minecraft:dust_color_transition" => update_dust_transition(result, contents),
        "minecraft:sculk_charge" => update_sculk_charge(result, contents),
        "minecraft:vibration" => update_vibration(result, contents),
        "minecraft:shriek" => update_shriek(result, contents),
        _ => result,
    }
}

fn update_item(result: Dynamic, contents: &str) -> Dynamic {
    let tag_part_start = contents.find('{');
    let mut item_stack = result.create_map([(result.create_string("Count"), result.create_int(1))]);
    match tag_part_start {
        None => item_stack = item_stack.set("id", result.create_string(contents)),
        Some(tag_part_start) => {
            item_stack = item_stack.set("id", result.create_string(&contents[..tag_part_start]));
            if let Some(item_tag) = parse_tag(&contents[tag_part_start..]) {
                item_stack = item_stack.set("tag", item_tag);
            }
        }
    }
    result.set("item", item_stack)
}

fn parse_tag(contents: &str) -> Option<Dynamic> {
    match crate::nbt::parse_fully(contents) {
        Ok(tag) => Some(Dynamic::new(tag)),
        Err(e) => {
            warn!("Failed to parse tag: {}\ncom.mojang.brigadier.exceptions.CommandSyntaxException: {}", contents, e.get_message());
            None
        }
    }
}

fn update_block(result: Dynamic, contents: &str) -> Dynamic {
    let state_part_start = contents.find('[');
    let mut block_state = result.empty_map();
    match state_part_start {
        None => block_state = block_state.set("Name", result.create_string(ensure_namespaced(contents))),
        Some(state_part_start) => {
            block_state = block_state.set("Name", result.create_string(ensure_namespaced(&contents[..state_part_start])));
            let properties = parse_block_properties(&contents[state_part_start..]);
            if !properties.is_empty() {
                let properties = result.create_map(properties.into_iter().map(|(key, value)| (result.create_string(key), result.create_string(value))));
                block_state = block_state.set("Properties", properties);
            }
        }
    }
    result.set("block_state", block_state)
}

fn parse_block_properties(contents: &str) -> Vec<(String, String)> {
    match read_block_properties(contents) {
        Ok(properties) => properties,
        Err(e) => {
            warn!("Failed to parse block properties: {}\ncom.mojang.brigadier.exceptions.CommandSyntaxException: {}", contents, e.get_message());
            Vec::new()
        }
    }
}

fn read_block_properties(contents: &str) -> Result<Vec<(String, String)>, CommandSyntaxException> {
    let units: Vec<u16> = contents.encode_utf16().collect();
    let mut reader = StringReader::new(&units);
    let mut result: Vec<(String, String)> = Vec::new();
    reader.expect('[')?;
    reader.skip_whitespace();
    while reader.can_read() && reader.peek() != ']' as u16 {
        reader.skip_whitespace();
        let key = reader.read_string()?;
        reader.skip_whitespace();
        reader.expect('=')?;
        reader.skip_whitespace();
        let value = reader.read_string()?;
        reader.skip_whitespace();
        match result.iter_mut().find(|(existing, _)| *existing == key) {
            Some((_, slot)) => *slot = value,
            None => result.push((key, value)),
        }
        if reader.can_read() {
            if reader.peek() != ',' as u16 {
                break;
            }
            reader.skip();
        }
    }
    reader.expect(']')?;
    Ok(result)
}

fn read_vector(result: &Dynamic, reader: &mut StringReader) -> Result<Dynamic, CommandSyntaxException> {
    let x = reader.read_float()?;
    reader.expect(' ')?;
    let y = reader.read_float()?;
    reader.expect(' ')?;
    let z = reader.read_float()?;
    Ok(result.create_list([x, y, z].into_iter().map(|value| result.create_float(value))))
}

fn update_options(result: Dynamic, contents: &str, update: impl FnOnce(&Dynamic, &mut StringReader) -> Result<Dynamic, CommandSyntaxException>) -> Dynamic {
    let units: Vec<u16> = contents.encode_utf16().collect();
    let mut reader = StringReader::new(&units);
    match update(&result, &mut reader) {
        Ok(updated) => updated,
        Err(e) => {
            warn!("Failed to parse particle options: {}\ncom.mojang.brigadier.exceptions.CommandSyntaxException: {}", contents, e.get_message());
            result
        }
    }
}

fn update_dust(result: Dynamic, contents: &str) -> Dynamic {
    update_options(result, contents, |result, reader| {
        let vector = read_vector(result, reader)?;
        reader.expect(' ')?;
        let scale = reader.read_float()?;
        Ok(result.clone().set("color", vector).set("scale", result.create_float(scale)))
    })
}

fn update_dust_transition(result: Dynamic, contents: &str) -> Dynamic {
    update_options(result, contents, |result, reader| {
        let from = read_vector(result, reader)?;
        reader.expect(' ')?;
        let scale = reader.read_float()?;
        reader.expect(' ')?;
        let to = read_vector(result, reader)?;
        Ok(result.clone().set("from_color", from).set("to_color", to).set("scale", result.create_float(scale)))
    })
}

fn update_sculk_charge(result: Dynamic, contents: &str) -> Dynamic {
    update_options(result, contents, |result, reader| {
        let roll = reader.read_float()?;
        Ok(result.clone().set("roll", result.create_float(roll)))
    })
}

fn update_vibration(result: Dynamic, contents: &str) -> Dynamic {
    update_options(result, contents, |result, reader| {
        let dest_x = reader.read_double()? as f32;
        reader.expect(' ')?;
        let dest_y = reader.read_double()? as f32;
        reader.expect(' ')?;
        let dest_z = reader.read_double()? as f32;
        reader.expect(' ')?;
        let arrival_in_ticks = reader.read_int()?;
        let block_pos = result.create_int_list([wafer_core::mth::floor_f(dest_x), wafer_core::mth::floor_f(dest_y), wafer_core::mth::floor_f(dest_z)]);
        let position_source = result.create_map([(result.create_string("type"), result.create_string("minecraft:block")), (result.create_string("pos"), block_pos)]);
        Ok(result.clone().set("destination", position_source).set("arrival_in_ticks", result.create_int(arrival_in_ticks)))
    })
}

fn update_shriek(result: Dynamic, contents: &str) -> Dynamic {
    update_options(result, contents, |result, reader| {
        let delay = reader.read_int()?;
        Ok(result.clone().set("delay", result.create_int(delay)))
    })
}

impl DataFix for ParticleUnflatteningFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let old_type = self.get_input_schema().get_type(PARTICLE);
        let new_type = self.get_output_schema().get_type(PARTICLE);
        self.write_fix_and_read("ParticleUnflatteningFix", old_type, new_type, fix)
    }
}
