use wafer_core::random::RandomSource;
use wafer_core::{AABB, BlockPos, Vec3, mth};
use wafer_data::{block_tags, damage_type_tags, damage_types, game_rule, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use super::dragon_flight_history::DragonFlightHistory;
use super::ender_dragon_part::EnderDragonPart;
use super::phases::ender_dragon_phase::EnderDragonPhase;
use super::phases::ender_dragon_phase_manager::{EnderDragonPhaseManager, switch_phase};
use crate::server::level::ServerLevel;
use crate::server::level::events;
use crate::world::damagesource::DamageSource;
use crate::world::entity::ai::targeting::targeting_conditions::TargetingConditions;
use crate::world::entity::attributes::ENDER_DRAGON_ATTRIBUTES;
use crate::world::entity::data::{DataValue, mob_defaults};
use crate::world::entity::entity_selector;
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::{Entity, EntityDimensions, EntityId, LevelEntity, MoverType, RemovalReason, calculate_view_vector};
use crate::world::level::BlockGetter;
use crate::world::level::chunk::HeightmapType;
use crate::world::level::dimension::end::ender_dragon_fight;
use crate::world::level::gameevent::game_event;
use crate::world::level::pathfinder::binary_heap::BinaryHeap;
use crate::world::level::pathfinder::node::Node;
use crate::world::level::pathfinder::path::Path;
use crate::world::level::server_level_accessor::ServerLevelAccessor;

pub const DATA_PHASE: usize = 16;
const CRYSTAL_DESTROY_TARGETING: TargetingConditions<'static> = TargetingConditions::for_combat().range(64.0);
const SITTING_ALLOWED_DAMAGE_PERCENTAGE: f32 = 0.25;
const DEFAULT_DEATH_TIME: i32 = 0;
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(16.0, 8.0);
const HEAD: usize = 0;
const NECK: usize = 1;
const BODY: usize = 2;
const TAIL1: usize = 3;
const WING1: usize = 6;
const WING2: usize = 7;
const NODE_ADJACENCY: [i32; 24] = [
    6146, 8197, 8202, 16404, 32808, 32848, 65696, 131392, 131712, 263424, 526848, 525313, 1581057, 3166214, 2138120, 6373424, 4358208, 12910976, 9044480, 9706496, 15216640,
    13688832, 11763712, 8257536,
];

pub struct EnderDragon {
    pub flight_history: DragonFlightHistory,
    pub sub_entities: [EnderDragonPart; 8],
    pub o_flap_time: f32,
    pub flap_time: f32,
    pub in_wall: bool,
    pub dragon_death_time: i32,
    pub y_rot_a: f32,
    pub nearest_crystal: Option<EntityId>,
    pub dragon_fight: bool,
    pub fight_origin: BlockPos,
    pub phase_manager: EnderDragonPhaseManager,
    ended_phase: Option<EnderDragonPhase>,
    sitting_damage_received: f32,
    nodes: Vec<Node>,
    open_set: BinaryHeap,
}

impl EnderDragon {
    pub fn create() -> Mob {
        let mut data = mob_defaults();
        data.push(DataValue::Int(EnderDragonPhase::Hovering.get_id()));
        let entity_type = registry_id(registry::ENTITY_TYPE, "minecraft:ender_dragon");
        let mut entity = Entity::new(entity_type, DIMENSIONS, data);
        let sub_entities = [
            EnderDragonPart::new(entity.id, entity_type, "head", 1.0, 1.0),
            EnderDragonPart::new(entity.id, entity_type, "neck", 3.0, 3.0),
            EnderDragonPart::new(entity.id, entity_type, "body", 5.0, 3.0),
            EnderDragonPart::new(entity.id, entity_type, "tail", 2.0, 2.0),
            EnderDragonPart::new(entity.id, entity_type, "tail", 2.0, 2.0),
            EnderDragonPart::new(entity.id, entity_type, "tail", 2.0, 2.0),
            EnderDragonPart::new(entity.id, entity_type, "wing", 4.0, 2.0),
            EnderDragonPart::new(entity.id, entity_type, "wing", 4.0, 2.0),
        ];
        entity.no_physics = true;
        let dragon = EnderDragon {
            flight_history: DragonFlightHistory::default(),
            sub_entities,
            o_flap_time: 0.0,
            flap_time: 0.0,
            in_wall: false,
            dragon_death_time: DEFAULT_DEATH_TIME,
            y_rot_a: 0.0,
            nearest_crystal: None,
            dragon_fight: false,
            fight_origin: BlockPos::ZERO,
            phase_manager: EnderDragonPhaseManager::new(),
            ended_phase: None,
            sitting_damage_received: 0.0,
            nodes: Vec::new(),
            open_set: BinaryHeap::default(),
        };
        Mob::new(entity, LivingEntity::new(&ENDER_DRAGON_ATTRIBUTES), MobKind::EnderDragon(Box::new(dragon)))
    }

    pub fn head(&self) -> EntityId {
        self.sub_entities[HEAD].entity.id
    }

    pub fn is_flapping(&self) -> bool {
        let flap = mth::cos((self.flap_time * std::f32::consts::TAU) as f64);
        let old_flap = mth::cos((self.o_flap_time * std::f32::consts::TAU) as f64);
        old_flap <= -0.3 && flap >= -0.3
    }

    pub fn add_additional_save_data(mob: &Mob, output: &mut Vec<(String, Nbt)>) {
        let dragon = ender_dragon(mob);
        output.push(("DragonPhase".into(), Nbt::Int(dragon.phase_manager.get_current_phase().get_id())));
        output.push(("DragonDeathTime".into(), Nbt::Int(dragon.dragon_death_time)));
        output.push(("sitting_damage_received".into(), Nbt::Float(dragon.sitting_damage_received)));
    }

    pub fn read_additional_save_data(mob: &mut Mob, input: &Nbt) {
        if let Some(phase_id) = input.get("DragonPhase").and_then(crate::world::level::block::entity::int_value) {
            let target = EnderDragonPhase::get_by_id(phase_id);
            let current_phase = ender_dragon(mob).phase_manager.current_phase;
            if current_phase != Some(target) {
                ender_dragon_mut(mob).ended_phase = current_phase;
                switch_phase(mob, target);
            }
        }
        let dragon = ender_dragon_mut(mob);
        dragon.dragon_death_time = input.get("DragonDeathTime").and_then(crate::world::level::block::entity::int_value).unwrap_or(DEFAULT_DEATH_TIME);
        dragon.sitting_damage_received = input.get("sitting_damage_received").and_then(crate::world::level::block::entity::float_value).unwrap_or(0.0);
    }
}

pub fn take_ended_phase(mob: &mut Mob) -> Option<EnderDragonPhase> {
    ender_dragon_mut(mob).ended_phase.take()
}

pub fn ender_dragon(mob: &Mob) -> &EnderDragon {
    match &mob.kind {
        MobKind::EnderDragon(dragon) => dragon,
        _ => unreachable!("not an ender dragon"),
    }
}

pub fn ender_dragon_mut(mob: &mut Mob) -> &mut EnderDragon {
    match &mut mob.kind {
        MobKind::EnderDragon(dragon) => dragon,
        _ => unreachable!("not an ender dragon"),
    }
}

pub fn get_ambient_sound() -> &'static str {
    "minecraft:entity.ender_dragon.ambient"
}

