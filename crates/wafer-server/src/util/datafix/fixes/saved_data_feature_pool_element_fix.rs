use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::{DataResult, Dynamic, OptionalDynamic};
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::STRUCTURE_FEATURE;

const PIECE_TYPE: [&str; 5] = ["minecraft:jigsaw", "minecraft:nvi", "minecraft:pcp", "minecraft:bastionremnant", "minecraft:runtime"];
const FEATURES: [&str; 4] = ["minecraft:tree", "minecraft:flower", "minecraft:block_pile", "minecraft:random_patch"];

pub struct SavedDataFeaturePoolElementFix {
    base: DataFixBase,
}

impl SavedDataFeaturePoolElementFix {
    pub fn new(output_schema: &Arc<Schema>) -> SavedDataFeaturePoolElementFix {
        SavedDataFeaturePoolElementFix { base: DataFixBase::new(output_schema, false) }
    }
}

impl DataFix for SavedDataFeaturePoolElementFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        self.write_fix_and_read(
            "SavedDataFeaturePoolElementFix",
            self.get_input_schema().get_type(STRUCTURE_FEATURE),
            self.get_output_schema().get_type(STRUCTURE_FEATURE),
            fix_tag,
        )
    }
}

fn fix_tag(input: Dynamic) -> Dynamic {
    input.update("Children", update_children)
}

fn update_children(input: Dynamic) -> Dynamic {
    match input.as_stream_opt().into_result() {
        Ok(stream) => input.create_list(stream.into_iter().map(update_child)),
        Err(_) => input,
    }
}

fn update_child(child: Dynamic) -> Dynamic {
    let id = child.get("id").as_string_or("");
    if !PIECE_TYPE.contains(&id.as_str()) {
        return child;
    }
    if child.get("pool_element").get("element_type").as_string_or("") != "minecraft:feature_pool_element" {
        return child;
    }
    child.update("pool_element", |pool| pool.update("feature", fix_feature))
}

fn get(input: &Dynamic, path: &[&str]) -> OptionalDynamic {
    if path.is_empty() {
        panic!("Missing path");
    }
    let mut output = input.get(path[0]);
    for element in &path[1..] {
        if let Some(id) = parse_index(element) {
            let dynamics = output.as_list(|dynamic| dynamic);
            output = match dynamics.into_iter().nth(id) {
                Some(dynamic) => OptionalDynamic(DataResult::success(dynamic)),
                None => OptionalDynamic(DataResult::error(format!("Missing id:{id}"))),
            };
        } else {
            output = output.get(element);
        }
    }
    output
}

fn parse_index(element: &str) -> Option<usize> {
    let digits = element.strip_prefix('[')?.strip_suffix(']')?;
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    Some(digits.parse().expect("an index"))
}

fn fix_feature(value: Dynamic) -> Dynamic {
    let replacement = get_replacement(
        &get(&value, &["type"]).as_string_or(""),
        &get(&value, &["name"]).as_string_or(""),
        &get(&value, &["config", "state_provider", "type"]).as_string_or(""),
        &get(&value, &["config", "state_provider", "state", "Name"]).as_string_or(""),
        &get(&value, &["config", "state_provider", "entries", "[0]", "data", "Name"]).as_string_or(""),
        &get(&value, &["config", "foliage_placer", "type"]).as_string_or(""),
        &get(&value, &["config", "leaves_provider", "state", "Name"]).as_string_or(""),
    );
    match replacement {
        Some(replacement) => value.create_string(replacement),
        None => value,
    }
}

fn get_replacement(
    ty: &str,
    name: &str,
    state_provider_type: &str,
    state_provider_state: &str,
    state_provider_first_weighed_state: &str,
    foliage_placer_type: &str,
    leaves_provider_state: &str,
) -> Option<&'static str> {
    let feature = if !ty.is_empty() {
        ty
    } else {
        if name.is_empty() {
            return None;
        }
        if name == "minecraft:normal_tree" { "minecraft:tree" } else { name }
    };
    if FEATURES.contains(&feature) {
        if feature == "minecraft:random_patch" {
            if state_provider_type == "minecraft:simple_state_provider" {
                if state_provider_state == "minecraft:sweet_berry_bush" {
                    return Some("minecraft:patch_berry_bush");
                }
                if state_provider_state == "minecraft:cactus" {
                    return Some("minecraft:patch_cactus");
                }
            } else if state_provider_type == "minecraft:weighted_state_provider" && (state_provider_first_weighed_state == "minecraft:grass" || state_provider_first_weighed_state == "minecraft:fern") {
                return Some("minecraft:patch_taiga_grass");
            }
        } else if feature == "minecraft:block_pile" {
            if state_provider_type != "minecraft:simple_state_provider" && state_provider_type != "minecraft:rotated_block_provider" {
                if state_provider_type == "minecraft:weighted_state_provider" {
                    if state_provider_first_weighed_state == "minecraft:packed_ice" || state_provider_first_weighed_state == "minecraft:blue_ice" {
                        return Some("minecraft:pile_ice");
                    }
                    if state_provider_first_weighed_state == "minecraft:jack_o_lantern" || state_provider_first_weighed_state == "minecraft:pumpkin" {
                        return Some("minecraft:pile_pumpkin");
                    }
                }
            } else {
                if state_provider_state == "minecraft:hay_block" {
                    return Some("minecraft:pile_hay");
                }
                if state_provider_state == "minecraft:melon" {
                    return Some("minecraft:pile_melon");
                }
                if state_provider_state == "minecraft:snow" {
                    return Some("minecraft:pile_snow");
                }
            }
        } else {
            if feature == "minecraft:flower" {
                return Some("minecraft:flower_plain");
            }
            if feature == "minecraft:tree" {
                if foliage_placer_type == "minecraft:acacia_foliage_placer" {
                    return Some("minecraft:acacia");
                }
                if foliage_placer_type == "minecraft:blob_foliage_placer" && leaves_provider_state == "minecraft:oak_leaves" {
                    return Some("minecraft:oak");
                }
                if foliage_placer_type == "minecraft:pine_foliage_placer" {
                    return Some("minecraft:pine");
                }
                if foliage_placer_type == "minecraft:spruce_foliage_placer" {
                    return Some("minecraft:spruce");
                }
            }
        }
    }
    None
}
