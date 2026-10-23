use wafer_protocol::nbt::Nbt;

use super::apply_bonus_count::ApplyBonusCount;
use super::apply_explosion_decay::ApplyExplosionDecay;
use super::copy_block_state::CopyBlockState;
use super::copy_components_function::CopyComponentsFunction;
use super::copy_custom_data_function::CopyCustomDataFunction;
use super::copy_name_function::CopyNameFunction;
use super::enchant_randomly_function::EnchantRandomlyFunction;
use super::enchant_with_levels_function::EnchantWithLevelsFunction;
use super::enchanted_count_increase_function::EnchantedCountIncreaseFunction;
use super::exploration_map_function::ExplorationMapFunction;
use super::fill_player_head::FillPlayerHead;
use super::filtered_function::FilteredFunction;
use super::limit_count::LimitCount;
use super::modify_container_contents::ModifyContainerContents;
use super::sequence_function::SequenceFunction;
use super::set_attributes_function::SetAttributesFunction;
use super::set_banner_pattern_function::SetBannerPatternFunction;
use super::set_book_cover_function::SetBookCoverFunction;
use super::set_components_function::SetComponentsFunction;
use super::set_container_contents::SetContainerContents;
use super::set_container_loot_table::SetContainerLootTable;
use super::set_custom_data_function::SetCustomDataFunction;
use super::set_custom_model_data_function::SetCustomModelDataFunction;
use super::set_enchantments_function::SetEnchantmentsFunction;
use super::set_firework_explosion_function::SetFireworkExplosionFunction;
use super::set_fireworks_function::SetFireworksFunction;
use super::set_instrument_function::SetInstrumentFunction;
use super::set_item_count_function::SetItemCountFunction;
use super::set_item_damage_function::SetItemDamageFunction;
use super::set_item_function::SetItemFunction;
use super::set_lore_function::SetLoreFunction;
use super::set_name_function::SetNameFunction;
use super::set_ominous_bottle_amplifier_function::SetOminousBottleAmplifierFunction;
use super::set_potion_function::SetPotionFunction;
use super::set_random_dyes_function::SetRandomDyesFunction;
use super::set_random_potion_function::SetRandomPotionFunction;
use super::set_stew_effect_function::SetStewEffectFunction;
use super::set_writable_book_pages_function::SetWritableBookPagesFunction;
use super::set_written_book_pages_function::SetWrittenBookPagesFunction;
use super::smelt_item_function::SmeltItemFunction;
use super::toggle_tooltips::ToggleTooltips;
use crate::core::holder::{Holder, RegistryElement};
use crate::server::reloadable_server_registries;
use crate::util::codec::{self, DecodeResult};
use crate::world::item::ItemStack;
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::predicates::loot_item_condition::LootItemCondition;
use crate::world::level::storage::loot::validatable::{self, Validatable};
use crate::world::level::storage::loot::validation_context::ValidationContext;
use crate::world::level::storage::loot::parameters::loot_context_params::LootContextParam;