pub fn get_hurt_sound() -> &'static str {
    "minecraft:entity.ender_dragon.hurt"
}

fn mob_attack(dragon: EntityId) -> DamageSource {
    DamageSource { causing_entity: Some(dragon), direct_entity: Some(dragon), ..DamageSource::new(damage_types::MOB_ATTACK) }
}

impl ServerLevel {
    pub fn is_ender_dragon(&self, id: EntityId) -> bool {
        matches!(self.mob(id).map(|mob| &mob.kind), Some(MobKind::EnderDragon(_)))
    }

    pub fn ender_dragon(&self, id: EntityId) -> &EnderDragon {
        ender_dragon(self.mob(id).expect("an ender dragon"))
    }

    pub fn ender_dragon_mut(&mut self, id: EntityId) -> &mut EnderDragon {
        ender_dragon_mut(self.mob_mut(id).expect("an ender dragon"))
    }

    pub fn dragon_phase_manager(&self, id: EntityId) -> &EnderDragonPhaseManager {
        &self.ender_dragon(id).phase_manager
    }

    pub fn dragon_phase_manager_mut(&mut self, id: EntityId) -> &mut EnderDragonPhaseManager {
        &mut self.ender_dragon_mut(id).phase_manager
    }

    pub fn dragon_current_phase(&self, id: EntityId) -> EnderDragonPhase {
        self.dragon_phase_manager(id).get_current_phase()
    }

    pub fn dragon_part_parent(&self, id: EntityId) -> Option<EntityId> {
        self.dragon_parts.get(id).map(|&(parent, _)| parent)
    }

    pub fn dragon_part_entity(&self, id: EntityId) -> Option<&Entity> {
        let &(parent, index) = self.dragon_parts.get(id)?;
        match self.entities.get(&parent) {
            Some(LevelEntity::Mob(mob)) => Some(&ender_dragon(mob).sub_entities[index].entity),
            _ => None,
        }
    }

