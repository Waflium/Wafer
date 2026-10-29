use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

use wafer_data::{GameRule, GameRuleType};
use wafer_protocol::nbt::Nbt;

use crate::world::flag::FeatureFlagSet;

#[derive(Clone)]
pub struct GameRules {
    values: Arc<[AtomicI32]>,
    dirty: Arc<AtomicBool>,
    enabled_features: FeatureFlagSet,
}

impl Default for GameRules {
    fn default() -> GameRules {
        GameRules::new(FeatureFlagSet::DEFAULT_FLAGS)
    }
}

pub fn required_features(rule: GameRule) -> FeatureFlagSet {
    if rule.info().experimental { FeatureFlagSet::MINECART_IMPROVEMENTS } else { FeatureFlagSet::default() }
}

impl GameRules {
    pub fn new(enabled_features: FeatureFlagSet) -> GameRules {
        GameRules { values: GameRule::all().map(|r| AtomicI32::new(r.info().default)).collect(), dirty: Arc::new(AtomicBool::new(true)), enabled_features }
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty.load(Ordering::Relaxed)
    }

    pub fn set_dirty(&self, dirty: bool) {
        self.dirty.store(dirty, Ordering::Relaxed);
    }

    pub fn has(&self, rule: GameRule) -> bool {
        required_features(rule).is_subset_of(self.enabled_features)
    }

    pub fn available(&self) -> impl Iterator<Item = GameRule> + '_ {
        GameRule::all().filter(|&r| self.has(r))
    }

    pub fn get_bool(&self, rule: GameRule) -> bool {
        debug_assert_eq!(rule.info().kind, GameRuleType::Bool);
        self.get(rule) != 0
    }

    pub fn get_int(&self, rule: GameRule) -> i32 {
        debug_assert_eq!(rule.info().kind, GameRuleType::Int);
        self.get(rule)
    }

    pub fn get(&self, rule: GameRule) -> i32 {
        self.values[rule.0 as usize].load(Ordering::Relaxed)
    }

    pub fn set(&self, rule: GameRule, value: i32) {
        self.set_dirty(true);
        self.values[rule.0 as usize].store(value, Ordering::Relaxed);
    }

    pub fn serialize(rule: GameRule, value: i32) -> String {
        match rule.info().kind {
            GameRuleType::Bool => (value != 0).to_string(),
            GameRuleType::Int => value.to_string(),
        }
    }

    pub fn to_nbt(&self) -> Nbt {
        Nbt::Compound(
            self.available()
                .map(|r| {
                    let value = self.get(r);
                    let tag = match r.info().kind {
                        GameRuleType::Bool => Nbt::Byte(value as i8),
                        GameRuleType::Int => Nbt::Int(value),
                    };
                    (format!("minecraft:{}", r.info().name), tag)
                })
                .collect(),
        )
    }

    pub fn load(&self, data: &Nbt) -> bool {
        let Nbt::Compound(entries) = data else { return true };
        let mut present = vec![false; self.values.len()];
        let mut removed = false;
        for (name, tag) in entries {
            let Some(rule) = GameRule::by_name(name) else { continue };
            if !self.has(rule) {
                removed = true;
                continue;
            }
            let info = rule.info();
            let value = match (info.kind, tag) {
                (GameRuleType::Bool, Nbt::Byte(b)) => (*b != 0) as i32,
                (GameRuleType::Int, Nbt::Int(i)) if (info.min..=info.max).contains(i) => *i,
                _ => continue,
            };
            self.values[rule.0 as usize].store(value, Ordering::Relaxed);
            present[rule.0 as usize] = true;
        }
        removed || self.available().any(|rule| !present[rule.0 as usize])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loading_reports_missing_rules_like_the_vanilla_constructor() {
        let rules = GameRules::default();
        assert!(rules.load(&Nbt::Compound(vec![("minecraft:keep_inventory".into(), Nbt::Byte(1))])));
        assert!(rules.get_bool(wafer_data::game_rule::KEEP_INVENTORY));
        assert!(rules.load(&Nbt::Compound(Vec::new())));
        assert!(!rules.load(&rules.to_nbt()));
    }
}