pub enum LootItemFunctionType {
    SetCount(SetItemCountFunction),
    SetItem(SetItemFunction),
    EnchantWithLevels(EnchantWithLevelsFunction),
    EnchantRandomly(EnchantRandomlyFunction),
    SetEnchantments(SetEnchantmentsFunction),
    SetCustomData(SetCustomDataFunction),
    SetComponents(SetComponentsFunction),
    FurnaceSmelt(SmeltItemFunction),
    EnchantedCountIncrease(EnchantedCountIncreaseFunction),
    SetDamage(SetItemDamageFunction),
    SetAttributes(SetAttributesFunction),
    SetName(SetNameFunction),
    ExplorationMap(ExplorationMapFunction),
    SetStewEffect(SetStewEffectFunction),
    CopyName(CopyNameFunction),
    SetContents(SetContainerContents),
    ModifyContents(ModifyContainerContents),
    Filtered(FilteredFunction),
    LimitCount(LimitCount),
    ApplyBonus(ApplyBonusCount),
    SetLootTable(SetContainerLootTable),
    ExplosionDecay(ApplyExplosionDecay),
    SetLore(SetLoreFunction),
    FillPlayerHead(FillPlayerHead),
    CopyCustomData(CopyCustomDataFunction),
    CopyState(CopyBlockState),
    SetBannerPattern(SetBannerPatternFunction),
    SetPotion(SetPotionFunction),
    SetRandomDyes(SetRandomDyesFunction),
    SetRandomPotion(SetRandomPotionFunction),
    SetInstrument(SetInstrumentFunction),
    Sequence(SequenceFunction),
    CopyComponents(CopyComponentsFunction),
    SetFireworks(SetFireworksFunction),
    SetFireworkExplosion(SetFireworkExplosionFunction),
    SetBookCover(SetBookCoverFunction),
    SetWrittenBookPages(SetWrittenBookPagesFunction),
    SetWritableBookPages(SetWritableBookPagesFunction),
    ToggleTooltips(ToggleTooltips),
    SetOminousBottleAmplifier(SetOminousBottleAmplifierFunction),
    SetCustomModelData(SetCustomModelDataFunction),
    Discard,
}

pub struct LootItemFunction {
    condition: Option<Holder<LootItemCondition>>,
    function: LootItemFunctionType,
}

impl RegistryElement for LootItemFunction {
    const REGISTRY: &'static str = "minecraft:item_modifier";

    fn contains_key(key: &str) -> bool {
        reloadable_server_registries::contains_key(Self::REGISTRY, key)
    }

    fn lookup(key: &str) -> Option<&'static LootItemFunction> {
        reloadable_server_registries::get().item_modifiers.get(key)
    }
}

impl LootItemFunction {
    pub fn decode_holder(tag: &Nbt) -> DecodeResult<Holder<LootItemFunction>> {
        Holder::decode_partial(tag, LootItemFunction::decode_direct)
    }

    pub fn decode_direct(tag: &Nbt) -> DecodeResult<LootItemFunction> {
        codec::either_result(LootItemFunction::decode_typed(tag), || SequenceFunction::decode_inline(tag)).map(|function| match function {
            Ok(function) => function,
            Err(sequence) => LootItemFunction { condition: None, function: LootItemFunctionType::Sequence(sequence) },
        })
    }

    pub fn decode_holder_set(tag: &Nbt) -> DecodeResult<Vec<Holder<LootItemFunction>>> {
        codec::holder_set_partial(tag, Self::REGISTRY, |t| Holder::decode_partial(t, LootItemFunction::decode_typed))
    }

