use wafer_data::game_rule;
use wafer_protocol::nbt::Nbt;

use super::{Entity, EntityId};
use super::entity_reference::EntityReference;
use super::mob::{Mob, MobKind};
use crate::server::level::ServerLevel;
use crate::world::difficulty::Difficulty;
use crate::world::level::block::entity::int_value;

pub const TAG_ANGER_END_TIME: &str = "anger_end_time";
pub const TAG_ANGRY_AT: &str = "angry_at";
pub const NO_ANGER_END_TIME: i64 = -1;

pub trait NeutralMob {
    fn get_persistent_anger_end_time(&self, entity: &Entity) -> i64;

    fn set_persistent_anger_end_time(&mut self, entity: &mut Entity, end_time: i64);

    fn get_persistent_anger_target(&self) -> Option<EntityReference>;

    fn set_persistent_anger_target(&mut self, persistent_anger_target: Option<EntityReference>);

    fn add_persistent_anger_save_data(&self, entity: &Entity, output: &mut Vec<(String, Nbt)>) {
        output.push((TAG_ANGER_END_TIME.into(), Nbt::Long(self.get_persistent_anger_end_time(entity))));
        EntityReference::store(self.get_persistent_anger_target(), output, TAG_ANGRY_AT);
    }
}

fn neutral_mob(mob: &Mob) -> &dyn NeutralMob {
    mob.neutral_mob().expect("a neutral mob")
}

fn neutral_mob_mut(mob: &mut Mob) -> (&mut dyn NeutralMob, &mut Entity) {
    mob.neutral_mob_mut().expect("a neutral mob")
}

impl ServerLevel {
    pub fn get_persistent_anger_end_time(&self, id: EntityId) -> i64 {
        let mob = self.mob(id).expect("a mob");
        neutral_mob(mob).get_persistent_anger_end_time(&mob.entity)
    }

    pub fn set_persistent_anger_end_time(&mut self, id: EntityId, end_time: i64) {
        let (neutral_mob, entity) = neutral_mob_mut(self.mob_mut(id).expect("a mob"));
        neutral_mob.set_persistent_anger_end_time(entity, end_time);
    }

    pub fn get_persistent_anger_target(&self, id: EntityId) -> Option<EntityReference> {
        neutral_mob(self.mob(id).expect("a mob")).get_persistent_anger_target()
    }

    pub fn set_persistent_anger_target(&mut self, id: EntityId, persistent_anger_target: Option<EntityReference>) {
        neutral_mob_mut(self.mob_mut(id).expect("a mob")).0.set_persistent_anger_target(persistent_anger_target);
    }

    pub fn start_persistent_anger_timer(&mut self, id: EntityId) {
        match self.mob(id).map(|mob| &mob.kind) {
            Some(MobKind::Enderman(_)) => self.enderman_start_persistent_anger_timer(id),
            Some(MobKind::Wolf(_)) => self.wolf_start_persistent_anger_timer(id),
            Some(MobKind::IronGolem(_)) => self.iron_golem_start_persistent_anger_timer(id),
            Some(MobKind::ZombifiedPiglin(_)) => self.zombified_piglin_start_persistent_anger_timer(id),
            _ => unreachable!("not a neutral mob"),
        }
    }

    pub fn set_time_to_remain_angry(&mut self, id: EntityId, remaining_time: i64) {
        let end_time = self.game_time + remaining_time;
        self.set_persistent_anger_end_time(id, end_time);
    }

    pub fn read_persistent_anger_save_data(&mut self, id: EntityId, input: &Nbt) {
        if let Some(end_time) = input.get(TAG_ANGER_END_TIME).and_then(Nbt::as_i64) {
            self.set_persistent_anger_end_time(id, end_time);
        } else if let Some(anger_time) = input.get("AngerTime").and_then(int_value) {
            self.set_time_to_remain_angry(id, anger_time as i64);
        } else {
            self.set_persistent_anger_end_time(id, NO_ANGER_END_TIME);
        }
        self.set_persistent_anger_target(id, EntityReference::read(input, TAG_ANGRY_AT));
        let target = EntityReference::get_living_entity(self.get_persistent_anger_target(id), self);
        self.set_target(id, target);
    }