    pub fn dragon_part_entity_mut(&mut self, id: EntityId) -> Option<&mut Entity> {
        let &(parent, index) = self.dragon_parts.get(id)?;
        match self.entities.get_mut(&parent) {
            Some(LevelEntity::Mob(mob)) => Some(&mut ender_dragon_mut(mob).sub_entities[index].entity),
            _ => None,
        }
    }

    pub fn dragon_alive_crystals(&self, id: EntityId) -> Option<i32> {
        if !self.ender_dragon(id).dragon_fight {
            return None;
        }
        self.dragon_fight.as_ref().map(|fight| fight.alive_crystals())
    }

    pub fn ender_dragon_can_attack(&self, target: EntityId) -> bool {
        self.can_be_seen_as_enemy(target)
    }

    pub fn ender_dragon_ai_step(&mut self, id: EntityId) {
        self.process_flapping_movement(id);
        if !self.ender_dragon(id).dragon_fight
            && let Some(fight) = &self.dragon_fight
            && fight.dragon_uuid() == Some(self.entity(id).uuid)
        {
            self.ender_dragon_mut(id).dragon_fight = true;
        }
        let dragon = self.ender_dragon_mut(id);
        dragon.o_flap_time = dragon.flap_time;
        if self.is_dead_or_dying(id) {
            let random = &mut self.entity_mut(id).random;
            random.next_float();
            random.next_float();
            random.next_float();
            return;
        }
        self.ender_dragon_check_crystals(id);
        let movement = self.entity(id).delta_movement;
        let mut flap_speed = 0.2 / (movement.horizontal_distance() as f32 * 10.0 + 1.0);
        flap_speed *= 2.0f64.powf(movement.y) as f32;
        let sitting = self.dragon_current_phase(id).is_sitting();
        let dragon = self.ender_dragon_mut(id);
        if sitting {
            dragon.flap_time += 0.1;
        } else if dragon.in_wall {
            dragon.flap_time += flap_speed * 0.5;
        } else {
            dragon.flap_time += flap_speed;
        }
        let e = self.entity_mut(id);
        e.set_y_rot(mth::wrap_degrees(e.y_rot));
        if self.mob(id).expect("an ender dragon").is_no_ai() {
            self.ender_dragon_mut(id).flap_time = 0.5;
            return;
        }
        let (y, y_rot) = (self.entity(id).y(), self.entity(id).y_rot);
        self.ender_dragon_mut(id).flight_history.record(y, y_rot);
        let mut current_phase = self.dragon_current_phase(id);
        self.dragon_phase_do_server_tick(id, current_phase);
        if self.dragon_current_phase(id) != current_phase {
            current_phase = self.dragon_current_phase(id);
            self.dragon_phase_do_server_tick(id, current_phase);
        }
        if let Some(target_location) = self.dragon_phase_get_fly_target_location(id, current_phase) {
            self.ender_dragon_fly_towards(id, current_phase, target_location);
        }
        self.apply_effects_from_blocks(id);
        let y_rot = self.entity(id).y_rot;
        self.living_mut(id).y_body_rot = y_rot;
        let dragon = self.ender_dragon(id);
        let old_pos: [Vec3; 8] = std::array::from_fn(|i| dragon.sub_entities[i].entity.position());
        let tilt = (dragon.flight_history.get(5).y - dragon.flight_history.get(10).y) as f32 * 10.0 * mth::DEG_TO_RAD;
        let cc_tilt = mth::cos(tilt as f64);
        let ss_tilt = mth::sin(tilt as f64);
        let rot1 = y_rot * mth::DEG_TO_RAD;
        let ss1 = mth::sin(rot1 as f64);
        let cc1 = mth::cos(rot1 as f64);
        self.ender_dragon_tick_part(id, BODY, (ss1 * 0.5) as f64, 0.0, (-cc1 * 0.5) as f64);
        self.ender_dragon_tick_part(id, WING1, (cc1 * 4.5) as f64, 2.0, (ss1 * 4.5) as f64);
        self.ender_dragon_tick_part(id, WING2, (cc1 * -4.5) as f64, 2.0, (ss1 * -4.5) as f64);
        if self.living(id).hurt_time <= 0 {
            for wing in [WING1, WING2] {
                let area = self.ender_dragon(id).sub_entities[wing].entity.bounding_box().inflate(4.0, 2.0, 4.0).move_by(0.0, -2.0, 0.0);
                let entities = self.get_entities_matching(Some(id), &area, entity_selector::no_creative_or_spectator);
                self.ender_dragon_knock_back(id, entities);
            }
            for part in [HEAD, NECK] {
                let area = self.ender_dragon(id).sub_entities[part].entity.bounding_box().inflate_all(1.0);
                let entities = self.get_entities_matching(Some(id), &area, entity_selector::no_creative_or_spectator);
                self.ender_dragon_hurt_entities(id, entities);
            }
        }
        let (y_rot, y_rot_a) = (self.entity(id).y_rot, self.ender_dragon(id).y_rot_a);
        let ss2 = mth::sin((y_rot * mth::DEG_TO_RAD - y_rot_a * 0.01) as f64);
        let cc2 = mth::cos((y_rot * mth::DEG_TO_RAD - y_rot_a * 0.01) as f64);
        let y_offset = self.ender_dragon_get_head_y_offset(id);
        self.ender_dragon_tick_part(id, HEAD, (ss2 * 6.5 * cc_tilt) as f64, (y_offset + ss_tilt * 6.5) as f64, (-cc2 * 6.5 * cc_tilt) as f64);
        self.ender_dragon_tick_part(id, NECK, (ss2 * 5.5 * cc_tilt) as f64, (y_offset + ss_tilt * 5.5) as f64, (-cc2 * 5.5 * cc_tilt) as f64);
        let p1 = self.ender_dragon(id).flight_history.get(5);
        for i in 0..3 {
            let p0 = self.ender_dragon(id).flight_history.get(12 + i * 2);
            let rot = y_rot * mth::DEG_TO_RAD + mth::wrap_degrees_d((p0.y_rot - p1.y_rot) as f64) as f32 * mth::DEG_TO_RAD;
            let ss = mth::sin(rot as f64);
            let cc = mth::cos(rot as f64);
            let dd = (i + 1) as f32 * 2.0;
            self.ender_dragon_tick_part(
                id,
                TAIL1 + i as usize,
                (-(ss1 * 1.5 + ss * dd) * cc_tilt) as f64,
                p0.y - p1.y - ((dd + 1.5) * ss_tilt) as f64 + 1.5,
                ((cc1 * 1.5 + cc * dd) * cc_tilt) as f64,
            );
        }
        let head_box = self.ender_dragon(id).sub_entities[HEAD].entity.bounding_box();
        let neck_box = self.ender_dragon(id).sub_entities[NECK].entity.bounding_box();
        let body_box = self.ender_dragon(id).sub_entities[BODY].entity.bounding_box();
        let in_wall = self.ender_dragon_check_walls(id, &head_box) | self.ender_dragon_check_walls(id, &neck_box) | self.ender_dragon_check_walls(id, &body_box);
        self.ender_dragon_mut(id).in_wall = in_wall;
        if self.ender_dragon(id).dragon_fight {
            self.dragon_fight_update_dragon(id);
        }
        let dragon = self.ender_dragon_mut(id);
        for (part, old) in dragon.sub_entities.iter_mut().zip(old_pos) {
            part.entity.xo = old;
            part.entity.old_pos = old;
        }
    }

