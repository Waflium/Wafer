use std::collections::HashMap;
use std::sync::{Arc, LazyLock};

use wafer_protocol::nbt::Nbt;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dsl::ConstType;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::{Typed, named_choice, type_finder};
use crate::datafixers::types::{Type, and, const_type, field, named, optional, or, remainder_type};
use crate::datafixers::value::Value;
use crate::util::datafix::fixes::block_state_data;
use crate::util::datafix::fixes::references::{BLOCK_NAME, BLOCK_STATE, ENTITY};

const ENTRIES: [(&str, i32); 254] = [
    ("minecraft:air", 0),
    ("minecraft:stone", 1),
    ("minecraft:grass", 2),
    ("minecraft:dirt", 3),
    ("minecraft:cobblestone", 4),
    ("minecraft:planks", 5),
    ("minecraft:sapling", 6),
    ("minecraft:bedrock", 7),
    ("minecraft:flowing_water", 8),
    ("minecraft:water", 9),
    ("minecraft:flowing_lava", 10),
    ("minecraft:lava", 11),
    ("minecraft:sand", 12),
    ("minecraft:gravel", 13),
    ("minecraft:gold_ore", 14),
    ("minecraft:iron_ore", 15),
    ("minecraft:coal_ore", 16),
    ("minecraft:log", 17),
    ("minecraft:leaves", 18),
    ("minecraft:sponge", 19),
    ("minecraft:glass", 20),
    ("minecraft:lapis_ore", 21),
    ("minecraft:lapis_block", 22),
    ("minecraft:dispenser", 23),
    ("minecraft:sandstone", 24),
    ("minecraft:noteblock", 25),
    ("minecraft:bed", 26),
    ("minecraft:golden_rail", 27),
    ("minecraft:detector_rail", 28),
    ("minecraft:sticky_piston", 29),
    ("minecraft:web", 30),
    ("minecraft:tallgrass", 31),
    ("minecraft:deadbush", 32),
    ("minecraft:piston", 33),
    ("minecraft:piston_head", 34),
    ("minecraft:wool", 35),
    ("minecraft:piston_extension", 36),
    ("minecraft:yellow_flower", 37),
    ("minecraft:red_flower", 38),
    ("minecraft:brown_mushroom", 39),
    ("minecraft:red_mushroom", 40),
    ("minecraft:gold_block", 41),
    ("minecraft:iron_block", 42),
    ("minecraft:double_stone_slab", 43),
    ("minecraft:stone_slab", 44),
    ("minecraft:brick_block", 45),
    ("minecraft:tnt", 46),
    ("minecraft:bookshelf", 47),
    ("minecraft:mossy_cobblestone", 48),
    ("minecraft:obsidian", 49),
    ("minecraft:torch", 50),
    ("minecraft:fire", 51),
    ("minecraft:mob_spawner", 52),
    ("minecraft:oak_stairs", 53),
    ("minecraft:chest", 54),
    ("minecraft:redstone_wire", 55),
    ("minecraft:diamond_ore", 56),
    ("minecraft:diamond_block", 57),
    ("minecraft:crafting_table", 58),
    ("minecraft:wheat", 59),
    ("minecraft:farmland", 60),
    ("minecraft:furnace", 61),
    ("minecraft:lit_furnace", 62),
    ("minecraft:standing_sign", 63),
    ("minecraft:wooden_door", 64),
    ("minecraft:ladder", 65),
    ("minecraft:rail", 66),
    ("minecraft:stone_stairs", 67),
    ("minecraft:wall_sign", 68),
    ("minecraft:lever", 69),
    ("minecraft:stone_pressure_plate", 70),
    ("minecraft:iron_door", 71),
    ("minecraft:wooden_pressure_plate", 72),
    ("minecraft:redstone_ore", 73),
    ("minecraft:lit_redstone_ore", 74),
    ("minecraft:unlit_redstone_torch", 75),
    ("minecraft:redstone_torch", 76),
    ("minecraft:stone_button", 77),
    ("minecraft:snow_layer", 78),
    ("minecraft:ice", 79),
    ("minecraft:snow", 80),
    ("minecraft:cactus", 81),
    ("minecraft:clay", 82),
    ("minecraft:reeds", 83),
    ("minecraft:jukebox", 84),
    ("minecraft:fence", 85),
    ("minecraft:pumpkin", 86),
    ("minecraft:netherrack", 87),
    ("minecraft:soul_sand", 88),
    ("minecraft:glowstone", 89),
    ("minecraft:portal", 90),
    ("minecraft:lit_pumpkin", 91),
    ("minecraft:cake", 92),
    ("minecraft:unpowered_repeater", 93),
    ("minecraft:powered_repeater", 94),
    ("minecraft:stained_glass", 95),
    ("minecraft:trapdoor", 96),
    ("minecraft:monster_egg", 97),
    ("minecraft:stonebrick", 98),
    ("minecraft:brown_mushroom_block", 99),
    ("minecraft:red_mushroom_block", 100),
    ("minecraft:iron_bars", 101),
    ("minecraft:glass_pane", 102),
    ("minecraft:melon_block", 103),
    ("minecraft:pumpkin_stem", 104),
    ("minecraft:melon_stem", 105),
    ("minecraft:vine", 106),
    ("minecraft:fence_gate", 107),
    ("minecraft:brick_stairs", 108),
    ("minecraft:stone_brick_stairs", 109),
    ("minecraft:mycelium", 110),
    ("minecraft:waterlily", 111),
    ("minecraft:nether_brick", 112),
    ("minecraft:nether_brick_fence", 113),
    ("minecraft:nether_brick_stairs", 114),
    ("minecraft:nether_wart", 115),
    ("minecraft:enchanting_table", 116),
    ("minecraft:brewing_stand", 117),
    ("minecraft:cauldron", 118),
    ("minecraft:end_portal", 119),
    ("minecraft:end_portal_frame", 120),
    ("minecraft:end_stone", 121),
    ("minecraft:dragon_egg", 122),
    ("minecraft:redstone_lamp", 123),
    ("minecraft:lit_redstone_lamp", 124),
    ("minecraft:double_wooden_slab", 125),
    ("minecraft:wooden_slab", 126),
    ("minecraft:cocoa", 127),
    ("minecraft:sandstone_stairs", 128),
    ("minecraft:emerald_ore", 129),
    ("minecraft:ender_chest", 130),
    ("minecraft:tripwire_hook", 131),
    ("minecraft:tripwire", 132),
    ("minecraft:emerald_block", 133),
    ("minecraft:spruce_stairs", 134),
    ("minecraft:birch_stairs", 135),
    ("minecraft:jungle_stairs", 136),
    ("minecraft:command_block", 137),
    ("minecraft:beacon", 138),
    ("minecraft:cobblestone_wall", 139),
    ("minecraft:flower_pot", 140),
    ("minecraft:carrots", 141),
    ("minecraft:potatoes", 142),
    ("minecraft:wooden_button", 143),
    ("minecraft:skull", 144),
    ("minecraft:anvil", 145),
    ("minecraft:trapped_chest", 146),
    ("minecraft:light_weighted_pressure_plate", 147),
    ("minecraft:heavy_weighted_pressure_plate", 148),
    ("minecraft:unpowered_comparator", 149),
    ("minecraft:powered_comparator", 150),
    ("minecraft:daylight_detector", 151),
    ("minecraft:redstone_block", 152),
    ("minecraft:quartz_ore", 153),
    ("minecraft:hopper", 154),
    ("minecraft:quartz_block", 155),
    ("minecraft:quartz_stairs", 156),
    ("minecraft:activator_rail", 157),
    ("minecraft:dropper", 158),
    ("minecraft:stained_hardened_clay", 159),
    ("minecraft:stained_glass_pane", 160),
    ("minecraft:leaves2", 161),
    ("minecraft:log2", 162),
    ("minecraft:acacia_stairs", 163),
    ("minecraft:dark_oak_stairs", 164),
    ("minecraft:slime", 165),
    ("minecraft:barrier", 166),
    ("minecraft:iron_trapdoor", 167),
    ("minecraft:prismarine", 168),
    ("minecraft:sea_lantern", 169),
    ("minecraft:hay_block", 170),
    ("minecraft:carpet", 171),
    ("minecraft:hardened_clay", 172),
    ("minecraft:coal_block", 173),
    ("minecraft:packed_ice", 174),
    ("minecraft:double_plant", 175),
    ("minecraft:standing_banner", 176),
    ("minecraft:wall_banner", 177),
    ("minecraft:daylight_detector_inverted", 178),
    ("minecraft:red_sandstone", 179),
    ("minecraft:red_sandstone_stairs", 180),
    ("minecraft:double_stone_slab2", 181),
    ("minecraft:stone_slab2", 182),
    ("minecraft:spruce_fence_gate", 183),
    ("minecraft:birch_fence_gate", 184),
    ("minecraft:jungle_fence_gate", 185),
    ("minecraft:dark_oak_fence_gate", 186),
    ("minecraft:acacia_fence_gate", 187),
    ("minecraft:spruce_fence", 188),
    ("minecraft:birch_fence", 189),
    ("minecraft:jungle_fence", 190),
    ("minecraft:dark_oak_fence", 191),
    ("minecraft:acacia_fence", 192),
    ("minecraft:spruce_door", 193),
    ("minecraft:birch_door", 194),
    ("minecraft:jungle_door", 195),
    ("minecraft:acacia_door", 196),
    ("minecraft:dark_oak_door", 197),
    ("minecraft:end_rod", 198),
    ("minecraft:chorus_plant", 199),
    ("minecraft:chorus_flower", 200),
    ("minecraft:purpur_block", 201),
    ("minecraft:purpur_pillar", 202),
    ("minecraft:purpur_stairs", 203),
    ("minecraft:purpur_double_slab", 204),
    ("minecraft:purpur_slab", 205),
    ("minecraft:end_bricks", 206),
    ("minecraft:beetroots", 207),
    ("minecraft:grass_path", 208),
    ("minecraft:end_gateway", 209),
    ("minecraft:repeating_command_block", 210),
    ("minecraft:chain_command_block", 211),
    ("minecraft:frosted_ice", 212),
    ("minecraft:magma", 213),
    ("minecraft:nether_wart_block", 214),
    ("minecraft:red_nether_brick", 215),
    ("minecraft:bone_block", 216),
    ("minecraft:structure_void", 217),
    ("minecraft:observer", 218),
    ("minecraft:white_shulker_box", 219),
    ("minecraft:orange_shulker_box", 220),
    ("minecraft:magenta_shulker_box", 221),
    ("minecraft:light_blue_shulker_box", 222),
    ("minecraft:yellow_shulker_box", 223),
    ("minecraft:lime_shulker_box", 224),
    ("minecraft:pink_shulker_box", 225),
    ("minecraft:gray_shulker_box", 226),
    ("minecraft:silver_shulker_box", 227),
    ("minecraft:cyan_shulker_box", 228),
    ("minecraft:purple_shulker_box", 229),
    ("minecraft:blue_shulker_box", 230),
    ("minecraft:brown_shulker_box", 231),
    ("minecraft:green_shulker_box", 232),
    ("minecraft:red_shulker_box", 233),
    ("minecraft:black_shulker_box", 234),
    ("minecraft:white_glazed_terracotta", 235),
    ("minecraft:orange_glazed_terracotta", 236),
    ("minecraft:magenta_glazed_terracotta", 237),
    ("minecraft:light_blue_glazed_terracotta", 238),
    ("minecraft:yellow_glazed_terracotta", 239),
    ("minecraft:lime_glazed_terracotta", 240),
    ("minecraft:pink_glazed_terracotta", 241),
    ("minecraft:gray_glazed_terracotta", 242),
    ("minecraft:silver_glazed_terracotta", 243),
    ("minecraft:cyan_glazed_terracotta", 244),
    ("minecraft:purple_glazed_terracotta", 245),
    ("minecraft:blue_glazed_terracotta", 246),
    ("minecraft:brown_glazed_terracotta", 247),
    ("minecraft:green_glazed_terracotta", 248),
    ("minecraft:red_glazed_terracotta", 249),
    ("minecraft:black_glazed_terracotta", 250),
    ("minecraft:concrete", 251),
    ("minecraft:concrete_powder", 252),
    ("minecraft:structure_block", 255),
];