    pub fn decode_typed(tag: &Nbt) -> DecodeResult<LootItemFunction> {
        if let Err(error) = codec::map(tag) {
            return DecodeResult::error(error);
        }
        let ty = match codec::decode_field(tag, "type", codec::identifier) {
            Ok(ty) => ty,
            Err(error) => return DecodeResult::error(error),
        };
        let function = match ty.as_str() {
            "minecraft:set_count" => SetItemCountFunction::decode(tag).map(LootItemFunctionType::SetCount),
            "minecraft:set_item" => SetItemFunction::decode(tag).map(LootItemFunctionType::SetItem),
            "minecraft:enchant_with_levels" => EnchantWithLevelsFunction::decode(tag).map(LootItemFunctionType::EnchantWithLevels),
            "minecraft:enchant_randomly" => EnchantRandomlyFunction::decode(tag).map(LootItemFunctionType::EnchantRandomly),
            "minecraft:set_enchantments" => SetEnchantmentsFunction::decode(tag).map(LootItemFunctionType::SetEnchantments),
            "minecraft:set_custom_data" => SetCustomDataFunction::decode(tag).map(LootItemFunctionType::SetCustomData),
            "minecraft:set_components" => SetComponentsFunction::decode(tag).map(LootItemFunctionType::SetComponents),
            "minecraft:furnace_smelt" => SmeltItemFunction::decode(tag).map(LootItemFunctionType::FurnaceSmelt),
            "minecraft:enchanted_count_increase" => EnchantedCountIncreaseFunction::decode(tag).map(LootItemFunctionType::EnchantedCountIncrease),
            "minecraft:set_damage" => SetItemDamageFunction::decode(tag).map(LootItemFunctionType::SetDamage),
            "minecraft:set_attributes" => SetAttributesFunction::decode(tag).map(LootItemFunctionType::SetAttributes),
            "minecraft:set_name" => SetNameFunction::decode(tag).map(LootItemFunctionType::SetName),
            "minecraft:exploration_map" => ExplorationMapFunction::decode(tag).map(LootItemFunctionType::ExplorationMap),
            "minecraft:set_stew_effect" => SetStewEffectFunction::decode(tag).map(LootItemFunctionType::SetStewEffect),
            "minecraft:copy_name" => CopyNameFunction::decode(tag).map(LootItemFunctionType::CopyName),
            "minecraft:set_contents" => SetContainerContents::decode(tag).map(LootItemFunctionType::SetContents),
            "minecraft:modify_contents" => ModifyContainerContents::decode(tag).map(LootItemFunctionType::ModifyContents),
            "minecraft:filtered" => FilteredFunction::decode(tag).map(LootItemFunctionType::Filtered),
            "minecraft:limit_count" => LimitCount::decode(tag).map(LootItemFunctionType::LimitCount),
            "minecraft:apply_bonus" => ApplyBonusCount::decode(tag).map(LootItemFunctionType::ApplyBonus),
            "minecraft:set_loot_table" => SetContainerLootTable::decode(tag).map(LootItemFunctionType::SetLootTable),
            "minecraft:explosion_decay" => DecodeResult::success(LootItemFunctionType::ExplosionDecay(ApplyExplosionDecay)),
            "minecraft:set_lore" => SetLoreFunction::decode(tag).map(LootItemFunctionType::SetLore),
            "minecraft:fill_player_head" => FillPlayerHead::decode(tag).map(LootItemFunctionType::FillPlayerHead),
            "minecraft:copy_custom_data" => CopyCustomDataFunction::decode(tag).map(LootItemFunctionType::CopyCustomData),
            "minecraft:copy_state" => CopyBlockState::decode(tag).map(LootItemFunctionType::CopyState),
            "minecraft:set_banner_pattern" => SetBannerPatternFunction::decode(tag).map(LootItemFunctionType::SetBannerPattern),
            "minecraft:set_potion" => SetPotionFunction::decode(tag).map(LootItemFunctionType::SetPotion),
            "minecraft:set_random_dyes" => SetRandomDyesFunction::decode(tag).map(LootItemFunctionType::SetRandomDyes),
            "minecraft:set_random_potion" => SetRandomPotionFunction::decode(tag).map(LootItemFunctionType::SetRandomPotion),
            "minecraft:set_instrument" => SetInstrumentFunction::decode(tag).map(LootItemFunctionType::SetInstrument),
            "minecraft:sequence" => SequenceFunction::decode(tag).map(LootItemFunctionType::Sequence),
            "minecraft:copy_components" => CopyComponentsFunction::decode(tag).map(LootItemFunctionType::CopyComponents),
            "minecraft:set_fireworks" => SetFireworksFunction::decode(tag).map(LootItemFunctionType::SetFireworks),
            "minecraft:set_firework_explosion" => SetFireworkExplosionFunction::decode(tag).map(LootItemFunctionType::SetFireworkExplosion),
            "minecraft:set_book_cover" => SetBookCoverFunction::decode(tag).map(LootItemFunctionType::SetBookCover),
            "minecraft:set_written_book_pages" => SetWrittenBookPagesFunction::decode(tag).map(LootItemFunctionType::SetWrittenBookPages),
            "minecraft:set_writable_book_pages" => SetWritableBookPagesFunction::decode(tag).map(LootItemFunctionType::SetWritableBookPages),
            "minecraft:toggle_tooltips" => ToggleTooltips::decode(tag).map(LootItemFunctionType::ToggleTooltips),
            "minecraft:set_ominous_bottle_amplifier" => SetOminousBottleAmplifierFunction::decode(tag).map(LootItemFunctionType::SetOminousBottleAmplifier),
            "minecraft:set_custom_model_data" => SetCustomModelDataFunction::decode(tag).map(LootItemFunctionType::SetCustomModelData),
            "minecraft:discard" => DecodeResult::success(LootItemFunctionType::Discard),
            other => return DecodeResult::error(format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:loot_function_type]: {other}")),
        };
        let mut record = codec::Record::default();
        let condition = record.optional(tag, "condition", LootItemCondition::decode_holder_partial);
        let function = record.absorb(function);
        record.build(|| Some(LootItemFunction { condition, function: function? }))
    }

