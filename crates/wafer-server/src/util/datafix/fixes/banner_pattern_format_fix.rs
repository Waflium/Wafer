use std::sync::Arc;

use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::Typed;
use crate::util::datafix::extra_data_fix_utils;
use crate::util::datafix::fixes::named_entity_fix::NamedEntityFix;
use crate::util::datafix::fixes::references::BLOCK_ENTITY;

const PATTERN_ID_MAP: [(&str, &str); 41] = [
    ("b", "minecraft:base"),
    ("bl", "minecraft:square_bottom_left"),
    ("br", "minecraft:square_bottom_right"),
    ("tl", "minecraft:square_top_left"),
    ("tr", "minecraft:square_top_right"),
    ("bs", "minecraft:stripe_bottom"),
    ("ts", "minecraft:stripe_top"),
    ("ls", "minecraft:stripe_left"),
    ("rs", "minecraft:stripe_right"),
    ("cs", "minecraft:stripe_center"),
    ("ms", "minecraft:stripe_middle"),
    ("drs", "minecraft:stripe_downright"),
    ("dls", "minecraft:stripe_downleft"),
    ("ss", "minecraft:small_stripes"),
    ("cr", "minecraft:cross"),
    ("sc", "minecraft:straight_cross"),
    ("bt", "minecraft:triangle_bottom"),
    ("tt", "minecraft:triangle_top"),
    ("bts", "minecraft:triangles_bottom"),
    ("tts", "minecraft:triangles_top"),
    ("ld", "minecraft:diagonal_left"),
    ("rd", "minecraft:diagonal_up_right"),
    ("lud", "minecraft:diagonal_up_left"),
    ("rud", "minecraft:diagonal_right"),
    ("mc", "minecraft:circle"),
    ("mr", "minecraft:rhombus"),
    ("vh", "minecraft:half_vertical"),
    ("hh", "minecraft:half_horizontal"),
    ("vhr", "minecraft:half_vertical_right"),
    ("hhb", "minecraft:half_horizontal_bottom"),
    ("bo", "minecraft:border"),
    ("cbo", "minecraft:curly_border"),
    ("gra", "minecraft:gradient"),
    ("gru", "minecraft:gradient_up"),
    ("bri", "minecraft:bricks"),
    ("glb", "minecraft:globe"),
    ("cre", "minecraft:creeper"),
    ("sku", "minecraft:skull"),
    ("flo", "minecraft:flower"),
    ("moj", "minecraft:mojang"),
    ("pig", "minecraft:piglin"),
];

pub struct BannerPatternFormatFix;

impl BannerPatternFormatFix {
    pub fn new(output_schema: &Arc<Schema>) -> NamedEntityFix {
        NamedEntityFix::new(output_schema, false, "BannerPatternFormatFix", BLOCK_ENTITY, "minecraft:banner", fix)
    }
}

fn fix(entity: Typed) -> Typed {
    entity.update_remainder(fix_tag)
}

fn fix_tag(tag: Dynamic) -> Dynamic {
    tag.rename_and_fix_field("Patterns", "patterns", |patterns| {
        let layers = patterns.as_stream();
        patterns.create_list(layers.into_iter().map(fix_layer))
    })
}

fn fix_layer(mut dynamic: Dynamic) -> Dynamic {
    dynamic = dynamic.rename_and_fix_field("Pattern", "pattern", |pattern| match pattern.as_string().into_result() {
        Ok(id) => {
            let mapped = PATTERN_ID_MAP.iter().find(|(from, _)| *from == id).map_or(id, |(_, to)| to.to_string());
            pattern.create_string(mapped)
        }
        Err(_) => pattern,
    });
    let color = dynamic.create_string(extra_data_fix_utils::dye_color_id_to_name(dynamic.get("Color").as_int(0)));
    dynamic = dynamic.set("color", color);
    dynamic.remove("Color")
}
