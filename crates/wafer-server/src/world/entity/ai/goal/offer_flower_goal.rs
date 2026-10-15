use wafer_core::AABB;
use wafer_core::random::RandomSource;
use wafer_data::{EquipmentSlot, entity_type_tags, items};

use super::goal::reduced_tick_delay;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::targeting::targeting_conditions::TargetingConditions;
use crate::world::item::ItemStack;

const OFFER_TARGET_CONTEXT: TargetingConditions<'static> = TargetingConditions::for_non_combat().range(6.0);
pub const OFFER_TICKS: i32 = 400;
const EQUIPMENT_SLOT_ANTENNA: EquipmentSlot = EquipmentSlot::Saddle;

#[derive(Default)]
pub struct OfferFlowerGoal {
    entity: Option<EntityId>,
    tick: i32,
}

fn get_golem_bounding_box(level: &ServerLevel, golem: EntityId) -> AABB {
    level.entity(golem).bounding_box().inflate(6.0, 2.0, 6.0)
}

impl OfferFlowerGoal {
    pub fn can_use(&mut self, level: &mut ServerLevel, golem: EntityId) -> bool {
        if !level.is_bright_outside() {
            return false;
        }
        if level.entity_mut(golem).random.next_int_bounded(8000) != 0 {
            return false;
        }
        let bb = get_golem_bounding_box(level, golem);
        let candidates: Vec<EntityId> = level
            .get_entities(None, &bb)
            .into_iter()
            .filter(|&entity| level.is_living(entity) && entity_type_tags::CANDIDATE_FOR_IRON_GOLEM_GIFT.contains(level.entity(entity).entity_type as usize))
            .collect();
        let e = level.entity(golem);
        let (x, y, z) = (e.x(), e.y(), e.z());
        self.entity = level.get_nearest_entity(&candidates, &OFFER_TARGET_CONTEXT, Some(golem), x, y, z);
        self.entity.is_some()
    }

    pub fn can_continue_to_use(&self) -> bool {
        self.tick > 0
    }

    pub fn start(&mut self, level: &mut ServerLevel, golem: EntityId) {
        self.tick = reduced_tick_delay(OFFER_TICKS);
        level.offer_flower(golem, true);
    }

    pub fn stop(&mut self, level: &mut ServerLevel, golem: EntityId) {
        level.offer_flower(golem, false);
        if self.tick == 0
            && let Some(mob) = self.entity.filter(|&entity| level.is_mob(entity))
            && entity_type_tags::ACCEPTS_IRON_GOLEM_GIFT.contains(level.entity(mob).entity_type as usize)
            && level.item_by_slot(mob, EQUIPMENT_SLOT_ANTENNA).is_none()
            && get_golem_bounding_box(level, golem).intersects(&level.entity(mob).bounding_box())
        {
            level.living_set_item_slot(mob, EQUIPMENT_SLOT_ANTENNA, ItemStack::new(items::POPPY, 1));
            level.set_guaranteed_drop(mob, EQUIPMENT_SLOT_ANTENNA);
        }
        self.entity = None;
    }

    pub fn tick(&mut self, level: &mut ServerLevel, golem: EntityId) {
        if let Some(entity) = self.entity.filter(|&entity| level.entity_ref(entity).is_some()) {
            level.set_look_at_entity(golem, entity, 30.0, 30.0);
        }
        self.tick -= 1;
    }
}