    fn ender_dragon_fly_towards(&mut self, id: EntityId, current_phase: EnderDragonPhase, target_location: Vec3) {
        let e = self.entity(id);
        let xdd = target_location.x - e.x();
        let mut ydd = target_location.y - e.y();
        let zdd = target_location.z - e.z();
        let dist_to_target = xdd * xdd + ydd * ydd + zdd * zdd;
        let max = current_phase.get_fly_speed();
        let horizontal_dist = (xdd * xdd + zdd * zdd).sqrt();
        if horizontal_dist > 0.0 {
            ydd = mth::clamp(ydd / horizontal_dist, -max as f64, max as f64);
        }
        let e = self.entity_mut(id);
        e.set_delta_movement(e.delta_movement.add(0.0, ydd * 0.01, 0.0));
        e.set_y_rot(mth::wrap_degrees(e.y_rot));
        let aim = target_location.subtract(e.position()).normalize();
        let dir = Vec3::new(mth::sin((e.y_rot * mth::DEG_TO_RAD) as f64) as f64, e.delta_movement.y, -mth::cos((e.y_rot * mth::DEG_TO_RAD) as f64) as f64).normalize();
        let dot = ((dir.dot(aim) as f32 + 0.5) / 1.5).max(0.0);
        if xdd.abs() > 1.0e-5f32 as f64 || zdd.abs() > 1.0e-5f32 as f64 {
            let y_rot = e.y_rot;
            let y_rot_d = mth::clamp_f(mth::wrap_degrees(180.0 - mth::atan2(xdd, zdd) as f32 * mth::RAD_TO_DEG - y_rot), -50.0, 50.0);
            let turn_speed = self.dragon_phase_get_turn_speed(id, current_phase);
            let dragon = self.ender_dragon_mut(id);
            dragon.y_rot_a *= 0.8;
            dragon.y_rot_a += y_rot_d * turn_speed;
            let y_rot_a = dragon.y_rot_a;
            let e = self.entity_mut(id);
            e.set_y_rot(e.y_rot + y_rot_a * 0.1);
        }
        let span = (2.0 / (dist_to_target + 1.0)) as f32;
        self.entity_mut(id).move_relative(0.06 * (dot * span + (1.0 - span)), Vec3::new(0.0, 0.0, -1.0));
        let delta = self.entity(id).delta_movement;
        if self.ender_dragon(id).in_wall {
            self.move_entity(id, MoverType::SelfMove, delta.scale(0.8f32 as f64));
        } else {
            self.move_entity(id, MoverType::SelfMove, delta);
        }
        let e = self.entity_mut(id);
        let actual = e.delta_movement.normalize();
        let slide = 0.8 + 0.15 * (actual.dot(dir) + 1.0) / 2.0;
        e.set_delta_movement(e.delta_movement.multiply(slide, 0.91f32 as f64, slide));
    }

