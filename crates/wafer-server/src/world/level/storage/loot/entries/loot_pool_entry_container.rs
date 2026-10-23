use wafer_protocol::nbt::Nbt;
use super::composite_entry_base::CompositeEntry;
use super::expandable_container_base::ExpandableEntry;
use super::single_entry_container_base::SingleEntry;
use crate::util::problem_reporter::Problem;
use crate::world::level::storage::loot::validatable::{self, Validatable};
use crate::world::level::storage::loot::validation_context::ValidationContext;

use super::composite_entry_base::CompositeEntryBase;
use super::expandable_container_base::ExpandableContainerBase;
use super::loot_pool_entry::LootPoolEntry;
use super::single_entry_container_base::SingleEntryContainerBase;
use crate::core::holder::Holder;
use crate::util::codec::{self, DataResult};
use crate::world::level::storage::loot::functions::loot_item_function::LootItemFunction;
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::predicates::loot_item_condition::LootItemCondition;

pub enum LootPoolEntryContainerType {
    Single(SingleEntryContainerBase),
    Expandable(ExpandableContainerBase),
    Composite(CompositeEntryBase),
}

pub struct LootPoolEntryContainer {
    pub(super) condition: Option<Holder<LootItemCondition>>,
    modifier: Option<Holder<LootItemFunction>>,
    entry: LootPoolEntryContainerType,
}

impl LootPoolEntryContainer {
    pub fn decode(tag: &Nbt, entry: LootPoolEntryContainerType) -> DataResult<LootPoolEntryContainer> {
        Ok(LootPoolEntryContainer {
            condition: codec::decode_optional(tag, "condition", LootItemCondition::decode_holder)?,
            modifier: codec::decode_optional(tag, "modifier", LootItemFunction::decode_holder)?,
            entry,
        })
    }

    fn expand_raw<'a, 'e>(
        &'e self,
        context: &mut LootContext<'a>,
        modifiers: &[&'e Holder<LootItemFunction>],
        output: &mut dyn FnMut(&mut LootContext<'a>, LootPoolEntry<'e>),
    ) -> bool {
        match &self.entry {
            LootPoolEntryContainerType::Single(entry) => entry.expand_raw(context, modifiers, output),
            LootPoolEntryContainerType::Expandable(entry) => entry.expand_raw(context, modifiers, output),
            LootPoolEntryContainerType::Composite(entry) => entry.expand_raw(context, modifiers, output),
        }
    }

    pub fn expand<'a, 'e>(
        &'e self,
        context: &mut LootContext<'a>,
        modifiers: &[&'e Holder<LootItemFunction>],
        output: &mut dyn FnMut(&mut LootContext<'a>, LootPoolEntry<'e>),
    ) -> bool {
        if !self.can_run(context) {
            return false;
        }
        match &self.modifier {
            Some(modifier) => {
                let adjusted: Vec<&'e Holder<LootItemFunction>> = std::iter::once(modifier).chain(modifiers.iter().copied()).collect();
                self.expand_raw(context, &adjusted, output)
            }
            None => self.expand_raw(context, modifiers, output),
        }
    }

    fn can_run(&self, context: &mut LootContext) -> bool {
        self.condition.as_ref().is_none_or(|condition| condition.value().test(context))
    }
}

impl Validatable for LootPoolEntryContainer {
    fn validate(&self, context: &ValidationContext) {
        validatable::validate_optional_holder(context, "condition", self.condition.as_ref());
        validatable::validate_optional_holder(context, "modifier", self.modifier.as_ref());
        match &self.entry {
            LootPoolEntryContainerType::Single(single) => {
                validatable::validate_optional_holder(context, "modifier", self.modifier.as_ref());
                if let SingleEntry::SlotLoot(slot_source) = &single.entry {
                    validatable::validate_holder(context, "slot_source", slot_source);
                }
            }
            LootPoolEntryContainerType::Expandable(expandable) => {
                validatable::validate_optional_holder(context, "modifier", self.modifier.as_ref());
                if let ExpandableEntry::NestedLootTable(value) = &expandable.entry {
                    validatable::validate_holder_set(context, "value", value);
                }
            }
            LootPoolEntryContainerType::Composite(composite) => {
                if composite.children.is_empty() {
                    context.report_problem(Problem::new("Empty children list"));
                }
                validatable::validate_list(context, "children", &composite.children);
                if let CompositeEntry::AlternativesEntry = composite.entry {
                    for child in composite.children.iter().take(composite.children.len().saturating_sub(1)) {
                        if child.condition.is_none() {
                            context.report_problem(Problem::new("Unreachable entry!"));
                        }
                    }
                }
            }
        }
    }
}
