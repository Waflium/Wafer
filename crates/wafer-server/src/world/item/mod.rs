pub mod adventure_mode_predicate;
pub mod alchemy;
pub mod array_item_provider;
pub mod arrow_item;
pub mod banner_item;
pub mod block_item;
pub mod boat_item;
pub mod bottle_item;
pub mod compass_item;
pub mod written_book_item;
pub mod bow_item;
pub mod bucket_item;
pub mod mob_bucket_item;
pub mod bone_meal_item;
pub mod brush_item;
pub mod bundle_item;
pub mod component;
pub mod consume_effects;
pub mod context;
pub mod item_display_context;
pub mod crafting;
pub mod crossbow_item;
pub mod dye_color;
pub mod dye_item;
pub mod egg_item;
pub mod empty_map_item;
pub mod enderpearl_item;
pub mod experience_bottle_item;
pub mod fire_charge_item;
pub mod firework_rocket_item;
pub mod fishing_rod_item;
pub mod glow_ink_sac_item;
pub mod ink_sac_item;
pub mod projectile_item;
pub mod projectile_weapon_item;
pub mod sign_applicator;
pub mod snowball_item;
pub mod wind_charge_item;
pub mod spyglass_item;
pub mod enchantment;
pub mod end_crystal_item;
pub mod ender_eye_item;
pub mod flint_and_steel_item;
pub mod food_on_a_stick_item;
pub mod throwable_potion_item;
pub mod trading;
pub mod trident_item;
mod item_stack;
pub mod honeycomb_item;
pub mod instrument;
pub mod instrument_item;
pub mod item_cooldowns;
pub mod knowledge_book_item;
pub mod item_provider;
pub mod item_utils;
pub mod jukebox_playable;
pub mod jukebox_song;
pub mod jukebox_song_player;
pub mod mace_item;
pub mod map_item;
pub mod minecart_item;
pub mod armor_stand_item;
pub mod cushion_item;
pub mod hanging_entity_item;
pub mod name_tag_item;
pub mod potion_item;
pub mod scaffolding_block_item;
pub mod shears_item;
pub mod slot;
pub mod spawn_egg_item;

use wafer_data::Item;

use crate::world::entity::player::stacked_item_contents::StackedItemContents;

pub use item_stack::{ItemStack, read_slot, validate_encodable, write_slot};

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::level::block::BlockHitResult;
use crate::world::level::block::state::pattern::block_in_world::BlockInWorld;
use crate::world::{InteractionHand, InteractionResult};
use context::BlockPlaceContext;

static BLOCK_ITEMS: std::sync::LazyLock<Vec<Option<Box<dyn block_item::BlockItemBehaviour>>>> =
    std::sync::LazyLock::new(|| (0..Item::count()).map(|i| block_item::for_item(Item(i as u16))).collect());

pub fn interact_living_entity(level: &mut ServerLevel, item_stack: &mut ItemStack, player: EntityId, target: EntityId, _hand: InteractionHand) -> InteractionResult {
    if let Some(equippable) = item_stack.get(component::EQUIPPABLE).map(component::values::Equippable::from_nbt)
        && equippable.equip_on_interact
    {
        let result = equip_on_target(level, &equippable, target, item_stack);
        if result != InteractionResult::Pass {
            return result;
        }
    }
    if item_stack.is(wafer_data::items::NAME_TAG) {
        return name_tag_item::interact_living_entity(level, item_stack, player, target);
    }
    if item_stack.get_item().is_instance_of("DyeItem") {
        return dye_item::interact_living_entity(level, item_stack, player, target);
    }
    InteractionResult::Pass
}

fn equip_on_target(level: &mut ServerLevel, equippable: &component::values::Equippable, target: EntityId, item_stack: &mut ItemStack) -> InteractionResult {
    let Some(slot) = wafer_data::EquipmentSlot::by_name(&equippable.slot) else { return InteractionResult::Pass };
    if level.is_equippable_in_slot(target, item_stack, slot) && level.item_by_slot(target, slot).is_none() && level.is_alive(target) {
        let equipped = item_stack.split(1);
        level.living_set_item_slot(target, slot, equipped);
        if level.is_mob(target) {
            level.set_guaranteed_drop(target, slot);
        }
        return InteractionResult::SUCCESS;
    }
    InteractionResult::Pass
}

