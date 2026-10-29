use wafer_data::{Item, items};
use wafer_protocol::nbt::Nbt;

use super::crafting_input::CraftingInput;
use super::recipe_manager::RecipeManager;
use crate::server::level::testing;
use crate::world::item::ItemStack;
use crate::world::item::component as dc;

fn stack(item: Item) -> ItemStack {
    ItemStack::new(item, 1)
}

fn craft(width: usize, grid: &[Option<Item>]) -> (Option<&'static str>, ItemStack) {
    let mut level = testing::level();
    let items: Vec<ItemStack> = grid.iter().map(|item| item.map_or_else(ItemStack::empty, stack)).collect();
    let mut input = CraftingInput::of(width, grid.len() / width, &items);
    match RecipeManager::get().get_crafting_recipe_for(&mut input, &mut level, None) {
        Some(holder) => (Some(holder.id), holder.crafting().expect("a crafting recipe").assemble(&input)),
        None => (None, ItemStack::empty()),
    }
}

pub fn vanilla_data_files_with_extension(directory: &str, extension: &str) -> usize {
    let archive = zip::ZipArchive::new(std::io::Cursor::new(wafer_data::VANILLA_DATA)).unwrap();
    archive.file_names().filter(|name| name.starts_with(&format!("data/minecraft/{directory}/")) && name.ends_with(extension)).count()
}

pub fn vanilla_data_files(directory: &str) -> usize {
    vanilla_data_files_with_extension(directory, ".json")
}

fn decode_recipe(json: serde_json::Value) -> Result<(), String> {
    let tag = dc::json_to_nbt(&json);
    crate::nbt::with_json_source(|| super::recipe::Recipe::decode(&tag)).map(drop)
}

#[test]
fn a_failed_pattern_row_is_reported_before_the_pattern_check() {
    let recipe = |pattern: serde_json::Value, key: serde_json::Value| {
        serde_json::json!({"type": "minecraft:crafting_shaped", "key": key, "pattern": pattern, "result": {"id": "minecraft:stick"}})
    };
    let planks = serde_json::json!({"#": "minecraft:oak_planks"});
    assert_eq!(decode_recipe(recipe(serde_json::json!([1]), planks.clone())).unwrap_err(), "Not a string: 1; Invalid pattern: empty pattern not allowed");
    let unknown = serde_json::json!({"#": "x"});
    assert_eq!(
        decode_recipe(recipe(serde_json::json!(["#"]), unknown)).unwrap_err(),
        "Map entry '#' : Failed to parse either. First: Not a tag id; Second: Failed to parse either. First: Not a json array: \"x\"; Second: Unknown registry key in ResourceKey[minecraft:root / minecraft:item]: minecraft:x"
    );
}

#[test]
fn a_failed_range_bound_is_reported_before_the_range_check() {
    let json = serde_json::json!({"type": "minecraft:crafting_transmute", "input": "minecraft:dirt", "material": "minecraft:stone", "material_count": {"min": 1, "max": "x"}, "result": "minecraft:stick"});
    assert_eq!(decode_recipe(json).unwrap_err(), "Not a number: \"x\"; Range must be within [1..8], but was [1..+\u{221e})");
}

#[test]
fn every_vanilla_recipe_decodes() {
    assert_eq!(RecipeManager::get().get_recipes().len(), vanilla_data_files("recipe"));
}

#[test]
fn crafts_shaped_and_shapeless_recipes() {
    let (id, result) = craft(2, &[None, None, None, Some(items::OAK_LOG)]);
    assert_eq!((id, result.item, result.count), (Some("minecraft:oak_planks"), items::OAK_PLANKS, 4));
    let p = Some(items::OAK_PLANKS);
    let s = Some(items::STICK);
    let (id, result) = craft(3, &[p, p, p, None, s, None, None, s, None]);
    assert_eq!((id, result.item), (Some("minecraft:wooden_pickaxe"), items::WOODEN_PICKAXE));
    let (_, axe) = craft(3, &[p, p, None, p, s, None, None, s, None]);
    let (_, mirrored) = craft(3, &[None, p, p, None, s, p, None, s, None]);
    assert_eq!((axe.item, mirrored.item), (items::WOODEN_AXE, items::WOODEN_AXE));
    assert_eq!(craft(2, &[Some(items::DIRT), None, None, None]).0, None);
}

fn decode_error(json: &str) -> String {
    let value: serde_json::Value = serde_json::from_str(json).unwrap();
    crate::nbt::with_json_source(|| super::recipe::Recipe::decode(&crate::world::item::component::json_to_nbt(&value)).err().expect("a decode error"))
}

#[test]
fn recipe_fields_report_every_error_in_vanilla_order() {
    assert_eq!(
        decode_error(r#"{"type":"minecraft:smelting","result":{"id":"stone"}}"#),
        r#"No key cookingtime in MapLike[{"type":"minecraft:smelting","result":{"id":"stone"}}]; No key ingredient in MapLike[{"type":"minecraft:smelting","result":{"id":"stone"}}]"#
    );
    assert_eq!(
        decode_error(r#"{"type":"minecraft:crafting_shapeless","category":"nope","group":5,"ingredients":["minecraft:stone"],"result":{"id":"stone"}}"#),
        "Not a string: 5; Unknown element name:nope"
    );
}

#[test]
fn list_and_item_errors_use_vanilla_wording() {
    assert_eq!(decode_error(r#"{"type":"minecraft:crafting_shapeless","ingredients":[null],"result":{"id":"stone"}}"#), "List is too short: 0, expected range [1-9]");
    assert_eq!(
        decode_error(r#"{"type":"minecraft:crafting_shapeless","ingredients":["minecraft:stone"],"result":{"id":"nope"}}"#),
        r#"Failed to parse either. First: Unknown registry key in ResourceKey[minecraft:root / minecraft:item]: minecraft:nope; Second: Not a string: {"id":"nope"}"#
    );
    assert_eq!(
        decode_error(r#"{"type":"minecraft:crafting_shaped","pattern":["a"],"key":{"a":"minecraft:stone","b":"minecraft:stone","c":"minecraft:stone"},"result":{"id":"stone"}}"#),
        "Key defines symbols that aren't used in pattern: {b, c}"
    );
}

#[test]
fn crafts_special_recipes() {
    let (id, helmet) = craft(2, &[Some(items::LEATHER_HELMET), Some(items::RED_DYE), None, None]);
    assert_eq!(id, Some("minecraft:leather_helmet_dyed"));
    assert_eq!(helmet.get(dc::DYED_COLOR), Some(&Nbt::Int(11546150)));
    let (_, rocket) = craft(2, &[Some(items::PAPER), Some(items::GUNPOWDER), None, None]);
    assert_eq!((rocket.item, rocket.count), (items::FIREWORK_ROCKET, 3));
    let (_, shulker_box) = craft(2, &[Some(items::SHULKER_BOX), Some(items::RED_DYE), None, None]);
    assert_eq!(shulker_box.item, items::RED_SHULKER_BOX);
}
