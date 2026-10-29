use wafer_protocol::nbt::Nbt;

use super::criterion_trigger::CriterionTrigger;
use crate::advancements::criterion_trigger_instance::CriterionTriggerInstance;
use crate::advancements::predicates::entity::entity_predicate::EntityPredicate;
use crate::core::holder::Holder;
use crate::server::level::ServerLevel;
use crate::server::player_advancements::{PlayerAdvancements, TriggerInstanceKey};
use crate::util::codec::{self, DataResult, DecodeResult, Record};
use crate::world::entity::EntityId;
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::predicates::loot_item_condition::LootItemCondition;
use crate::world::level::storage::loot::validation_context_source::ValidationContextSource;
use crate::world::level::storage::loot::validatable;

pub fn trigger(level: &mut ServerLevel, player: EntityId, trigger: CriterionTrigger, mut matcher: impl FnMut(&mut ServerLevel, &'static CriterionTriggerInstance) -> bool) {
    let Some(listeners_for_type) = level.player(player).advancements.get_trigger_map_for_type(trigger) else { return };
    if listeners_for_type.is_empty() {
        return;
    }
    let listeners: Vec<_> = listeners_for_type.iter().collect();
    let mut matched_conditions: Vec<TriggerInstanceKey> = Vec::new();
    for listener in listeners {
        if matcher(level, listener.instance) {
            let player_matches = match listener.instance.player() {
                None => true,
                Some(predicate) => predicate.value().test(&mut EntityPredicate::create_context(level, player, player)),
            };
            if player_matches {
                matched_conditions.push(listener.key);
            }
        }
    }
    award_matched_from(level, player, matched_conditions, 0);
}

fn award_matched_from(level: &mut ServerLevel, player: EntityId, matched_conditions: Vec<TriggerInstanceKey>, first: usize) {
    for index in first..matched_conditions.len() {
        let criterion = matched_conditions[index];
        PlayerAdvancements::award(level, player, criterion.advancement, criterion.criterion);
        if level.resumable_triggers {
            level.yield_for_functions();
            if level.tick_suspended() && index + 1 < matched_conditions.len() {
                level.suspend_tick(move |level| {
                    level.resumable_triggers = true;
                    award_matched_from(level, player, matched_conditions, index + 1);
                    level.resumable_triggers = false;
                });
                return;
            }
        }
    }
}

pub fn decode<T>(tag: &Nbt, read: impl FnOnce(&mut Fields) -> Option<T>) -> DecodeResult<T> {
    let mut fields = match Fields::new(tag) {
        Ok(fields) => fields,
        Err(error) => return DecodeResult::error(error),
    };
    let value = read(&mut fields);
    fields.build(|| value)
}

pub struct Fields<'a> {
    tag: &'a Nbt,
    record: Record,
}

impl<'a> Fields<'a> {
    pub fn new(tag: &'a Nbt) -> Result<Fields<'a>, String> {
        codec::map(tag)?;
        Ok(Fields { tag, record: Record::default() })
    }

    pub fn player(&mut self) -> Option<Holder<LootItemCondition>> {
        self.condition("player")
    }

    pub fn condition(&mut self, name: &str) -> Option<Holder<LootItemCondition>> {
        self.record.optional(self.tag, name, LootItemCondition::decode_holder_partial)
    }

    pub fn conditions(&mut self, name: &str) -> Vec<Holder<LootItemCondition>> {
        self.record.with_default(self.tag, name, Vec::new(), |list| codec::list_partial(list, LootItemCondition::decode_holder_partial))
    }

    pub fn optional<T>(&mut self, name: &str, decode: impl FnOnce(&Nbt) -> DataResult<T>) -> Option<T> {
        self.record.optional(self.tag, name, decode)
    }

    pub fn with_default<T>(&mut self, name: &str, default: T, decode: impl FnOnce(&Nbt) -> DataResult<T>) -> T {
        self.record.with_default(self.tag, name, default, decode)
    }

    pub fn list<T>(&mut self, name: &str, decode: impl Fn(&Nbt) -> DataResult<T>) -> Vec<T> {
        self.record.with_default(self.tag, name, Vec::new(), |list| codec::list_partial(list, |element| DecodeResult::from(decode(element))))
    }

    pub fn required<T, R: Into<DecodeResult<T>>>(&mut self, name: &str, decode: impl FnOnce(&Nbt) -> R) -> Option<T> {
        self.record.required(self.tag, name, decode)
    }

    pub fn build<T>(self, build: impl FnOnce() -> Option<T>) -> DecodeResult<T> {
        self.record.build(build)
    }
}

pub fn matches(predicate: Option<&Holder<LootItemCondition>>, context: &mut LootContext) -> bool {
    predicate.is_none_or(|predicate| predicate.value().test(context))
}

pub fn matches_entity(level: &mut ServerLevel, player: EntityId, predicate: Option<&Holder<LootItemCondition>>, entity: EntityId) -> bool {
    predicate.is_none_or(|predicate| level.entity_ref(entity).is_some() && predicate.value().test(&mut EntityPredicate::create_context(level, player, entity)))
}

pub fn validate(validator: &mut ValidationContextSource, player: Option<&Holder<LootItemCondition>>) {
    validatable::validate_optional_holder(validator.entity_context(), "player", player);
}