pub fn block_item_behaviour(item: Item) -> Option<&'static dyn block_item::BlockItemBehaviour> {
    BLOCK_ITEMS[item.0 as usize].as_deref()
}

pub fn use_on(level: &mut ServerLevel, player: EntityId, hand: InteractionHand, hit: BlockHitResult, stack: ItemStack) -> (InteractionResult, Option<ItemStack>) {
    let pos = hit.block_pos;
    if !level.player(player).abilities.may_build && !stack.can_place_on_block_in_adventure_mode(&mut BlockInWorld::new(level, pos, false)) {
        return (InteractionResult::Pass, None);
    }
    let stack_before_use = stack.copy();
    let used_item = stack.item;
    let result = item_use_on(level, player, hand, hit, stack);
    if let InteractionResult::Success { was_item_interaction: true, .. } = result {
        level.award_stat(player, crate::stats::stats::ITEM_USED.get(used_item.0), 1);
        let transform_to = level.player(player).item_in_hand(hand).unwrap_or_else(ItemStack::empty);
        return (result, Some(level.apply_after_use_component_side_effects(player, transform_to, stack_before_use)));
    }
    (result, None)
}

fn item_use_on(level: &mut ServerLevel, player: EntityId, hand: InteractionHand, hit: BlockHitResult, stack: ItemStack) -> InteractionResult {
    match stack.item.info().classes.first() {
        Some(&"BoneMealItem") => return bone_meal_item::use_on(level, player, hand, hit, stack),
        Some(&"BrushItem") => return brush_item::use_on(level, player, hand),
        Some(&"FlintAndSteelItem") => {
            let ctx = BlockPlaceContext::new(level, Some(player), hand, stack, hit);
            return flint_and_steel_item::flint_and_steel_use_on(level, player, hand, &ctx);
        }
        Some(&"FireChargeItem") => {
            let ctx = BlockPlaceContext::new(level, Some(player), hand, stack, hit);
            return flint_and_steel_item::fire_charge_use_on(level, player, hand, &ctx);
        }
        Some(&"SpawnEggItem") => return spawn_egg_item::use_on(level, player, hand, hit, stack),
        Some(&"MinecartItem") => return minecart_item::use_on(level, player, hand, hit, stack),
        Some(&"ArmorStandItem") => return armor_stand_item::use_on(level, player, hand, hit, stack),
        Some(&"CushionItem") => return cushion_item::use_on(level, player, hand, hit, stack),
        Some(&"HangingEntityItem" | &"ItemFrameItem") => return hanging_entity_item::use_on(level, player, hand, hit, stack),
        Some(&"HoneycombItem") => return honeycomb_item::use_on(level, player, hand, hit, stack),
        Some(&"EndCrystalItem") => return end_crystal_item::use_on(level, player, hand, hit, stack),
        Some(&"CompassItem") => return compass_item::use_on(level, player, hand, hit, stack),
        Some(&"EnderEyeItem") => return ender_eye_item::use_on(level, player, hand, hit, stack),
        Some(&"FireworkRocketItem") => return firework_rocket_item::use_on(level, player, hand, hit, stack),
        Some(&"PotionItem") => return potion_item::use_on(level, player, hand, hit, stack),
        Some(&"ShearsItem") => return shears_item::use_on(level, player, hand, hit, stack),
        Some(&"LeadItem") => {
            if crate::world::level::BlockGetter::block_state(level, hit.block_pos).is(&wafer_data::block_tags::FENCES) {
                return crate::world::entity::decoration::leash_fence_knot_entity::bind_player_mobs(level, player, hit.block_pos);
            }
            return InteractionResult::Pass;
        }
        Some(&"MapItem") => {
            if let Some(result) = map_item::use_on(level, &hit, &stack) {
                return result;
            }
        }
        _ => {}
    }
    match &BLOCK_ITEMS[stack.item.0 as usize] {
        Some(item) => block_item::use_on(item.as_ref(), level, player, hand, hit, stack),
        None => default_use_on(level, player, hand, hit, stack),
    }
}