    pub fn apply(&self, item_stack: ItemStack, context: &mut LootContext) -> ItemStack {
        if let Some(condition) = &self.condition
            && !condition.value().test(context)
        {
            return item_stack;
        }
        self.run(item_stack, context)
    }

    fn run(&self, item_stack: ItemStack, context: &mut LootContext) -> ItemStack {
        match &self.function {
            LootItemFunctionType::SetCount(f) => f.run(item_stack, context),
            LootItemFunctionType::SetItem(f) => f.run(item_stack, context),
            LootItemFunctionType::EnchantWithLevels(f) => f.run(item_stack, context),
            LootItemFunctionType::EnchantRandomly(f) => f.run(item_stack, context),
            LootItemFunctionType::SetEnchantments(f) => f.run(item_stack, context),
            LootItemFunctionType::SetCustomData(f) => f.run(item_stack, context),
            LootItemFunctionType::SetComponents(f) => f.run(item_stack, context),
            LootItemFunctionType::FurnaceSmelt(f) => f.run(item_stack, context),
            LootItemFunctionType::EnchantedCountIncrease(f) => f.run(item_stack, context),
            LootItemFunctionType::SetDamage(f) => f.run(item_stack, context),
            LootItemFunctionType::SetAttributes(f) => f.run(item_stack, context),
            LootItemFunctionType::SetName(f) => f.run(item_stack, context),
            LootItemFunctionType::ExplorationMap(f) => f.run(item_stack, context),
            LootItemFunctionType::SetStewEffect(f) => f.run(item_stack, context),
            LootItemFunctionType::CopyName(f) => f.run(item_stack, context),
            LootItemFunctionType::SetContents(f) => f.run(item_stack, context),
            LootItemFunctionType::ModifyContents(f) => f.run(item_stack, context),
            LootItemFunctionType::Filtered(f) => f.run(item_stack, context),
            LootItemFunctionType::LimitCount(f) => f.run(item_stack, context),
            LootItemFunctionType::ApplyBonus(f) => f.run(item_stack, context),
            LootItemFunctionType::SetLootTable(f) => f.run(item_stack, context),
            LootItemFunctionType::ExplosionDecay(f) => f.run(item_stack, context),
            LootItemFunctionType::SetLore(f) => f.run(item_stack, context),
            LootItemFunctionType::FillPlayerHead(f) => f.run(item_stack, context),
            LootItemFunctionType::CopyCustomData(f) => f.run(item_stack, context),
            LootItemFunctionType::CopyState(f) => f.run(item_stack, context),
            LootItemFunctionType::SetBannerPattern(f) => f.run(item_stack, context),
            LootItemFunctionType::SetPotion(f) => f.run(item_stack, context),
            LootItemFunctionType::SetRandomDyes(f) => f.run(item_stack, context),
            LootItemFunctionType::SetRandomPotion(f) => f.run(item_stack, context),
            LootItemFunctionType::SetInstrument(f) => f.run(item_stack, context),
            LootItemFunctionType::Sequence(f) => f.run(item_stack, context),
            LootItemFunctionType::CopyComponents(f) => f.run(item_stack, context),
            LootItemFunctionType::SetFireworks(f) => f.run(item_stack, context),
            LootItemFunctionType::SetFireworkExplosion(f) => f.run(item_stack, context),
            LootItemFunctionType::SetBookCover(f) => f.run(item_stack, context),
            LootItemFunctionType::SetWrittenBookPages(f) => f.run(item_stack, context),
            LootItemFunctionType::SetWritableBookPages(f) => f.run(item_stack, context),
            LootItemFunctionType::ToggleTooltips(f) => f.run(item_stack, context),
            LootItemFunctionType::SetOminousBottleAmplifier(f) => f.run(item_stack, context),
            LootItemFunctionType::SetCustomModelData(f) => f.run(item_stack, context),
            LootItemFunctionType::Discard => ItemStack::empty(),
        }
    }
}

