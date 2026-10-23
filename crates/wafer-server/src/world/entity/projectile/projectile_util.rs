use wafer_core::{AABB, Vec3, mth};
use wafer_data::{EquipmentSlot, Item, items};

use super::projectile::Projectile;
use crate::server::level::ServerLevel;
use crate::world::InteractionHand;
use crate::world::entity::EntityId;
use crate::world::item::{ItemStack, arrow_item};
use crate::util::either::Either;
use crate::world::entity::calculate_view_vector;
use crate::world::item::component::attack_range::AttackRange;
use crate::world::level::clip::{ClipBlock, ClipContext, ClipFluid, ClipResult};
use crate::world::phys::entity_hit_result::EntityHitResult;
use crate::world::phys::hit_result::{HitResult, Type};
use crate::world::phys::shapes::collision_context::CollisionContext;

pub fn compute_margin(tick_count: i32) -> f32 {
    0.0f32.max(0.3f32.min((tick_count - 2) as f32 / 20.0))
}

pub fn get_weapon_holding_hand(level: &ServerLevel, mob: EntityId, weapon_item: Item) -> InteractionHand {
    if level.item_by_slot_ref(mob, EquipmentSlot::Mainhand).is_some_and(|main_hand_item| main_hand_item.is(weapon_item)) { InteractionHand::MainHand } else { InteractionHand::OffHand }
}

pub fn get_mob_arrow(level: &mut ServerLevel, mob: EntityId, projectile: &ItemStack, power: f32, fired_from_weapon: Option<&ItemStack>) -> Projectile {
    let arrow_item = if projectile.get_item().is_instance_of("ArrowItem") { projectile.get_item() } else { items::ARROW };
    let mut arrow = arrow_item::create_arrow(level, arrow_item, projectile, mob, fired_from_weapon);
    arrow.set_base_damage_from_mob(power, level.difficulty as i32);
    arrow
}

impl ServerLevel {
    pub fn rotate_towards_movement(&mut self, projectile: EntityId, rotation_speed: f32) {
        let e = self.entity_mut(projectile);
        let movement = e.delta_movement;
        if movement.length_sqr() != 0.0 {
            let sd = movement.horizontal_distance();
            e.set_y_rot((mth::atan2(movement.z, movement.x) * mth::RAD_TO_DEG as f64) as f32 + 90.0);
            e.set_x_rot((mth::atan2(sd, movement.y) * mth::RAD_TO_DEG as f64) as f32 - 90.0);
            while e.x_rot - e.x_rot_o < -180.0 {
                e.x_rot_o -= 360.0;
            }
            while e.x_rot - e.x_rot_o >= 180.0 {
                e.x_rot_o += 360.0;
            }
            while e.y_rot - e.y_rot_o < -180.0 {
                e.y_rot_o -= 360.0;
            }
            while e.y_rot - e.y_rot_o >= 180.0 {
                e.y_rot_o += 360.0;
            }
            let x_rot = mth::lerp_f(rotation_speed, e.x_rot_o, e.x_rot);
            e.set_x_rot(x_rot);
            let y_rot = mth::lerp_f(rotation_speed, e.y_rot_o, e.y_rot);
            e.set_y_rot(y_rot);
        }
    }

    pub fn get_hit_entities_along(
        &mut self,
        attacker: EntityId,
        attack_range: &AttackRange,
        matching: impl Fn(&ServerLevel, EntityId) -> bool,
        block_clip_type: ClipBlock,
    ) -> Either<ClipResult, Vec<EntityHitResult>> {
        let e = self.entity(attacker);
        let look = calculate_view_vector(e.x_rot, e.y_head_rot);
        let eye_position = e.eye_position();
        let from = eye_position.add_vec(look.scale(attack_range.effective_min_range(self, attacker) as f64));
        let movement_component = self.known_movement(attacker).dot(look);
        let to = eye_position.add_vec(look.scale(attack_range.effective_max_range(self, attacker) as f64 + 0.0f64.max(movement_component)));
        self.get_hit_entities_along_between(attacker, eye_position, from, matching, to, attack_range.hitbox_margin, block_clip_type)
    }

    #[allow(clippy::too_many_arguments)]
    fn get_hit_entities_along_between(
        &mut self,
        source: EntityId,
        origin: Vec3,
        from: Vec3,
        matching: impl Fn(&ServerLevel, EntityId) -> bool,
        mut to: Vec3,
        entity_margin: f32,
        clip_type: ClipBlock,
    ) -> Either<ClipResult, Vec<EntityHitResult>> {
        let ctx = ClipContext { from: origin, to, block: clip_type, fluid: ClipFluid::None, collision_context: CollisionContext::of(self, source) };
        let hit_result = self.clip_including_border(&ctx);
        if !hit_result.miss {
            to = hit_result.hit.location;
            if origin.distance_to_sqr(to) < origin.distance_to_sqr(from) {
                return Either::Left(hit_result);
            }
        }
        let margin = entity_margin as f64;
        let search_area = AABB::of_size(from, margin, margin, margin).expand_towards_vec(to.subtract(from)).inflate_all(1.0);
        let entity_hit = self.get_many_entity_hit_result(source, from, to, &search_area, matching, entity_margin, clip_type, true, true);
        if !entity_hit.is_empty() { Either::Right(entity_hit) } else { Either::Left(hit_result) }
    }