pub(super) fn default_use_on(level: &mut ServerLevel, player: EntityId, hand: InteractionHand, hit: BlockHitResult, stack: ItemStack) -> InteractionResult {
    match stack.get(component::BLOCK_TRANSFORMER).and_then(crate::core::component::block_transformer::BlockTransformer::get) {
        Some(block_transformer) => {
            let ctx = BlockPlaceContext::new(level, Some(player), hand, stack, hit);
            block_transformer.transform_block(level, &ctx)
        }
        None => InteractionResult::Pass,
    }
}

pub fn use_item(level: &mut ServerLevel, player: EntityId, hand: InteractionHand, stack: ItemStack) -> (InteractionResult, Option<ItemStack>) {
    let stack_before_use = stack.copy();
    let is_instantly_used = get_use_duration(&stack) <= 0;
    let (result, transformed) = item_use(level, player, hand, stack);
    if is_instantly_used && let InteractionResult::Success { .. } = result {
        let transform_to = transformed.or_else(|| level.player(player).item_in_hand(hand)).unwrap_or_else(ItemStack::empty);
        return (result, Some(level.apply_after_use_component_side_effects(player, transform_to, stack_before_use)));
    }
    (result, transformed)
}

fn item_use(level: &mut ServerLevel, player: EntityId, hand: InteractionHand, stack: ItemStack) -> (InteractionResult, Option<ItemStack>) {
    if stack.item.is_instance_of("SpawnEggItem") {
        return spawn_egg_item::use_item(level, player, hand);
    }
    if stack.item.is_instance_of("EnderEyeItem") {
        return ender_eye_item::use_item(level, player, hand);
    }
    if stack.item.is_instance_of("EmptyMapItem") {
        return empty_map_item::use_item(level, player, hand);
    }
    if stack.item.is_instance_of("BowItem") {
        return bow_item::use_item(level, player, hand);
    }
    if stack.item.is_instance_of("CrossbowItem") {
        return crossbow_item::use_item(level, player, hand);
    }
    if stack.item.is_instance_of("TridentItem") {
        return trident_item::use_item(level, player, hand);
    }
    if stack.item.is_instance_of("SnowballItem") {
        return snowball_item::use_item(level, player, hand);
    }
    if stack.item.is_instance_of("FishingRodItem") {
        return fishing_rod_item::use_item(level, player, hand);
    }
    if stack.item.is_instance_of("WindChargeItem") {
        return wind_charge_item::use_item(level, player, hand);
    }
    if stack.item.is_instance_of("EggItem") {
        return egg_item::use_item(level, player, hand);
    }
    if stack.item.is_instance_of("EnderpearlItem") {
        return enderpearl_item::use_item(level, player, hand);
    }
    if stack.item.is_instance_of("ExperienceBottleItem") {
        return experience_bottle_item::use_item(level, player, hand);
    }
    if stack.item.is_instance_of("ThrowablePotionItem") {
        return throwable_potion_item::use_item(level, player, hand);
    }
    if stack.item.is_instance_of("FireworkRocketItem") {
        return firework_rocket_item::use_item(level, player, hand);

    }
    if stack.item.is_instance_of("SpyglassItem") {
        return spyglass_item::use_item(level, player, hand);
    }
    if stack.item.is_instance_of("BundleItem") {
        return bundle_item::use_item(level, player, hand);
    }
    if stack.item.is_instance_of("InstrumentItem") {
        return instrument_item::use_item(level, player, hand);
    }
    if stack.item.is_instance_of("WrittenBookItem") || stack.item.is_instance_of("WritableBookItem") {
        return written_book_item::use_item(level, player, hand);
    }
    if stack.item.is_instance_of("KnowledgeBookItem") {
        return knowledge_book_item::use_item(level, player, hand);
    }
    if stack.item.is_instance_of("BottleItem") {
        return bottle_item::use_item(level, player, hand);
    }
    if stack.item.is_instance_of("BoatItem") {
        return boat_item::use_item(level, player, hand);
    }
    if stack.item.is_instance_of("FoodOnAStickItem") {
        return food_on_a_stick_item::use_item(level, player, hand);
    }
    match bucket_item::BucketItem::for_item(stack.item) {
        Some(bucket) => bucket.use_item(level, player, hand),
        None => default_use(level, player, hand, stack),
    }
}