    pub fn update_persistent_anger(&mut self, id: EntityId, stay_angry_if_target_present: bool) {
        let previous_target = self.mob(id).expect("a mob").target.filter(|&target| self.entity_ref(target).is_some());
        let persistent_anger_target = self.get_persistent_anger_target(id);
        if let (Some(previous_target), Some(persistent_anger_target)) = (previous_target, persistent_anger_target)
            && self.is_dead_or_dying(previous_target)
            && persistent_anger_target.matches(self, previous_target)
            && self.is_mob(previous_target)
        {
            self.stop_being_angry(id);
            return;
        }
        let target = self.get_target(id);
        if let Some(target) = target {
            let new_target = persistent_anger_target.is_none_or(|persistent_anger_target| !persistent_anger_target.matches(self, target));
            if new_target {
                let reference = EntityReference::of(self.entity(target).uuid);
                self.set_persistent_anger_target(id, Some(reference));
            }
            if new_target || stay_angry_if_target_present {
                self.start_persistent_anger_timer(id);
            }
        }
        if persistent_anger_target.is_some() && !self.is_angry(id) && (target.is_none_or(|target| !self.is_valid_player_target(target)) || !stay_angry_if_target_present) {
            self.stop_being_angry(id);
        }
        if let Some(player) = EntityReference::get_living_entity(persistent_anger_target, self).filter(|&entity| self.is_player(entity))
            && (self.player(player).game_mode.game_type.is_creative() || self.is_spectator(player) || self.difficulty == Difficulty::Peaceful)
        {
            self.stop_being_angry(id);
        }
    }

    fn is_valid_player_target(&self, target: EntityId) -> bool {
        self.is_player(target) && !self.player(target).game_mode.game_type.is_creative() && !self.is_spectator(target) && self.difficulty != Difficulty::Peaceful
    }

    pub fn is_angry_at(&self, id: EntityId, entity: EntityId) -> bool {
        if !self.can_attack(id, entity) {
            return false;
        }
        if self.is_valid_player_target(entity) && self.is_angry_at_all_players(id) {
            return true;
        }
        self.get_persistent_anger_target(id).is_some_and(|persistent_anger_target| persistent_anger_target.matches(self, entity))
    }

    pub fn is_angry_at_all_players(&self, id: EntityId) -> bool {
        self.game_rules.get_bool(game_rule::UNIVERSAL_ANGER) && self.is_angry(id) && self.get_persistent_anger_target(id).is_none()
    }

    pub fn is_angry(&self, id: EntityId) -> bool {
        let end_time = self.get_persistent_anger_end_time(id);
        if end_time > 0 {
            let remaining = end_time - self.game_time;
            remaining > 0
        } else {
            false
        }
    }

    pub fn player_died(&mut self, id: EntityId, player: EntityId) {
        if self.game_rules.get_bool(game_rule::FORGIVE_DEAD_PLAYERS)
            && self.get_persistent_anger_target(id).is_some_and(|persistent_anger_target| persistent_anger_target.matches(self, player))
        {
            self.stop_being_angry(id);
        }
    }

    pub fn forget_current_target_and_refresh_universal_anger(&mut self, id: EntityId) {
        self.stop_being_angry(id);
        self.start_persistent_anger_timer(id);
    }

    pub fn stop_being_angry(&mut self, id: EntityId) {
        self.set_last_hurt_by_mob(id, None);
        self.set_persistent_anger_target(id, None);
        self.set_target(id, None);
        self.set_persistent_anger_end_time(id, NO_ANGER_END_TIME);
    }
}