static MAP: LazyLock<HashMap<&'static str, i32>> = LazyLock::new(|| ENTRIES.iter().copied().collect());

pub struct EntityBlockStateFix {
    base: DataFixBase,
}

impl EntityBlockStateFix {
    pub fn new(output_schema: &Arc<Schema>, changes_type: bool) -> EntityBlockStateFix {
        EntityBlockStateFix { base: DataFixBase::new(output_schema, changes_type) }
    }

    pub fn get_block_id(name: &str) -> i32 {
        MAP.get(name).copied().unwrap_or(0)
    }
}

fn block_name_type() -> Arc<Type> {
    named(BLOCK_NAME.type_name(), or(const_type(ConstType::Int), const_type(ConstType::NamespacedString)))
}

fn block_state_type() -> Arc<Type> {
    named(BLOCK_STATE.type_name(), remainder_type())
}

fn block_id_of(block: Value) -> i32 {
    match block {
        Value::Left(value) => match *value {
            Value::Prim(Nbt::Int(block)) => block,
            other => panic!("Unexpected block id {other:?}"),
        },
        Value::Right(value) => EntityBlockStateFix::get_block_id(value.as_str().unwrap_or_default()),
        other => panic!("Unexpected block {other:?}"),
    }
}

fn update_falling_block(input: Typed) -> Typed {
    let old_type = optional(field("Block", block_name_type()));
    let new_type = optional(field("BlockState", block_state_type()));
    let tag = input.get_remainder();
    let updated_tag = tag.clone().remove("Data").remove("TileID").remove("Tile");
    input
        .update_to(&type_finder(old_type), &new_type, |tile| {
            let block = match tile {
                Value::Left(block) => block_id_of(*block),
                _ => match tag.get("TileID").as_number().into_result() {
                    Ok(tile_id) => tile_id.int_value(),
                    Err(_) => tag.get("Tile").as_byte(0) as i32 & 0xFF,
                },
            };
            let data = tag.get("Data").as_int(0) & 15;
            Value::Left(Box::new(Value::Dynamic((*block_state_data::get_tag(block << 4 | data)).clone())))
        })
        .set_remainder(updated_tag)
}