pub(super) fn default_use(level: &mut ServerLevel, player: EntityId, hand: InteractionHand, stack: ItemStack) -> (InteractionResult, Option<ItemStack>) {
    if let Some(consumable) = stack.get(component::CONSUMABLE).map(component::values::Consumable::from_nbt) {
        return level.start_consuming(player, hand, stack.clone(), &consumable);
    }
    if let Some(equippable) = stack.get(component::EQUIPPABLE).map(component::values::Equippable::from_nbt)
        && equippable.swappable
    {
        return swap_with_equipment_slot(level, &equippable, stack, hand, player);
    }
    if stack.has(component::BLOCKS_ATTACKS) {
        level.start_using_item(player, hand);
        return (InteractionResult::CONSUME, None);
    }
    if let Some(kinetic_weapon) = component::kinetic_weapon::KineticWeapon::of(&stack) {
        level.start_using_item(player, hand);
        kinetic_weapon.make_sound(level, player);
        return (InteractionResult::CONSUME, None);
    }
    (InteractionResult::Pass, None)
}

fn swap_with_equipment_slot(
    level: &mut ServerLevel,
    equippable: &component::values::Equippable,
    mut in_hand: ItemStack,
    hand: InteractionHand,
    player: EntityId,
) -> (InteractionResult, Option<ItemStack>) {
    let Some(slot) = wafer_data::EquipmentSlot::by_name(&equippable.slot) else { return (InteractionResult::Pass, None) };
    let entity_type = level.entity(player).entity_type;
    if !(level.can_use_slot(player, slot) && equippable.can_be_equipped_by(entity_type)) {
        return (InteractionResult::Pass, None);
    }
    let in_equipment_slot = level.item_by_slot(player, slot).unwrap_or_else(ItemStack::empty);
    let is_creative = level.player(player).game_mode.game_type == crate::server::level::game_mode::GameType::Creative;
    if (enchantment::enchantment_helper::has_prevent_armor_change(&in_equipment_slot) && !is_creative) || ItemStack::is_same_item_same_components(&in_hand, &in_equipment_slot) {
        return (InteractionResult::Fail, None);
    }
    level.award_stat(player, crate::stats::stats::ITEM_USED.get(in_hand.get_item().0), 1);
    if in_hand.count <= 1 {
        let swapped_to_equipment = in_hand.copy();
        if !is_creative {
            in_hand.count = 0;
        }
        let swapped_to_hand = if in_equipment_slot.is_empty() { in_hand } else { in_equipment_slot };
        level.living_set_item_slot(player, slot, swapped_to_equipment);
        return (InteractionResult::SUCCESS, Some(swapped_to_hand));
    }
    let mut swapped_to_inventory = in_equipment_slot;
    let swapped_to_equipment = in_hand.copy_with_count(1);
    in_hand.consume(1, level, Some(player));
    level.player_mut(player).set_item_in_hand(hand, Some(in_hand.clone()));
    level.living_set_item_slot(player, slot, swapped_to_equipment);
    let infinite_materials = level.has_infinite_materials(player);
    if !level.player_mut(player).inventory.add_stack(&mut swapped_to_inventory, infinite_materials).0 {
        level.player_drop(player, swapped_to_inventory, false, crate::util::Prediction::Predicted);
    }
    (InteractionResult::SUCCESS, Some(in_hand))
}

pub fn can_destroy_block(level: &ServerLevel, item_stack: Option<&ItemStack>, user: EntityId) -> bool {
    match item_stack.and_then(ItemStack::tool) {
        Some(tool) if !tool.can_destroy_blocks_in_creative => !(level.is_player(user) && level.player(user).abilities.instabuild),
        _ => true,
    }
}

