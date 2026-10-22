use wafer_core::Vec3;
use wafer_protocol::nbt::Nbt;

use crate::server::level::ServerLevel;
use crate::world::entity::data::DataValue;
use crate::world::entity::entity_reference::EntityReference;
use crate::world::entity::projectile::projectile::{Projectile, ProjectileKind};
use crate::world::entity::{EntityDimensions, EntityId};
use crate::world::level::clip::ClipBlock;
use crate::world::phys::hit_result::Type;

pub const INITAL_ACCELERATION_POWER: f64 = 0.1;
pub const DEFLECTION_SCALE: f64 = 0.5;

pub struct AbstractHurtingProjectile {
    pub acceleration_power: f64,
}

impl Default for AbstractHurtingProjectile {
    fn default() -> AbstractHurtingProjectile {
        AbstractHurtingProjectile { acceleration_power: INITAL_ACCELERATION_POWER }
    }
}

pub fn at(entity_type: &str, dimensions: EntityDimensions, data: Vec<DataValue>, kind: ProjectileKind, x: f64, y: f64, z: f64, direction: Vec3) -> Projectile {
    let mut projectile = Projectile::new(entity_type, dimensions, data, kind);
    let e = &mut projectile.entity;
    let (y_rot, x_rot) = (e.y_rot, e.x_rot);
    e.snap_to(x, y, z, y_rot, x_rot);
    e.reapply_position();
    let acceleration_power = projectile.abstract_hurting_projectile().expect("a hurting projectile").acceleration_power;
    projectile.assign_directional_movement(direction, acceleration_power);
    projectile
}

pub fn from_mob(level: &ServerLevel, entity_type: &str, dimensions: EntityDimensions, data: Vec<DataValue>, kind: ProjectileKind, mob: EntityId, direction: Vec3) -> Projectile {
    let m = level.entity(mob);
    let mut projectile = at(entity_type, dimensions, data, kind, m.x(), m.y(), m.z(), direction);
    projectile.owner = Some(EntityReference::of(m.uuid));
    projectile.entity.set_rot(m.y_rot, m.x_rot);
    projectile
}

impl Projectile {
    fn assign_directional_movement(&mut self, direction: Vec3, speed: f64) {
        self.entity.set_delta_movement(direction.normalize().scale(speed));
        self.entity.needs_sync = true;
    }

    pub fn abstract_hurting_projectile_add_additional_save_data(&self, output: &mut Vec<(String, Nbt)>) {
        let acceleration_power = self.abstract_hurting_projectile().expect("a hurting projectile").acceleration_power;
        output.push(("acceleration_power".into(), Nbt::Double(acceleration_power)));
    }

    pub fn abstract_hurting_projectile_read_additional_save_data(&mut self, input: &Nbt) {
        let acceleration_power = match input.get("acceleration_power") {
            Some(Nbt::Double(value)) => *value,
            Some(value) => value.as_f64().unwrap_or(INITAL_ACCELERATION_POWER),
            None => INITAL_ACCELERATION_POWER,
        };
        self.abstract_hurting_projectile_mut().expect("a hurting projectile").acceleration_power = acceleration_power;
    }
}

impl ServerLevel {
    pub fn abstract_hurting_projectile_tick(&mut self, id: EntityId) {
        self.abstract_hurting_projectile_apply_inertia(id);
        let pos = self.entity(id).block_position();
        if !self.has_chunk_at(pos) {
            self.discard(id);
            return;
        }
        let clip_type = self.abstract_hurting_projectile_get_clip_type(id);
        let hit_result = self.get_hit_result_on_move_vector_with_clip(id, |level, entity| level.can_hit_entity(id, entity), clip_type);
        let new_position = if hit_result.get_type() != Type::Miss {
            hit_result.get_location()
        } else {
            let e = self.entity(id);
            e.position().add_vec(e.delta_movement)
        };
        self.rotate_towards_movement(id, 0.2);
        self.entity_mut(id).set_pos(new_position);
        self.entity_moved(id);
        self.apply_effects_from_blocks(id);
        self.projectile_base_tick(id);
        if self.entity_ref(id).is_none() {
            return;
        }
        if self.should_burn(id) {
            self.ignite_for_seconds(id, 1.0);
        }
        if hit_result.get_type() != Type::Miss && self.is_alive(id) {
            self.hit_target_or_deflect_self(id, hit_result);
        }
    }

    fn abstract_hurting_projectile_get_clip_type(&self, _id: EntityId) -> ClipBlock {
        ClipBlock::Collider
    }

    fn abstract_hurting_projectile_apply_inertia(&mut self, id: EntityId) {
        let inertia = if self.entity(id).is_in_water() { self.get_liquid_inertia(id) } else { self.get_inertia(id) };
        let acceleration_power = self.projectile(id).abstract_hurting_projectile().expect("a hurting projectile").acceleration_power;
        let e = self.entity_mut(id);
        let movement = e.delta_movement;
        e.set_delta_movement(movement.add_vec(movement.normalize().scale(acceleration_power)).scale(inertia as f64));
    }

    pub fn abstract_hurting_projectile_can_hit_entity(&self, id: EntityId, entity: EntityId) -> bool {
        self.projectile_base_can_hit_entity(id, entity) && !self.entity(entity).no_physics
    }

    fn should_burn(&self, id: EntityId) -> bool {
        !self.projectile(id).is_abstract_wind_charge() && !self.is_wither_skull(id) && !self.is_dragon_fireball(id)
    }

    fn get_inertia(&self, id: EntityId) -> f32 {
        if self.is_wither_skull(id) {
            return super::wither_skull::get_inertia(self.entity(id));
        }
        if self.projectile(id).is_abstract_wind_charge() { 1.0 } else { 0.95 }
    }

    fn get_liquid_inertia(&self, id: EntityId) -> f32 {
        if self.projectile(id).is_abstract_wind_charge() { self.get_inertia(id) } else { 0.8 }
    }

    pub fn abstract_hurting_projectile_on_deflection(&mut self, id: EntityId, by_attack: bool) {
        let projectile = self.projectile_mut(id).abstract_hurting_projectile_mut().expect("a hurting projectile");
        if by_attack {
            projectile.acceleration_power = INITAL_ACCELERATION_POWER;
        } else {
            projectile.acceleration_power *= DEFLECTION_SCALE;
        }
    }
}