fn update_block_to_block_state(input: Typed, old_field_name: &str, data_name: &str, new_field_name: &str) -> Typed {
    let old_type = field(old_field_name, block_name_type());
    let new_type = field(new_field_name, block_state_type());
    let tag = input.get_remainder();
    let updated_tag = tag.clone().remove(data_name);
    input
        .update_to(&type_finder(old_type), &new_type, |tile| {
            let block = block_id_of(tile);
            let data = tag.get(data_name).as_int(0) & 15;
            Value::Dynamic((*block_state_data::get_tag(block << 4 | data)).clone())
        })
        .set_remainder(updated_tag)
}

fn update_entity(input_schema: &Schema, output_schema: &Schema, input: Typed, name: &str, function: &dyn Fn(Typed) -> Typed) -> Typed {
    let old_type = input_schema.get_choice_type(ENTITY, name);
    let new_type = output_schema.get_choice_type(ENTITY, name);
    input.update_typed(&named_choice(name, old_type), Some(&new_type), |typed| function(typed))
}

impl DataFix for EntityBlockStateFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let input_schema = self.get_input_schema().clone();
        let output_schema = self.get_output_schema().clone();
        let minecart_updater = |input| update_block_to_block_state(input, "DisplayTile", "DisplayData", "DisplayState");
        let arrow_updater = |input| update_block_to_block_state(input, "inTile", "inData", "inBlockState");
        let old_projectile_type = and(optional(field("inTile", block_name_type())), remainder_type());
        let remove_in_tile = move |input: Typed| {
            input.update_to(&type_finder(old_projectile_type.clone()), &remainder_type(), |pair| match pair {
                Value::Pair(_, second) => *second,
                other => panic!("Unexpected projectile {other:?}"),
            })
        };
        let (input_type, output_type) = (input_schema.get_type(ENTITY), output_schema.get_type(ENTITY));
        self.fix_type_everywhere_typed_to("EntityBlockStateFix", input_type, output_type, move |input| {
            let (i, o) = (&*input_schema, &*output_schema);
            let input = update_entity(i, o, input, "minecraft:falling_block", &update_falling_block);
            let input = update_entity(i, o, input, "minecraft:enderman", &|input2| update_block_to_block_state(input2, "carried", "carriedData", "carriedBlockState"));
            let input = update_entity(i, o, input, "minecraft:arrow", &arrow_updater);
            let input = update_entity(i, o, input, "minecraft:spectral_arrow", &arrow_updater);
            let input = update_entity(i, o, input, "minecraft:egg", &remove_in_tile);
            let input = update_entity(i, o, input, "minecraft:ender_pearl", &remove_in_tile);
            let input = update_entity(i, o, input, "minecraft:fireball", &remove_in_tile);
            let input = update_entity(i, o, input, "minecraft:potion", &remove_in_tile);
            let input = update_entity(i, o, input, "minecraft:small_fireball", &remove_in_tile);
            let input = update_entity(i, o, input, "minecraft:snowball", &remove_in_tile);
            let input = update_entity(i, o, input, "minecraft:wither_skull", &remove_in_tile);
            let input = update_entity(i, o, input, "minecraft:xp_bottle", &remove_in_tile);
            let input = update_entity(i, o, input, "minecraft:commandblock_minecart", &minecart_updater);
            let input = update_entity(i, o, input, "minecraft:minecart", &minecart_updater);
            let input = update_entity(i, o, input, "minecraft:chest_minecart", &minecart_updater);
            let input = update_entity(i, o, input, "minecraft:furnace_minecart", &minecart_updater);
            let input = update_entity(i, o, input, "minecraft:tnt_minecart", &minecart_updater);
            let input = update_entity(i, o, input, "minecraft:hopper_minecart", &minecart_updater);
            update_entity(i, o, input, "minecraft:spawner_minecart", &minecart_updater)
        })
    }
}