pub fn get_use_duration(stack: &ItemStack) -> i32 {
    if stack.item.is_instance_of("BrushItem") {
        return brush_item::USE_DURATION;
    }
    if stack.item.is_instance_of("BundleItem") {
        return bundle_item::TICKS_MAX_THROW_DURATION;
    }
    if stack.item.is_instance_of("BowItem") {
        return bow_item::USE_DURATION;
    }
    if stack.item.is_instance_of("CrossbowItem") {
        return crossbow_item::USE_DURATION;
    }
    if stack.item.is_instance_of("TridentItem") {
        return trident_item::USE_DURATION;
    }
    if stack.item.is_instance_of("SpyglassItem") {
        return spyglass_item::USE_DURATION;
    }
    if stack.item.is_instance_of("InstrumentItem") {
        return instrument_item::get_use_duration(stack);
    }
    match stack.get(component::CONSUMABLE).map(component::values::Consumable::from_nbt) {
        Some(consumable) => consumable.consume_ticks(),
        None => {
            if !stack.has(component::BLOCKS_ATTACKS) && !stack.has(component::KINETIC_WEAPON) {
                0
            } else {
                72000
            }
        }
    }
}

pub fn release_using(level: &mut ServerLevel, stack: &ItemStack, entity: EntityId, remaining_time: i32) -> bool {
    if stack.item.is_instance_of("BowItem") {
        return bow_item::release_using(level, stack, entity, remaining_time);
    }
    if stack.item.is_instance_of("CrossbowItem") {
        return crossbow_item::release_using(level, stack, entity, remaining_time);
    }
    if stack.item.is_instance_of("TridentItem") {
        return trident_item::release_using(level, stack, entity, remaining_time);
    }
    if stack.item.is_instance_of("SpyglassItem") {
        return spyglass_item::release_using(level, entity);
    }
    false
}

pub fn finish_using_item(item_stack: ItemStack, level: &mut ServerLevel, entity: EntityId) -> ItemStack {
    if item_stack.item.is_instance_of("SpyglassItem") {
        return spyglass_item::finish_using_item(item_stack, level, entity);
    }
    match item_stack.get(component::CONSUMABLE).map(component::values::Consumable::from_nbt) {
        Some(consumable) => level.on_consume(entity, item_stack, &consumable),
        None => item_stack,
    }
}

pub fn hurt_enemy(level: &mut ServerLevel, item_stack: &ItemStack, mob: EntityId, attacker: EntityId) {
    if mace_item::is_mace(item_stack) {
        mace_item::hurt_enemy(level, mob, attacker);
    }
}

pub fn post_hurt_enemy(level: &mut ServerLevel, item_stack: &ItemStack, attacker: EntityId) {
    if mace_item::is_mace(item_stack) {
        mace_item::post_hurt_enemy(level, attacker);
    }
}

pub fn get_attack_damage_bonus(level: &mut ServerLevel, item_stack: &ItemStack, victim: EntityId, damage_source: &crate::world::damagesource::DamageSource) -> f32 {
    if mace_item::is_mace(item_stack) { mace_item::get_attack_damage_bonus(level, victim, damage_source) } else { 0.0 }
}

pub fn get_item_damage_source(level: &ServerLevel, item_stack: &ItemStack, attacker: EntityId) -> Option<crate::world::damagesource::DamageSource> {
    if mace_item::is_mace(item_stack) { mace_item::get_item_damage_source(level, attacker) } else { None }
}

pub fn on_use_tick(level: &mut ServerLevel, entity: EntityId, stack: &mut ItemStack, ticks_remaining: i32) {
    if stack.item.is_instance_of("BrushItem") {
        brush_item::on_use_tick(level, entity, stack, ticks_remaining);
    }
    if stack.item.is_instance_of("BundleItem") {
        bundle_item::on_use_tick(level, entity, stack, ticks_remaining);
    }
    if stack.item.is_instance_of("CrossbowItem") {
        crossbow_item::on_use_tick(level, entity, stack, ticks_remaining);
    }
}

pub fn on_destroyed(level: &mut ServerLevel, entity: EntityId, item: &mut ItemStack) {
    if item.item.is_instance_of("BundleItem") {
        bundle_item::on_destroyed(level, entity, item);
    } else if item.item.is_instance_of("BlockItem") {
        block_item::on_destroyed(level, entity, item);
    }
}

