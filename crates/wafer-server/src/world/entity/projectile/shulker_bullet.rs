use wafer_core::random::RandomSource;
use wafer_core::{Axis, BlockPos, Direction, Vec3};
use wafer_data::{damage_types, mob_effects};
use wafer_protocol::nbt::Nbt;

use super::projectile::{Projectile, ProjectileKind};
use super::projectile_deflection::ProjectileDeflection;
use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::difficulty::Difficulty;
use crate::world::effect::mob_effect_instance::MobEffectInstance;
use crate::world::entity::data::entity_defaults;
use crate::world::entity::entity_reference::EntityReference;
use crate::world::entity::{EntityDimensions, EntityId, sound_event};
use crate::world::level::block::BlockHitResult;
use crate::world::phys::entity_hit_result::EntityHitResult;
use crate::world::phys::hit_result::{HitResult, Type};

const ENTITY_TYPE: &str = "minecraft:shulker_bullet";
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.3125, 0.3125);
pub const GRAVITY: f64 = 0.04;

#[derive(Default)]
pub struct ShulkerBullet {
    final_target: Option<EntityReference>,
    current_move_direction: Option<Direction>,
    flight_steps: i32,
    target_delta_x: f64,
    target_delta_y: f64,
    target_delta_z: f64,
}

fn shulker_bullet(projectile: &Projectile) -> &ShulkerBullet {
    match &projectile.kind {
        ProjectileKind::ShulkerBullet(bullet) => bullet,
        _ => unreachable!("a shulker bullet"),
    }
}

fn shulker_bullet_mut(projectile: &mut Projectile) -> &mut ShulkerBullet {
    match &mut projectile.kind {
        ProjectileKind::ShulkerBullet(bullet) => bullet,
        _ => unreachable!("a shulker bullet"),
    }
}

impl ShulkerBullet {
    pub fn of_type() -> Projectile {
        let mut projectile = Projectile::new(ENTITY_TYPE, DIMENSIONS, entity_defaults(), ProjectileKind::ShulkerBullet(ShulkerBullet::default()));
        projectile.entity.no_physics = true;
        projectile
    }

    pub fn from_owner(level: &ServerLevel, owner: EntityId, target: EntityId, invalid_start_axis: Axis) -> Projectile {
        let mut bullet = ShulkerBullet::of_type();
        bullet.owner = Some(EntityReference::of(level.entity(owner).uuid));
        let position = level.entity(owner).bounding_box().center();
        let (y_rot, x_rot) = (bullet.entity.y_rot, bullet.entity.x_rot);
        bullet.entity.snap_to(position.x, position.y, position.z, y_rot, x_rot);
        let shulker_bullet = shulker_bullet_mut(&mut bullet);
        shulker_bullet.final_target = Some(EntityReference::of(level.entity(target).uuid));
        shulker_bullet.current_move_direction = Some(Direction::Up);
        let move_target = MoveTarget::of(level, bullet.entity.block_position(), Some(target));
        select_next_move_direction(&mut bullet, Some(invalid_start_axis), move_target);
        bullet
    }

    pub fn add_additional_save_data(projectile: &Projectile, output: &mut Vec<(String, Nbt)>) {
        let bullet = shulker_bullet(projectile);
        if let Some(final_target) = bullet.final_target {
            output.push(("Target".into(), crate::world::level::storage::uuid_ints(final_target.get_uuid())));
        }
        if let Some(direction) = bullet.current_move_direction {
            output.push(("Dir".into(), Nbt::Byte(direction as i8)));
        }
        output.push(("Steps".into(), Nbt::Int(bullet.flight_steps)));
        output.push(("TXD".into(), Nbt::Double(bullet.target_delta_x)));
        output.push(("TYD".into(), Nbt::Double(bullet.target_delta_y)));
        output.push(("TZD".into(), Nbt::Double(bullet.target_delta_z)));
    }

    pub fn read_additional_save_data(projectile: &mut Projectile, input: &Nbt) {
        let bullet = shulker_bullet_mut(projectile);
        bullet.flight_steps = input.get("Steps").and_then(crate::world::level::block::entity::int_value).unwrap_or(0);
        let double = |key: &str| input.get(key).and_then(Nbt::as_f64).unwrap_or(0.0);
        bullet.target_delta_x = double("TXD");
        bullet.target_delta_y = double("TYD");
        bullet.target_delta_z = double("TZD");
        bullet.current_move_direction = input.get("Dir").and_then(Nbt::as_i64).map(|id| Direction::from_3d_data_value(id as i32));
        bullet.final_target = EntityReference::read(input, "Target");
    }
}

struct MoveTarget {
    target: Option<(Vec3, f32)>,
    empty_neighbours: [bool; 6],
}

impl MoveTarget {
    fn of(level: &ServerLevel, current: BlockPos, target: Option<EntityId>) -> MoveTarget {
        let region = crate::world::level::path_navigation_region::PathNavigationRegion::new(level);
        let empty_neighbours = Direction::VALUES.map(|direction| region.get_block_state(current.relative(direction)).is_air());
        MoveTarget { target: target.map(|target| (level.entity(target).position(), level.entity(target).dimensions.height)), empty_neighbours }
    }
}