    fn ender_dragon_tick_part(&mut self, id: EntityId, part: usize, x: f64, y: f64, z: f64) {
        let pos = self.entity(id).position().add(x, y, z);
        self.ender_dragon_mut(id).sub_entities[part].entity.set_pos(pos);
    }

    fn ender_dragon_get_head_y_offset(&self, id: EntityId) -> f32 {
        if self.dragon_current_phase(id).is_sitting() {
            return -1.0;
        }
        let dragon = self.ender_dragon(id);
        (dragon.flight_history.get(5).y - dragon.flight_history.get(0).y) as f32
    }

    fn ender_dragon_check_crystals(&mut self, id: EntityId) {
        if let Some(nearest_crystal) = self.ender_dragon(id).nearest_crystal {
            if self.entity_ref(nearest_crystal).is_none_or(Entity::is_removed) {
                self.ender_dragon_mut(id).nearest_crystal = None;
            } else if self.entity(id).tick_count % 10 == 0 && self.health(id) < self.max_health(id) {
                let health = self.health(id);
                self.set_health(id, health + 1.0);
            }
        }
        if self.entity_mut(id).random.next_int_bounded(10) == 0 {
            let area = self.entity(id).bounding_box().inflate_all(32.0);
            let crystals = self.get_entities_matching(None, &area, |level, e| matches!(level.entities.get(&e), Some(LevelEntity::EndCrystal(_))));
            let mut nearest = None;
            let mut distance = f64::MAX;
            for crystal in crystals {
                let dist = self.entity(crystal).distance_to_sqr(self.entity(id));
                if dist < distance {
                    distance = dist;
                    nearest = Some(crystal);
                }
            }
            self.ender_dragon_mut(id).nearest_crystal = nearest;
        }
    }

    fn ender_dragon_knock_back(&mut self, id: EntityId, entities: Vec<EntityId>) {
        let body = self.ender_dragon(id).sub_entities[BODY].entity.bounding_box();
        let xm = (body.min_x + body.max_x) / 2.0;
        let zm = (body.min_z + body.max_z) / 2.0;
        for entity in entities {
            if !self.is_living(entity) {
                continue;
            }
            let e = self.entity(entity);
            let xd = e.x() - xm;
            let zd = e.z() - zm;
            let dd = (xd * xd + zd * zd).max(0.1);
            self.entity_mut(entity).push(xd / dd * 4.0, 0.2f32 as f64, zd / dd * 4.0);
            if !self.dragon_current_phase(id).is_sitting() && self.living(entity).last_hurt_by_mob_timestamp < self.entity(entity).tick_count - 2 && self.do_teams_allow_damage(id, entity) {
                let damage_source = mob_attack(id);
                self.hurt_server(entity, damage_source.clone(), 5.0);
                self.do_post_attack_effects(entity, &damage_source);
            }
        }
    }

    fn ender_dragon_hurt_entities(&mut self, id: EntityId, entities: Vec<EntityId>) {
        for target in entities {
            if self.is_living(target) && self.do_teams_allow_damage(id, target) {
                let damage_source = mob_attack(id);
                self.hurt_server(target, damage_source.clone(), 10.0);
                self.do_post_attack_effects(target, &damage_source);
            }
        }
    }