pub fn use_on_release(stack: &ItemStack) -> bool {
    crossbow_item::use_on_release(stack)
}

pub fn inventory_tick(level: &mut ServerLevel, owner: EntityId, slot_index: usize, slot: Option<wafer_data::EquipmentSlot>) {
    let Some(item_stack) = level.player(owner).inventory.slots[slot_index].as_ref().filter(|stack| !stack.is_empty()) else { return };
    if item_stack.item.is_instance_of("MapItem") {
        let item_stack = item_stack.clone();
        map_item::inventory_tick(&item_stack, level, owner, slot);
    } else if item_stack.item.is_instance_of("CompassItem") {
        let old_tracker = item_stack.get(component::LODESTONE_TRACKER).cloned();
        let mut new_stack = item_stack.clone();
        compass_item::inventory_tick(&mut new_stack, level);
        if new_stack.get(component::LODESTONE_TRACKER) != old_tracker.as_ref() {
            level.player_mut(owner).inventory.slots[slot_index] = Some(new_stack);
        }
    }
}

pub fn on_crafted_by(item_stack: &mut ItemStack, level: &mut ServerLevel, _player: EntityId) {
    on_crafted_post_process(item_stack, level);
}

pub fn on_crafted_post_process(item_stack: &mut ItemStack, level: &mut ServerLevel) {
    if item_stack.item.is_instance_of("MapItem") {
        map_item::on_crafted_post_process(item_stack, level);
    }
}

pub fn can_fit_inside_container_items(item: Item) -> bool {
    !item.info().block.is_some_and(|block| block.is_instance_of("ShulkerBoxBlock"))
}

pub const HOTBAR_START: usize = 36;
pub const MAIN_START: usize = 9;
pub const OFFHAND: usize = 45;
pub const SLOTS: usize = 46;

pub fn inventory_slot(index: usize) -> usize {
    match index {
        0..=8 => HOTBAR_START + index,
        36..=39 => 44 - index,
        40 => OFFHAND,
        _ => index,
    }
}

pub fn inventory_index(slot: usize) -> Option<usize> {
    match slot {
        36..=44 => Some(slot - HOTBAR_START),
        9..=35 => Some(slot),
        5..=8 => Some(44 - slot),
        OFFHAND => Some(40),
        _ => None,
    }
}

pub fn protocol_slot(storage: i8) -> Option<usize> {
    match storage {
        0..=8 => Some(storage as usize + 36),
        9..=35 => Some(storage as usize),
        100..=103 => Some(108 - storage as usize),
        -106 => Some(OFFHAND),
        _ => None,
    }
}

pub struct Inventory {
    pub slots: [Option<ItemStack>; SLOTS],
    pub selected: usize,
}

impl Inventory {
    pub fn new() -> Inventory {
        Inventory { slots: std::array::from_fn(|_| None), selected: 0 }
    }

    pub fn selected_slot(&self) -> usize {
        HOTBAR_START + self.selected
    }

    pub fn held(&self) -> Option<&ItemStack> {
        self.slots[self.selected_slot()].as_ref()
    }

    fn item(&self, index: usize) -> Option<&ItemStack> {
        self.slots[inventory_slot(index)].as_ref().filter(|s| !s.is_empty())
    }

    pub fn is_hotbar_slot(index: usize) -> bool {
        index < 9
    }

    pub fn is_usable_for_crafting(item_stack: &ItemStack) -> bool {
        !item_stack.is_damaged() && !item_stack.is_enchanted() && !item_stack.has(component::CUSTOM_NAME)
    }

    pub fn find_slot_matching_item(&self, stack: &ItemStack) -> Option<usize> {
        (0..36).find(|&i| self.item(i).is_some_and(|item| ItemStack::is_same_item_same_components(stack, item)))
    }

    fn get_free_slot(&self) -> Option<usize> {
        (0..36).find(|&i| self.item(i).is_none())
    }

