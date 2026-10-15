use wafer_core::random::RandomSource;
use wafer_data::mob_effects;

use super::behavior::{Behavior, BehaviorState};
use super::behavior_utils;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::memory_status::MemoryStatus;
use crate::world::entity::npc::villager::villager_profession::ProfessionId;

const THROW_GIFT_AT_DISTANCE: f64 = 5.0;
const MIN_TIME_BETWEEN_GIFTS: i32 = 600;
const MAX_TIME_BETWEEN_GIFTS: i32 = 6600;
const TIME_TO_DELAY_FOR_HEAD_TO_FINISH_TURNING: i64 = 20;
const SPEED_MODIFIER: f32 = 0.5;

fn gift(profession: ProfessionId) -> Option<&'static str> {
    Some(match profession {
        ProfessionId::ARMORER => "minecraft:gameplay/hero_of_the_village/armorer_gift",
        ProfessionId::BUTCHER => "minecraft:gameplay/hero_of_the_village/butcher_gift",
        ProfessionId::CARTOGRAPHER => "minecraft:gameplay/hero_of_the_village/cartographer_gift",
        ProfessionId::CLERIC => "minecraft:gameplay/hero_of_the_village/cleric_gift",
        ProfessionId::FARMER => "minecraft:gameplay/hero_of_the_village/farmer_gift",
        ProfessionId::FISHERMAN => "minecraft:gameplay/hero_of_the_village/fisherman_gift",
        ProfessionId::FLETCHER => "minecraft:gameplay/hero_of_the_village/fletcher_gift",
        ProfessionId::LEATHERWORKER => "minecraft:gameplay/hero_of_the_village/leatherworker_gift",
        ProfessionId::LIBRARIAN => "minecraft:gameplay/hero_of_the_village/librarian_gift",
        ProfessionId::MASON => "minecraft:gameplay/hero_of_the_village/mason_gift",
        ProfessionId::SHEPHERD => "minecraft:gameplay/hero_of_the_village/shepherd_gift",
        ProfessionId::TOOLSMITH => "minecraft:gameplay/hero_of_the_village/toolsmith_gift",
        ProfessionId::WEAPONSMITH => "minecraft:gameplay/hero_of_the_village/weaponsmith_gift",
        _ => return None,
    })
}

pub struct GiveGiftToHero {
    behavior: BehaviorState,
    time_until_next_gift: i32,
    gift_given_during_this_run: bool,
    time_since_start: i64,
}

impl GiveGiftToHero {
    pub fn new(timeout: i32) -> GiveGiftToHero {
        GiveGiftToHero {
            behavior: BehaviorState::with_time_out(
                vec![
                    (memory_module_type::WALK_TARGET.erase(), MemoryStatus::Registered),
                    (memory_module_type::LOOK_TARGET.erase(), MemoryStatus::Registered),
                    (memory_module_type::INTERACTION_TARGET.erase(), MemoryStatus::Registered),
                    (memory_module_type::NEAREST_VISIBLE_PLAYER.erase(), MemoryStatus::ValuePresent),
                ],
                timeout,
            ),
            time_until_next_gift: MIN_TIME_BETWEEN_GIFTS,
            gift_given_during_this_run: false,
            time_since_start: 0,
        }
    }

    fn throw_gift(level: &mut ServerLevel, villager: EntityId, target: EntityId) {
        let loot_table = get_loot_table_to_throw(level, villager);
        let target_pos = level.entity(target).position();
        level.drop_from_gift_loot_table(villager, loot_table, |level, item_stack| behavior_utils::throw_item(level, villager, item_stack, target_pos));
    }

    fn is_hero_visible(level: &ServerLevel, body: EntityId) -> bool {
        GiveGiftToHero::get_nearest_targetable_hero(level, body).is_some()
    }

    fn get_nearest_targetable_hero(level: &ServerLevel, body: EntityId) -> Option<EntityId> {
        level.get_brain(body).get_memory(memory_module_type::NEAREST_VISIBLE_PLAYER).copied().filter(|&player| is_hero(level, player))
    }

    fn is_within_throwing_distance(level: &ServerLevel, villager: EntityId, player: EntityId) -> bool {
        let player_pos = level.entity(player).block_position();
        let villager_pos = level.entity(villager).block_position();
        villager_pos.closer_than(player_pos, THROW_GIFT_AT_DISTANCE)
    }
}

fn get_loot_table_to_throw(level: &ServerLevel, villager: EntityId) -> &'static str {
    if level.is_baby(villager) {
        return "minecraft:gameplay/hero_of_the_village/baby_gift";
    }
    gift(level.get_villager_data(villager).profession).unwrap_or("minecraft:gameplay/hero_of_the_village/unemployed_gift")
}

fn is_hero(level: &ServerLevel, player: EntityId) -> bool {
    level.living(player).has_effect(mob_effects::HERO_OF_THE_VILLAGE)
}

fn calculate_time_until_next_gift(level: &mut ServerLevel) -> i32 {
    MIN_TIME_BETWEEN_GIFTS + level.random.next_int_bounded(MAX_TIME_BETWEEN_GIFTS - MIN_TIME_BETWEEN_GIFTS + 1)
}

impl Behavior for GiveGiftToHero {
    fn behavior(&self) -> &BehaviorState {
        &self.behavior
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        &mut self.behavior
    }

    fn check_extra_start_conditions(&mut self, level: &mut ServerLevel, body: EntityId) -> bool {
        if !GiveGiftToHero::is_hero_visible(level, body) {
            false
        } else if self.time_until_next_gift > 0 {
            self.time_until_next_gift -= 1;
            false
        } else {
            true
        }
    }

    fn start(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) {
        self.gift_given_during_this_run = false;
        self.time_since_start = timestamp;
        let player = GiveGiftToHero::get_nearest_targetable_hero(level, body).expect("a hero");
        level.get_brain_mut(body).set_memory(memory_module_type::INTERACTION_TARGET, player);
        behavior_utils::look_at_entity(level, body, player);
    }

    fn can_still_use(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) -> bool {
        GiveGiftToHero::is_hero_visible(level, body) && !self.gift_given_during_this_run
    }

    fn tick(&mut self, level: &mut ServerLevel, villager: EntityId, timestamp: i64) {
        let player = GiveGiftToHero::get_nearest_targetable_hero(level, villager).expect("a hero");
        behavior_utils::look_at_entity(level, villager, player);
        if GiveGiftToHero::is_within_throwing_distance(level, villager, player) {
            if timestamp - self.time_since_start > TIME_TO_DELAY_FOR_HEAD_TO_FINISH_TURNING {
                GiveGiftToHero::throw_gift(level, villager, player);
                self.gift_given_during_this_run = true;
            }
        } else {
            behavior_utils::set_walk_and_look_target_memories_to_entity(level, villager, player, SPEED_MODIFIER, 5);
        }
    }

    fn stop(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        self.time_until_next_gift = calculate_time_until_next_gift(level);
        let brain = level.get_brain_mut(body);
        brain.erase_memory(memory_module_type::INTERACTION_TARGET);
        brain.erase_memory(memory_module_type::WALK_TARGET);
        brain.erase_memory(memory_module_type::LOOK_TARGET);
    }
}