pub fn decorate<'a>(
    function: &Option<Holder<LootItemFunction>>,
    item_stack: ItemStack,
    context: &mut LootContext<'a>,
    output: &mut dyn FnMut(&mut LootContext<'a>, ItemStack),
) {
    match function {
        Some(function) => {
            let item_stack = function.value().apply(item_stack, context);
            output(context, item_stack);
        }
        None => output(context, item_stack),
    }
}

impl LootItemFunctionType {
    fn get_referenced_context_params(&self) -> Vec<LootContextParam> {
        match self {
            LootItemFunctionType::ApplyBonus(function) => function.get_referenced_context_params(),
            LootItemFunctionType::CopyState(function) => function.get_referenced_context_params(),
            LootItemFunctionType::CopyComponents(function) => function.get_referenced_context_params(),
            LootItemFunctionType::CopyName(function) => function.get_referenced_context_params(),
            LootItemFunctionType::EnchantedCountIncrease(function) => function.get_referenced_context_params(),
            LootItemFunctionType::EnchantRandomly(function) => function.get_referenced_context_params(),
            LootItemFunctionType::EnchantWithLevels(function) => function.get_referenced_context_params(),
            LootItemFunctionType::ExplorationMap(function) => function.get_referenced_context_params(),
            LootItemFunctionType::FillPlayerHead(function) => function.get_referenced_context_params(),
            LootItemFunctionType::SetLore(function) => function.get_referenced_context_params(),
            LootItemFunctionType::SetName(function) => function.get_referenced_context_params(),
            _ => Vec::new(),
        }
    }

    fn validate(&self, context: &ValidationContext) {
        match self {
            LootItemFunctionType::CopyCustomData(function) => function.validate(context),
            LootItemFunctionType::EnchantedCountIncrease(function) => function.validate(context),
            LootItemFunctionType::EnchantWithLevels(function) => function.validate(context),
            LootItemFunctionType::Filtered(function) => function.validate(context),
            LootItemFunctionType::LimitCount(function) => function.validate(context),
            LootItemFunctionType::ModifyContents(function) => function.validate(context),
            LootItemFunctionType::Sequence(function) => function.validate(context),
            LootItemFunctionType::SetAttributes(function) => function.validate(context),
            LootItemFunctionType::SetContents(function) => function.validate(context),
            LootItemFunctionType::SetCustomModelData(function) => function.validate(context),
            LootItemFunctionType::SetEnchantments(function) => function.validate(context),
            LootItemFunctionType::SetCount(function) => function.validate(context),
            LootItemFunctionType::SetDamage(function) => function.validate(context),
            LootItemFunctionType::SetOminousBottleAmplifier(function) => function.validate(context),
            LootItemFunctionType::SetStewEffect(function) => function.validate(context),
            _ => {}
        }
    }
}

impl Validatable for LootItemFunction {
    fn validate(&self, context: &ValidationContext) {
        context.validate_context_usage(&self.function.get_referenced_context_params());
        validatable::validate_optional_holder(context, "condition", self.condition.as_ref());
        self.function.validate(context);
    }
}