    pub fn find_slot_matching_crafting_ingredient(&self, item: Item, existing_item: &ItemStack) -> Option<usize> {
        (0..36).find(|&i| {
            self.item(i).is_some_and(|inventory_item_stack| {
                inventory_item_stack.is(item)
                    && Inventory::is_usable_for_crafting(inventory_item_stack)
                    && (existing_item.is_empty() || ItemStack::is_same_item_same_components(existing_item, inventory_item_stack))
            })
        })
    }

    pub fn get_item(&self, index: usize) -> ItemStack {
        self.item(index).cloned().unwrap_or_else(ItemStack::empty)
    }

    pub fn remove_item(&mut self, index: usize, count: i32) -> ItemStack {
        match &mut self.slots[inventory_slot(index)] {
            Some(item_stack) if !item_stack.is_empty() && count > 0 => item_stack.split(count),
            _ => ItemStack::empty(),
        }
    }

    pub fn remove_from_selected(&mut self, all: bool) -> ItemStack {
        let slot = self.selected_slot();
        let Some(selected_item) = self.slots[slot].as_mut().filter(|s| !s.is_empty()) else { return ItemStack::empty() };
        let count = if all { selected_item.count } else { 1 };
        let removed = selected_item.split(count);
        if selected_item.is_empty() {
            self.slots[slot] = None;
        }
        removed
    }

    pub fn remove_item_no_update(&mut self, index: usize) -> ItemStack {
        self.slots[inventory_slot(index)].take().unwrap_or_else(ItemStack::empty)
    }

    pub fn fill_stacked_contents(&self, contents: &mut StackedItemContents) {
        (0..36).filter_map(|i| self.item(i)).for_each(|item_stack| contents.account_simple_stack(item_stack));
    }

    pub fn get_suitable_hotbar_slot(&self) -> usize {
        let hotbar = || (0..9).map(|slot| (self.selected + slot) % 9);
        hotbar()
            .find(|&i| self.item(i).is_none())
            .or_else(|| hotbar().find(|&i| !self.item(i).is_some_and(ItemStack::is_enchanted)))
            .unwrap_or(self.selected)
    }

    pub fn pick_slot(&mut self, index: usize) -> [usize; 2] {
        self.selected = self.get_suitable_hotbar_slot();
        let (held, picked) = (inventory_slot(self.selected), inventory_slot(index));
        self.slots.swap(held, picked);
        [held, picked]
    }

    pub fn add_and_pick_item(&mut self, stack: ItemStack) -> Vec<usize> {
        self.selected = self.get_suitable_hotbar_slot();
        let held = inventory_slot(self.selected);
        let mut changed = vec![held];
        if self.item(self.selected).is_some()
            && let Some(free) = self.get_free_slot()
        {
            let free = inventory_slot(free);
            self.slots[free] = self.slots[held].clone();
            changed.push(free);
        }
        self.slots[held] = Some(stack);
        changed
    }

    fn storage_order() -> impl Iterator<Item = usize> {
        (HOTBAR_START..HOTBAR_START + 9).chain(MAIN_START..HOTBAR_START)
    }

    pub fn add(&mut self, mut stack: ItemStack) -> (Vec<usize>, i32) {
        let mut changed = Vec::new();
        let max = stack.max_stack_size();
        let partial = std::iter::once(self.selected_slot()).chain([OFFHAND]).chain(Self::storage_order());
        for slot in partial.collect::<Vec<_>>() {
            if stack.count == 0 {
                break;
            }
            if let Some(existing) = &mut self.slots[slot] {
                if ItemStack::is_same_item_same_components(existing, &stack) && existing.count < max {
                    let moved = stack.count.min(max - existing.count);
                    existing.count += moved;
                    stack.count -= moved;
                    if !changed.contains(&slot) {
                        changed.push(slot);
                    }
                }
            }
        }
        for slot in Self::storage_order() {
            if stack.count == 0 {
                break;
            }
            if self.slots[slot].is_none() {
                let moved = stack.count.min(max);
                self.slots[slot] = Some(stack.copy_with_count(moved));
                stack.count -= moved;
                changed.push(slot);
            }
        }
        (changed, stack.count)
    }