    pub fn get_hit_result_on_view_vector(&mut self, source: EntityId, matching: impl Fn(&ServerLevel, EntityId) -> bool, distance: f64) -> HitResult {
        let e = self.entity(source);
        let y_rot = if self.is_living(source) { self.living(source).y_head_rot_o } else { e.y_rot_o };
        let view_vector = calculate_view_vector(e.x_rot_o, y_rot).scale(distance);
        let from = e.eye_position();
        self.get_hit_result(from, source, matching, view_vector, 0.0, ClipBlock::Collider)
    }

    pub fn get_hit_result_on_move_vector(&mut self, source: EntityId, matching: impl Fn(&ServerLevel, EntityId) -> bool) -> HitResult {
        self.get_hit_result_on_move_vector_with_clip(source, matching, ClipBlock::Collider)
    }

    pub fn get_hit_result_on_move_vector_with_clip(&mut self, source: EntityId, matching: impl Fn(&ServerLevel, EntityId) -> bool, clip_type: ClipBlock) -> HitResult {
        let e = self.entity(source);
        let (movement, from, margin) = (e.delta_movement, e.position(), compute_margin(e.tick_count));
        self.get_hit_result(from, source, matching, movement, margin, clip_type)
    }

    fn get_hit_result(&mut self, from: Vec3, source: EntityId, matching: impl Fn(&ServerLevel, EntityId) -> bool, delta: Vec3, entity_margin: f32, clip_type: ClipBlock) -> HitResult {
        let mut to = from.add_vec(delta);
        let ctx = ClipContext { from, to, block: clip_type, fluid: ClipFluid::None, collision_context: CollisionContext::of(self, source) };
        let clip = self.clip_including_border(&ctx);
        let mut hit_result = HitResult::Block { hit: clip.hit, miss: clip.miss };
        if hit_result.get_type() != Type::Miss {
            to = hit_result.get_location();
        }
        let area = self.entity(source).bounding_box().expand_towards_vec(delta).inflate_all(1.0);
        if let Some((entity, location)) = self.get_entity_hit_result(source, from, to, &area, matching, entity_margin) {
            hit_result = HitResult::Entity(EntityHitResult::new(entity, location));
        }
        hit_result
    }

    pub fn get_entity_hit_result(
        &self,
        source: EntityId,
        from: Vec3,
        to: Vec3,
        target_search_area: &AABB,
        matching: impl Fn(&ServerLevel, EntityId) -> bool,
        entity_margin: f32,
    ) -> Option<(EntityId, Vec3)> {
        let mut nearest = f64::MAX;
        let mut hit: Option<(EntityId, Vec3)> = None;
        for entity in self.get_entities(Some(source), target_search_area) {
            if !matching(self, entity) {
                continue;
            }
            let bb = self.entity(entity).bounding_box().inflate_all(entity_margin as f64);
            if let Some(location) = bb.clip(from, to) {
                let dd = from.distance_to_sqr(location);
                if dd < nearest {
                    hit = Some((entity, location));
                    nearest = dd;
                }
            }
        }
        hit
    }

    #[allow(clippy::too_many_arguments)]
    pub fn get_many_entity_hit_result(
        &mut self,
        source: EntityId,
        from: Vec3,
        to: Vec3,
        target_search_area: &AABB,
        matching: impl Fn(&ServerLevel, EntityId) -> bool,
        entity_margin: f32,
        clip_type: ClipBlock,
        include_from_entity: bool,
        project_surface_hit_location: bool,
    ) -> Vec<EntityHitResult> {
        let mut collector = Vec::new();
        for entity in self.get_entities(Some(source), target_search_area) {
            if !matching(self, entity) {
                continue;
            }
            let entity_bb = self.entity(entity).bounding_box();
            if include_from_entity && entity_bb.contains_vec(from) {
                collector.push(EntityHitResult::new(entity, from));
                continue;
            }
            if let Some(exact_hit) = entity_bb.clip(from, to) {
                collector.push(EntityHitResult::new(entity, exact_hit));
            } else if !(entity_margin <= 0.0)
                && let Some(outside_hit_position) = entity_bb.inflate_all(entity_margin as f64).clip(from, to)
            {
                let mut towards_target = entity_bb.center();
                let ctx = ClipContext { from: outside_hit_position, to: towards_target, block: clip_type, fluid: ClipFluid::None, collision_context: CollisionContext::of(self, source) };
                let clip = self.clip_including_border(&ctx);
                if !clip.miss {
                    towards_target = clip.hit.location;
                }
                if let Some(surface_hit) = self.entity(entity).bounding_box().clip(outside_hit_position, towards_target) {
                    collector.push(EntityHitResult::new(entity, if project_surface_hit_location { surface_hit } else { outside_hit_position }));
                }
            }
        }
        collector
    }
}