fn select_next_move_direction(projectile: &mut Projectile, avoid_axis: Option<Axis>, move_target: MoveTarget) {
    let mut y_offset = 0.5;
    let e = &projectile.entity;
    let target_pos = match move_target.target {
        None => e.block_position().below(),
        Some((position, height)) => {
            y_offset = height as f64 * 0.5;
            BlockPos::containing(position.x, position.y + y_offset, position.z)
        }
    };
    let mut target_x = target_pos.x as f64 + 0.5;
    let mut target_y = target_pos.y as f64 + y_offset;
    let mut target_z = target_pos.z as f64 + 0.5;
    let mut selection = None;
    if !target_pos.closer_to_center_than(e.position(), 2.0) {
        let current = e.block_position();
        let is_empty = |direction: Direction| move_target.empty_neighbours[direction as usize];
        let mut options = Vec::new();
        if avoid_axis != Some(Axis::X) {
            if current.x < target_pos.x && is_empty(Direction::East) {
                options.push(Direction::East);
            } else if current.x > target_pos.x && is_empty(Direction::West) {
                options.push(Direction::West);
            }
        }
        if avoid_axis != Some(Axis::Y) {
            if current.y < target_pos.y && is_empty(Direction::Up) {
                options.push(Direction::Up);
            } else if current.y > target_pos.y && is_empty(Direction::Down) {
                options.push(Direction::Down);
            }
        }
        if avoid_axis != Some(Axis::Z) {
            if current.z < target_pos.z && is_empty(Direction::South) {
                options.push(Direction::South);
            } else if current.z > target_pos.z && is_empty(Direction::North) {
                options.push(Direction::North);
            }
        }
        let random = &mut projectile.entity.random;
        let mut chosen = Direction::get_random(random);
        if options.is_empty() {
            let mut attempts = 5;
            while !is_empty(chosen) && attempts > 0 {
                chosen = Direction::get_random(random);
                attempts -= 1;
            }
        } else {
            chosen = options[random.next_int_bounded(options.len() as i32) as usize];
        }
        let e = &projectile.entity;
        target_x = e.x() + chosen.step_x() as f64;
        target_y = e.y() + chosen.step_y() as f64;
        target_z = e.z() + chosen.step_z() as f64;
        selection = Some(chosen);
    }
    let e = &projectile.entity;
    let xa = target_x - e.x();
    let ya = target_y - e.y();
    let za = target_z - e.z();
    let distance = (xa * xa + ya * ya + za * za).sqrt();
    let bullet = shulker_bullet_mut(projectile);
    bullet.current_move_direction = selection;
    if distance == 0.0 {
        bullet.target_delta_x = 0.0;
        bullet.target_delta_y = 0.0;
        bullet.target_delta_z = 0.0;
    } else {
        bullet.target_delta_x = xa / distance * 0.15;
        bullet.target_delta_y = ya / distance * 0.15;
        bullet.target_delta_z = za / distance * 0.15;
    }
    projectile.entity.needs_sync = true;
    let flight_steps = 10 + projectile.entity.random.next_int_bounded(5) * 10;
    shulker_bullet_mut(projectile).flight_steps = flight_steps;
}

impl ServerLevel {
    pub fn is_shulker_bullet(&self, id: EntityId) -> bool {
        self.as_projectile(id).is_some_and(|projectile| matches!(projectile.kind, ProjectileKind::ShulkerBullet(_)))
    }

    fn shulker_bullet_select_next_move_direction(&mut self, id: EntityId, avoid_axis: Option<Axis>, target: Option<EntityId>) {
        let move_target = MoveTarget::of(self, self.entity(id).block_position(), target);
        select_next_move_direction(self.projectile_mut(id), avoid_axis, move_target);
    }

    pub fn shulker_bullet_check_despawn(&mut self, id: EntityId) {
        if self.difficulty == Difficulty::Peaceful {
            self.discard(id);
        }
    }