    pub(crate) fn free_slot(&self) -> Option<usize> {
        Self::storage_order().find(|&slot| self.slots[slot].as_ref().is_none_or(|s| s.is_empty()))
    }

    pub(crate) fn slot_with_remaining_space(&self, stack: &ItemStack) -> Option<usize> {
        let has_space = |slot: usize| {
            self.slots[slot].as_ref().is_some_and(|s| !s.is_empty() && ItemStack::is_same_item_same_components(s, stack) && s.is_stackable() && s.count < s.max_stack_size().min(99))
        };
        std::iter::once(self.selected_slot()).chain([OFFHAND]).chain(Self::storage_order()).find(|&slot| has_space(slot))
    }

    fn add_resource(&mut self, stack: &ItemStack, changed: &mut Vec<usize>) -> i32 {
        let Some(slot) = self.slot_with_remaining_space(stack).or_else(|| self.free_slot()) else { return stack.count };
        if self.slots[slot].as_ref().is_none_or(|s| s.is_empty()) {
            self.slots[slot] = Some(stack.copy_with_count(0));
        }
        let in_slot = self.slots[slot].as_mut().unwrap();
        let to_add = stack.count.min(in_slot.max_stack_size().min(99) - in_slot.count);
        in_slot.count += to_add;
        if !changed.contains(&slot) {
            changed.push(slot);
        }
        stack.count - to_add
    }

    pub fn add_stack(&mut self, stack: &mut ItemStack, infinite_materials: bool) -> (bool, Vec<usize>) {
        let mut changed = Vec::new();
        if stack.is_empty() {
            return (false, changed);
        }
        if stack.is_damaged() {
            return match self.free_slot() {
                Some(slot) => {
                    self.slots[slot] = Some(stack.clone());
                    stack.count = 0;
                    changed.push(slot);
                    (true, changed)
                }
                None if infinite_materials => {
                    stack.count = 0;
                    (true, changed)
                }
                None => (false, changed),
            };
        }
        let mut last;
        loop {
            last = stack.count;
            stack.count = self.add_resource(stack, &mut changed);
            if stack.is_empty() || stack.count >= last {
                break;
            }
        }
        if stack.count == last && infinite_materials {
            stack.count = 0;
            return (true, changed);
        }
        (stack.count < last, changed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wafer_data::items;
    use wafer_protocol::Writer;

    #[test]
    fn first_pickup_goes_to_first_hotbar_slot() {
        let dirt = items::DIRT;
        let mut inv = Inventory::new();
        assert_eq!(inv.add(ItemStack::new(dirt, 1)), (vec![36], 0));
        assert_eq!(inv.add(ItemStack::new(dirt, 70)), (vec![36, 37], 0));
        assert_eq!(inv.slots[36], Some(ItemStack::new(dirt, 64)));
        assert_eq!(inv.slots[37], Some(ItemStack::new(dirt, 7)));
    }

    #[test]
    fn stacks_merge_only_with_equal_components() {
        let mut named = ItemStack::new(items::DIRT, 3);
        named.set(component::CUSTOM_NAME, Some(wafer_protocol::nbt::Nbt::String("x".into())));
        let mut inv = Inventory::new();
        assert_eq!(inv.add(ItemStack::new(items::DIRT, 1)), (vec![36], 0));
        assert_eq!(inv.add(named.clone()), (vec![37], 0));
        assert_eq!(inv.add(named.copy_with_count(2)), (vec![37], 0));
        assert_eq!(inv.slots[37], Some(named.copy_with_count(5)));
        let mut plain = ItemStack::new(items::DIRT, 1);
        plain.set(component::MAX_STACK_SIZE, Some(wafer_protocol::nbt::Nbt::Int(64)));
        assert!(plain.components_patch().is_empty());
        assert!(ItemStack::is_same_item_same_components(&plain, &ItemStack::new(items::DIRT, 2)));
    }

    #[test]
    fn slot_encoding_matches_vanilla() {
        let mut w = Writer::new();
        write_slot(&mut w, Some(&ItemStack::new(items::DIRT, 1)));
        assert_eq!(w.as_slice(), &[0x01, 0x37, 0x00, 0x00]);
    }
}