    fn ender_dragon_check_walls(&mut self, id: EntityId, bb: &AABB) -> bool {
        let x0 = mth::floor(bb.min_x);
        let y0 = mth::floor(bb.min_y);
        let z0 = mth::floor(bb.min_z);
        let x1 = mth::floor(bb.max_x);
        let y1 = mth::floor(bb.max_y);
        let z1 = mth::floor(bb.max_z);
        let mut hit_wall = false;
        let mut destroyed_block = false;
        for x in x0..=x1 {
            for y in y0..=y1 {
                for z in z0..=z1 {
                    let block_pos = BlockPos::new(x, y, z);
                    let state = self.block_state(block_pos);
                    if !state.is_air() && !state.is(&block_tags::DRAGON_TRANSPARENT) {
                        if self.game_rules.get_bool(game_rule::MOB_GRIEFING) && !state.is(&block_tags::DRAGON_IMMUNE) {
                            destroyed_block = self.remove_block(block_pos, false) || destroyed_block;
                        } else {
                            hit_wall = true;
                        }
                    }
                }
            }
        }
        if destroyed_block {
            let random = &mut self.entity_mut(id).random;
            let rx = x0 + random.next_int_bounded(x1 - x0 + 1);
            let ry = y0 + random.next_int_bounded(y1 - y0 + 1);
            let rz = z0 + random.next_int_bounded(z1 - z0 + 1);
            self.level_event(None, events::PARTICLES_DRAGON_BLOCK_BREAK, BlockPos::new(rx, ry, rz), 0);
        }
        hit_wall
    }

    pub fn ender_dragon_hurt(&mut self, id: EntityId, part: usize, source: DamageSource, damage: f32) -> bool {
        let phase = self.dragon_current_phase(id);
        if phase == EnderDragonPhase::Dying {
            return false;
        }
        let mut damage = self.dragon_phase_on_hurt(phase, &source, damage);
        if part != HEAD && part != NECK {
            damage = damage / 4.0 + damage.min(1.0);
        }
        if damage < 0.01 {
            return false;
        }
        if source.causing_entity.is_some_and(|entity| self.is_player(entity)) || source.is(&damage_type_tags::ALWAYS_HURTS_ENDER_DRAGONS) {
            let health_before = self.health(id);
            self.living_entity_hurt_server(id, source, damage);
            if self.dragon_current_phase(id).is_sitting() {
                let health = self.health(id);
                let max_health = self.max_health(id);
                let dragon = self.ender_dragon_mut(id);
                dragon.sitting_damage_received = dragon.sitting_damage_received + health_before - health;
                if dragon.sitting_damage_received > SITTING_ALLOWED_DAMAGE_PERCENTAGE * max_health {
                    dragon.sitting_damage_received = 0.0;
                    self.dragon_set_phase(id, EnderDragonPhase::Takeoff);
                }
            }
        }
        true
    }

    pub fn ender_dragon_hurt_server(&mut self, id: EntityId, source: DamageSource, damage: f32) -> bool {
        self.ender_dragon_hurt(id, BODY, source, damage)
    }

    pub fn ender_dragon_part_hurt_server(&mut self, part: EntityId, source: DamageSource, damage: f32) -> bool {
        if self.is_invulnerable_to_base(part, &source) {
            return false;
        }
        let &(parent, index) = self.dragon_parts.get(part).expect("a dragon part");
        self.ender_dragon_hurt(parent, index, source, damage)
    }

    pub fn ender_dragon_handle_killing_blow(&mut self, id: EntityId) {
        if !self.dragon_current_phase(id).is_sitting() {
            self.set_health(id, 1.0);
            self.dragon_set_phase(id, EnderDragonPhase::Dying);
        }
    }

    pub fn ender_dragon_kill(&mut self, id: EntityId) {
        self.remove(id, RemovalReason::Killed);
        self.entity_game_event(id, game_event::ENTITY_DIE);
        if self.ender_dragon(id).dragon_fight {
            self.dragon_fight_update_dragon(id);
            self.dragon_fight_set_dragon_killed(id);
        }
    }

    pub fn ender_dragon_tick_death(&mut self, id: EntityId) {
        if self.ender_dragon(id).dragon_fight {
            self.dragon_fight_update_dragon(id);
        }
        let dragon = self.ender_dragon_mut(id);
        dragon.dragon_death_time += 1;
        let dragon_death_time = dragon.dragon_death_time;
        if (180..=200).contains(&dragon_death_time) {
            let random = &mut self.entity_mut(id).random;
            random.next_float();
            random.next_float();
            random.next_float();
        }
        let mut xp_count = 500;
        if self.ender_dragon(id).dragon_fight && self.dragon_fight.as_ref().is_some_and(|fight| !fight.has_previously_killed_dragon()) {
            xp_count = 12000;
        }
        if dragon_death_time > 150 && dragon_death_time % 5 == 0 && self.game_rules.get_bool(game_rule::MOB_DROPS) {
            let position = self.entity(id).position();
            self.award_experience(position, mth::floor_f(xp_count as f32 * 0.08));
        }
        if dragon_death_time == 1 && !self.entity(id).is_silent() {
            let block_position = self.entity(id).block_position();
            self.global_level_event(events::SOUND_DRAGON_DEATH, block_position, 0);
        }
        let death_move = Vec3::new(0.0, 0.1f32 as f64, 0.0);
        self.move_entity(id, MoverType::SelfMove, death_move);
        for part in &mut self.ender_dragon_mut(id).sub_entities {
            part.entity.set_old_pos_and_rot();
            let pos = part.entity.position().add_vec(death_move);
            part.entity.set_pos(pos);
        }
        if dragon_death_time >= 200 {
            if self.game_rules.get_bool(game_rule::MOB_DROPS) {
                let position = self.entity(id).position();
                self.award_experience(position, mth::floor_f(xp_count as f32 * 0.2));
            }
            if self.ender_dragon(id).dragon_fight {
                self.dragon_fight_set_dragon_killed(id);
            }
            self.remove(id, RemovalReason::Killed);
            self.entity_game_event(id, game_event::ENTITY_DIE);
        }
    }

