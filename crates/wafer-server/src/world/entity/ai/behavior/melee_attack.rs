use wafer_data::EquipmentSlot;

use super::declarative::behavior_builder;
use super::entity_tracker::EntityTracker;
use super::one_shot::OneShot;
use super::position_tracker::PositionTracker;
use crate::server::level::ServerLevel;
use crate::world::InteractionHand;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::item::ItemStack;

pub fn create(cooldown_between_attacks: i32) -> OneShot {
    create_with(|_, _| true, cooldown_between_attacks)
}

pub fn create_with(can_attack_predicate: fn(&ServerLevel, EntityId) -> bool, cooldown_between_attacks: i32) -> OneShot {
    behavior_builder::create(|i| {
        let look_target = i.registered(memory_module_type::LOOK_TARGET);
        let attack_target = i.present(memory_module_type::ATTACK_TARGET);
        let attack_cooling_down = i.absent(memory_module_type::ATTACK_COOLING_DOWN);
        let nearest_entities = i.present(memory_module_type::NEAREST_VISIBLE_LIVING_ENTITIES);
        move |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            let target = attack_target.get(level, body);
            if can_attack_predicate(level, body)
                && !is_holding_usable_non_melee_weapon(level, body)
                && level.is_within_melee_attack_range(body, target)
                && nearest_entities.get(level, body).contains(level, target)
            {
                let tracker = EntityTracker::new(level, target, true);
                look_target.set(level, body, PositionTracker::Entity(tracker));
                level.swing_for_attack(body, InteractionHand::MainHand);
                level.do_hurt_target(body, target);
                attack_cooling_down.set_with_expiry(level, body, true, cooldown_between_attacks as i64);
                true
            } else {
                false
            }
        }
    })
}

fn is_holding_usable_non_melee_weapon(level: &ServerLevel, body: EntityId) -> bool {
    [EquipmentSlot::Mainhand, EquipmentSlot::Offhand].into_iter().any(|slot| {
        let item = level.item_by_slot(body, slot).unwrap_or_else(ItemStack::empty);
        level.can_use_non_melee_weapon(body, &item)
    })
}