    pub fn shulker_bullet_tick(&mut self, id: EntityId) {
        self.projectile_base_tick(id);
        if self.entity_ref(id).is_none() {
            return;
        }
        let final_target = shulker_bullet(self.projectile(id)).final_target.and_then(|target| target.get_entity(self));
        if final_target.is_none() {
            shulker_bullet_mut(self.projectile_mut(id)).final_target = None;
        }
        if final_target.is_none_or(|target| !self.is_alive(target) || self.is_player(target) && self.is_spectator(target)) {
            self.apply_gravity(id);
        } else {
            let bullet = shulker_bullet_mut(self.projectile_mut(id));
            bullet.target_delta_x = (bullet.target_delta_x * 1.025).clamp(-1.0, 1.0);
            bullet.target_delta_y = (bullet.target_delta_y * 1.025).clamp(-1.0, 1.0);
            bullet.target_delta_z = (bullet.target_delta_z * 1.025).clamp(-1.0, 1.0);
            let (tx, ty, tz) = (bullet.target_delta_x, bullet.target_delta_y, bullet.target_delta_z);
            let e = self.entity_mut(id);
            let movement = e.delta_movement;
            e.set_delta_movement(movement.add((tx - movement.x) * 0.2, (ty - movement.y) * 0.2, (tz - movement.z) * 0.2));
        }
        let hit_result = self.get_hit_result_on_move_vector(id, |level, entity| level.can_hit_entity(id, entity));
        let e = self.entity_mut(id);
        let movement = e.delta_movement;
        let new_position = e.position().add_vec(movement);
        e.set_pos(new_position);
        self.entity_moved(id);
        self.apply_effects_from_blocks(id);
        if self.entities.contains_key(&id) && self.entity(id).portal_process.is_some_and(|process| process.is_inside_portal_this_tick()) {
            self.handle_portal(id);
        }
        if self.entity_ref(id).is_none() {
            return;
        }
        if self.is_alive(id) && hit_result.get_type() != Type::Miss {
            self.hit_target_or_deflect_self(id, hit_result);
        }
        if self.entity_ref(id).is_none() {
            return;
        }
        self.rotate_towards_movement(id, 0.5);
        let Some(final_target) = final_target else { return };
        let bullet = shulker_bullet_mut(self.projectile_mut(id));
        if bullet.flight_steps > 0 {
            bullet.flight_steps -= 1;
            if bullet.flight_steps == 0 {
                let axis = bullet.current_move_direction.map(Direction::axis);
                self.shulker_bullet_select_next_move_direction(id, axis, Some(final_target));
            }
        }
        if let Some(current_move_direction) = shulker_bullet(self.projectile(id)).current_move_direction {
            let current = self.entity(id).block_position();
            let axis = current_move_direction.axis();
            if self.loaded_and_entity_can_stand_on_face(current.relative(current_move_direction), id, Direction::Up) {
                self.shulker_bullet_select_next_move_direction(id, Some(axis), Some(final_target));
            } else {
                let target_pos = self.entity(final_target).block_position();
                if axis == Axis::X && current.x == target_pos.x || axis == Axis::Z && current.z == target_pos.z || axis == Axis::Y && current.y == target_pos.y {
                    self.shulker_bullet_select_next_move_direction(id, Some(axis), Some(final_target));
                }
            }
        }
    }

    pub fn shulker_bullet_can_hit_entity(&self, id: EntityId, entity: EntityId) -> bool {
        self.projectile_base_can_hit_entity(id, entity) && !self.entity(entity).no_physics
    }

    pub fn shulker_bullet_on_hit_entity(&mut self, id: EntityId, hit_result: EntityHitResult) {
        let target = hit_result.entity;
        let owner = self.projectile_get_owner(id);
        let living_owner = owner.filter(|&owner| self.is_living(owner));
        let damage_source = DamageSource::source(damage_types::MOB_PROJECTILE, Some(id), living_owner);
        let was_hurt = self.hurt_server(target, damage_source.clone(), 4.0);
        if was_hurt {
            self.do_post_attack_effects(target, &damage_source);
            if self.is_living(target) {
                self.add_effect(target, MobEffectInstance::new(mob_effects::LEVITATION, 200, 0), Some(owner.unwrap_or(id)));
            }
        }
    }

    pub fn shulker_bullet_on_hit_block(&mut self, id: EntityId, hit_result: BlockHitResult) {
        self.projectile_base_on_hit_block(id, hit_result);
        let e = self.entity(id);
        let (x, y, z) = (e.x(), e.y(), e.z());
        self.send_particles("minecraft:explosion", |_| {}, x, y, z, 2, (0.2, 0.2, 0.2), 0.0);
        self.entity_play_sound(id, sound_event("minecraft:entity.shulker_bullet.hit"), 1.0, 1.0);
    }

    pub fn shulker_bullet_on_redirect_projectile(&mut self, id: EntityId, hit_projectile: EntityId) {
        let owner_reference = self.projectile(id).owner;
        self.deflect(hit_projectile, ProjectileDeflection::MomentumDeflect, Some(id), owner_reference, true, Vec3::new(1.0, 1.0, 1.0));
    }

    pub fn shulker_bullet_on_hit(&mut self, id: EntityId, hit_result: HitResult) {
        self.projectile_base_on_hit(id, hit_result);
        self.shulker_bullet_destroy(id);
    }

    pub fn shulker_bullet_hurt_server(&mut self, id: EntityId) -> bool {
        self.entity_play_sound(id, sound_event("minecraft:entity.shulker_bullet.hurt"), 1.0, 1.0);
        let e = self.entity(id);
        let (x, y, z) = (e.x(), e.y(), e.z());
        self.send_particles("minecraft:crit", |_| {}, x, y, z, 15, (0.2, 0.2, 0.2), 0.0);
        self.shulker_bullet_destroy(id);
        true
    }

    fn shulker_bullet_destroy(&mut self, id: EntityId) {
        self.discard(id);
        let position = self.entity(id).position();
        self.game_event(crate::world::level::gameevent::game_event::ENTITY_DAMAGE, position, crate::world::level::gameevent::game_event::Context::of(Some(id)));
    }
}