    pub fn dragon_find_closest_node(&mut self, id: EntityId) -> i32 {
        if self.ender_dragon(id).nodes.is_empty() {
            let mut nodes = Vec::with_capacity(24);
            for i in 0..24 {
                let mut y_adjustment = 5;
                let mut multiplier = i;
                let (node_x, node_z);
                if i < 12 {
                    node_x = mth::floor_f(60.0 * mth::cos((2.0 * (-std::f32::consts::PI + (std::f64::consts::PI / 12.0) as f32 * multiplier as f32)) as f64));
                    node_z = mth::floor_f(60.0 * mth::sin((2.0 * (-std::f32::consts::PI + (std::f64::consts::PI / 12.0) as f32 * multiplier as f32)) as f64));
                } else if i < 20 {
                    multiplier -= 12;
                    node_x = mth::floor_f(40.0 * mth::cos((2.0 * (-std::f32::consts::PI + (std::f64::consts::PI / 8.0) as f32 * multiplier as f32)) as f64));
                    node_z = mth::floor_f(40.0 * mth::sin((2.0 * (-std::f32::consts::PI + (std::f64::consts::PI / 8.0) as f32 * multiplier as f32)) as f64));
                    y_adjustment += 10;
                } else {
                    multiplier -= 20;
                    node_x = mth::floor_f(20.0 * mth::cos((2.0 * (-std::f32::consts::PI + (std::f64::consts::PI / 4.0) as f32 * multiplier as f32)) as f64));
                    node_z = mth::floor_f(20.0 * mth::sin((2.0 * (-std::f32::consts::PI + (std::f64::consts::PI / 4.0) as f32 * multiplier as f32)) as f64));
                }
                let node_y = 73.max(self.get_heightmap_pos(HeightmapType::MotionBlockingNoLeaves, BlockPos::new(node_x, 0, node_z)).y + y_adjustment);
                nodes.push(Node::new(node_x, node_y, node_z));
            }
            self.ender_dragon_mut(id).nodes = nodes;
        }
        let e = self.entity(id);
        let (x, y, z) = (e.x(), e.y(), e.z());
        self.dragon_find_closest_node_to(id, x, y, z)
    }

    pub fn dragon_find_closest_node_to(&self, id: EntityId, t_x: f64, t_y: f64, t_z: f64) -> i32 {
        let mut closest_dist = 10000.0f32;
        let mut closest_index = 0;
        let current_pos = Node::new(mth::floor(t_x), mth::floor(t_y), mth::floor(t_z));
        let start_index = if self.dragon_alive_crystals(id).is_none_or(|crystals| crystals == 0) { 12 } else { 0 };
        let nodes = &self.ender_dragon(id).nodes;
        for i in start_index..24 {
            if let Some(node) = nodes.get(i) {
                let dist = node.distance_to_sqr(&current_pos);
                if dist < closest_dist {
                    closest_dist = dist;
                    closest_index = i as i32;
                }
            }
        }
        closest_index
    }

