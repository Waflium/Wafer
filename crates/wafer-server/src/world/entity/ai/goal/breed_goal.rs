use super::goal::reduced_tick_delay;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::targeting::targeting_conditions::TargetingConditions;

const PARTNER_TARGETING: TargetingConditions<'static> = TargetingConditions::for_non_combat().range(8.0).ignore_line_of_sight();

pub type PartnerClass = fn(&ServerLevel, EntityId) -> bool;

pub struct BreedGoal {
    partner_class: Option<PartnerClass>,
    pub partner: Option<EntityId>,
    love_time: i32,
    speed_modifier: f64,
}

impl BreedGoal {
    pub fn new(speed_modifier: f64) -> BreedGoal {
        BreedGoal { partner_class: None, partner: None, love_time: 0, speed_modifier }
    }

    pub fn with_partner_class(speed_modifier: f64, partner_class: PartnerClass) -> BreedGoal {
        BreedGoal { partner_class: Some(partner_class), ..BreedGoal::new(speed_modifier) }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        if !level.is_in_love(mob) {
            return false;
        }
        self.partner = self.get_free_partner(level, mob);
        self.partner.is_some()
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, _mob: EntityId) -> bool {
        let partner = self.partner.expect("a partner");
        level.is_alive(partner) && level.is_in_love(partner) && self.love_time < 60 && !level.is_panicking(partner)
    }

    pub fn stop(&mut self) {
        self.partner = None;
        self.love_time = 0;
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.tick_with(level, mob, BreedGoal::breed);
    }

    pub fn tick_with(&mut self, level: &mut ServerLevel, mob: EntityId, breed: fn(&BreedGoal, &mut ServerLevel, EntityId, EntityId)) {
        let partner = self.partner.expect("a partner");
        let max_head_x_rot = level.mob(mob).expect("a mob").get_max_head_x_rot() as f32;
        level.set_look_at_entity(mob, partner, 10.0, max_head_x_rot);
        let speed_modifier = self.speed_modifier;
        level.get_navigation(mob, |navigation, level| navigation.move_to_entity(level, mob, partner, speed_modifier));
        self.love_time += 1;
        if self.love_time >= reduced_tick_delay(60) && level.entity(mob).distance_to_sqr(level.entity(partner)) < 9.0 {
            breed(self, level, mob, partner);
        }
    }

    fn get_free_partner(&self, level: &mut ServerLevel, mob: EntityId) -> Option<EntityId> {
        let area = level.entity(mob).bounding_box().inflate_all(8.0);
        let animals = match self.partner_class {
            Some(partner_class) => {
                let mut nearby = level.get_entities(None, &area);
                nearby.retain(|&entity| partner_class(level, entity));
                nearby.retain(|&entity| PARTNER_TARGETING.test(level, Some(mob), entity));
                nearby
            }
            None => level.get_nearby_entities_same_kind(&PARTNER_TARGETING, mob, &area),
        };
        let mut dist = f64::MAX;
        let mut partner = None;
        for potential_partner in animals {
            if level.can_mate(mob, potential_partner) && !level.is_panicking(potential_partner) && level.entity(mob).distance_to_sqr(level.entity(potential_partner)) < dist {
                partner = Some(potential_partner);
                dist = level.entity(mob).distance_to_sqr(level.entity(potential_partner));
            }
        }
        partner
    }

    fn breed(&self, level: &mut ServerLevel, mob: EntityId, partner: EntityId) {
        level.spawn_child_from_breeding(mob, partner);
    }
}