    pub fn dragon_find_path(&mut self, id: EntityId, start_index: i32, end_index: i32, final_node: Option<Node>) -> Option<Path> {
        let minimum_node_index = if self.dragon_alive_crystals(id).is_none_or(|crystals| crystals == 0) { 12 } else { 0 };
        let dragon = self.ender_dragon_mut(id);
        let nodes = &mut dragon.nodes;
        for node in nodes.iter_mut() {
            node.closed = false;
            node.f = 0.0;
            node.g = 0.0;
            node.h = 0.0;
            node.came_from = None;
            node.heap_idx = -1;
        }
        let (from, mut to) = (start_index as u32, end_index as u32);
        let h = nodes[from as usize].distance_to(&nodes[to as usize]);
        let from_node = &mut nodes[from as usize];
        from_node.g = 0.0;
        from_node.h = h;
        from_node.f = h;
        let open_set = &mut dragon.open_set;
        open_set.clear();
        open_set.insert(nodes, from);
        let mut closest = from;
        while !open_set.is_empty() {
            let open_node = open_set.pop(nodes);
            let (o, t) = (nodes[open_node as usize], nodes[to as usize]);
            if o.x == t.x && o.y == t.y && o.z == t.z {
                if let Some(mut final_node) = final_node {
                    final_node.came_from = Some(to);
                    nodes.push(final_node);
                    to = 24;
                }
                let path = reconstruct_path(nodes, to);
                nodes.truncate(24);
                return Some(path);
            }
            if o.distance_to(&t) < nodes[closest as usize].distance_to(&t) {
                closest = open_node;
            }
            nodes[open_node as usize].closed = true;
            let x_index = open_node as usize;
            for i in minimum_node_index..24 {
                if NODE_ADJACENCY[x_index] & 1 << i > 0 {
                    let adjacent = nodes[i];
                    if !adjacent.closed {
                        let tentative_g_score = nodes[open_node as usize].g + nodes[open_node as usize].distance_to(&adjacent);
                        if !adjacent.in_open_set() || tentative_g_score < adjacent.g {
                            let h = adjacent.distance_to(&nodes[to as usize]);
                            let adjacent_node = &mut nodes[i];
                            adjacent_node.came_from = Some(open_node);
                            adjacent_node.g = tentative_g_score;
                            adjacent_node.h = h;
                            if adjacent_node.in_open_set() {
                                let cost = adjacent_node.g + adjacent_node.h;
                                open_set.change_cost(nodes, i as u32, cost);
                            } else {
                                adjacent_node.f = adjacent_node.g + adjacent_node.h;
                                open_set.insert(nodes, i as u32);
                            }
                        }
                    }
                }
            }
        }
        if closest == from {
            return None;
        }
        if let Some(mut final_node) = final_node {
            final_node.came_from = Some(closest);
            nodes.push(final_node);
            closest = 24;
        }
        let path = reconstruct_path(nodes, closest);
        nodes.truncate(24);
        Some(path)
    }

    pub fn ender_dragon_get_head_look_vector(&mut self, id: EntityId) -> Vec3 {
        let phase = self.dragon_current_phase(id);
        let y_rot = self.entity(id).y_head_rot;
        if phase == EnderDragonPhase::Landing || phase == EnderDragonPhase::Takeoff {
            let fight_origin = self.ender_dragon(id).fight_origin;
            let egg = self.get_heightmap_pos(HeightmapType::MotionBlockingNoLeaves, ender_dragon_fight::get_podium_location(fight_origin));
            let position = self.entity(id).position();
            let dist = ((egg.dist_to_center_sqr(position.x, position.y, position.z).sqrt() as f32) / 4.0).max(1.0);
            let y_offset = 6.0 / dist;
            calculate_view_vector(clamped_x_rot(-y_offset * 1.5 * 5.0), y_rot)
        } else if phase.is_sitting() {
            calculate_view_vector(clamped_x_rot(-45.0), y_rot)
        } else {
            self.get_view_vector(id)
        }
    }

    pub fn ender_dragon_on_crystal_destroyed(&mut self, id: EntityId, crystal: EntityId, pos: BlockPos, source: &DamageSource) {
        let player = match source.causing_entity.filter(|&entity| self.is_player(entity)) {
            Some(player) => Some(player),
            None => self.get_nearest_player(&CRYSTAL_DESTROY_TARGETING, None, pos.x as f64, pos.y as f64, pos.z as f64),
        };
        if self.ender_dragon(id).nearest_crystal == Some(crystal) {
            let ty = if player.is_some() { damage_types::PLAYER_EXPLOSION } else { damage_types::EXPLOSION };
            let damage_source = DamageSource { ty, causing_entity: player, direct_entity: Some(crystal), damage_source_position: None };
            self.ender_dragon_hurt(id, HEAD, damage_source, 10.0);
        }
        let phase = self.dragon_current_phase(id);
        self.dragon_phase_on_crystal_destroyed(id, phase, player);
    }
}

fn clamped_x_rot(x_rot: f32) -> f32 {
    (x_rot % 360.0).clamp(-90.0, 90.0)
}

fn reconstruct_path(nodes: &[Node], to: u32) -> Path {
    let mut path = Vec::new();
    let mut node = to;
    path.push(nodes[node as usize]);
    while let Some(came_from) = nodes[node as usize].came_from {
        node = came_from;
        path.insert(0, nodes[node as usize]);
    }
    let to = &nodes[to as usize];
    Path::new(path, BlockPos::new(to.x, to.y, to.z), true)
}
