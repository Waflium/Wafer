use wafer_core::{BlockPos, Vec3};
use wafer_data::{damage_type_tags, mob_effects, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use super::anger_level::AngerLevel;
use super::anger_management::AngerManagement;
use super::warden_ai;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::level::server_level_accessor::ServerLevelAccessor;
use crate::world::damagesource::DamageSource;
use crate::world::effect::{MobEffectInstance, mob_effect_util};
use crate::world::entity::ai::behavior::warden::sonic_boom;
use crate::world::entity::ai::brain::{self, Brain, Provider};
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::navigation::path_navigation::{PathNavigation, PathNavigationKind};
use crate::world::entity::ai::sensing::sensor_type::SensorType;
use crate::world::entity::attributes::WARDEN_ATTRIBUTES;
use crate::world::entity::data::{DataValue, Pose, mob_defaults};
use crate::world::entity::entity_event;
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::{Entity, EntityDimensions, EntityId, entity_selector, sound_event};
use crate::world::level::gameevent::dynamic_game_event_listener::DynamicGameEventListener;
use crate::world::level::gameevent::game_event::{Context, GameEvent};
use crate::world::level::gameevent::game_event_listener::{EntityListenerKind, GameEventListener};
use crate::world::level::gameevent::position_source::PositionSource;
use crate::world::level::gameevent::vibrations::vibration_system::{Data, VibrationUser};
use crate::world::level::pathfinder::path_type::PathType;

pub const DATA_CLIENT_ANGER_LEVEL: usize = 16;
const VIBRATION_COOLDOWN_TICKS: i64 = 40;
const TIME_TO_USE_MELEE_UNTIL_SONIC_BOOM: i32 = 200;
const DARKNESS_DISPLAY_LIMIT: i32 = 200;
const DARKNESS_DURATION: i32 = 260;
const DARKNESS_RADIUS: i32 = 20;
const DARKNESS_INTERVAL: i32 = 120;
const ANGERMANAGEMENT_TICK_DELAY: i32 = 20;
const DEFAULT_ANGER: i32 = 35;
const PROJECTILE_ANGER: i32 = 10;
const ON_HURT_ANGER_BOOST: i32 = 20;
const RECENT_PROJECTILE_TICK_THRESHOLD: i64 = 100;
const TOUCH_COOLDOWN_TICKS: i64 = 20;
const PROJECTILE_ANGER_DISTANCE: f64 = 30.0;
pub const GAME_EVENT_LISTENER_RANGE: i32 = 16;
const CHEST_ATTACHMENT_Y: f32 = 1.6;
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.9, 2.9).with_passenger_attachments(&[Vec3::new(0.0, 3.15f32 as f64, 0.0)]);

pub static BRAIN_PROVIDER: Provider = brain::provider_with_memories(
    &[
        memory_module_type::NEAREST_VISIBLE_NEMESIS.erase(),
        memory_module_type::RECENT_PROJECTILE.erase(),
        memory_module_type::TOUCH_COOLDOWN.erase(),
        memory_module_type::VIBRATION_COOLDOWN.erase(),
    ],
    &[SensorType::NearestPlayers, SensorType::WardenEntitySensor],
    warden_ai::get_activities,
);

pub struct Warden {
    pub dynamic_game_event_listener: DynamicGameEventListener,
    vibration_data: Data,
    anger_management: AngerManagement,
}

impl Warden {
    fn new(id: EntityId) -> Warden {
        Warden {
            dynamic_game_event_listener: DynamicGameEventListener::new(GameEventListener::Entity { entity: id, kind: EntityListenerKind::Vibration }),
            vibration_data: Data::default(),
            anger_management: AngerManagement::new(Vec::new()),
        }
    }

    pub fn create() -> Mob {
        let mut data = mob_defaults();
        data.push(DataValue::Int(0));
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:warden"), DIMENSIONS, data);
        let id = entity.id;
        let mut mob = Mob::new(entity, LivingEntity::new(&WARDEN_ATTRIBUTES), MobKind::Warden(Warden::new(id)));
        mob.xp_reward = 5;
        mob.ai.navigation.set_can_float(true);
        mob.set_pathfinding_malus(PathType::UnpassableRail, 0.0);
        mob.set_pathfinding_malus(PathType::Damaging, 8.0);
        mob.set_pathfinding_malus(PathType::PowderSnow, 8.0);
        mob.set_pathfinding_malus(PathType::Lava, 8.0);
        mob.set_pathfinding_malus(PathType::Fire, 0.0);
        mob.set_pathfinding_malus(PathType::FireInNeighbor, 0.0);
        mob
    }

    pub fn create_navigation(follow_range_base: f64) -> PathNavigation {
        PathNavigation::of_kind(PathNavigationKind::Warden, follow_range_base)
    }

    pub fn make_brain(mob: &mut Mob, packed_brain: brain::Packed) -> Brain {
        BRAIN_PROVIDER.make_brain(mob, packed_brain)
    }

    pub fn add_additional_save_data(mob: &Mob, output: &mut Vec<(String, Nbt)>) {
        let warden = warden(mob);
        output.push(("anger".into(), warden.anger_management.save()));
        output.push(("listener".into(), warden.vibration_data.to_nbt()));
    }

    pub fn read_additional_save_data(mob: &mut Mob, input: &Nbt) {
        warden_mut(mob).anger_management = AngerManagement::load(input.get("anger"));
        sync_client_anger_level(mob);
        warden_mut(mob).vibration_data = Data::load(input.get("listener"));
    }
}

fn get_active_anger(mob: &Mob) -> i32 {
    let target = mob.living.brain.get_memory(memory_module_type::ATTACK_TARGET).copied();
    warden(mob).anger_management.get_active_anger(target)
}

pub fn get_anger_level(mob: &Mob) -> AngerLevel {
    AngerLevel::by_anger(get_active_anger(mob))
}

fn sync_client_anger_level(mob: &mut Mob) {
    let anger = get_active_anger(mob);
    mob.entity.data.set(DATA_CLIENT_ANGER_LEVEL, DataValue::Int(anger));
}

pub fn get_ambient_sound(mob: &Mob) -> Option<&'static str> {
    if mob.entity.pose() != Pose::Roaring && !is_digging_or_emerging(&mob.entity) { Some(get_anger_level(mob).get_ambient_sound()) } else { None }
}

pub fn passenger_attachment_y(entity: &Entity) -> f64 {
    if is_digging_or_emerging(entity) { 1.0 } else { 3.15f32 as f64 }
}

pub fn finalize_spawn(level: &mut (impl ServerLevelAccessor + ?Sized), mob: &mut Mob, spawn_reason: EntitySpawnReason) {
    mob.living.brain.set_memory_with_expiry(memory_module_type::DIG_COOLDOWN, (), warden_ai::DIGGING_COOLDOWN as i64);
    if spawn_reason == EntitySpawnReason::Triggered {
        mob.entity.set_pose(Pose::Emerging);
        mob.refresh_dimensions();
        mob.living.brain.set_memory_with_expiry(memory_module_type::IS_EMERGING, (), warden_ai::EMERGE_DURATION as i64);
        if !mob.entity.is_silent()
            && let Some(level) = level.as_server_level()
        {
            let (x, y, z) = (mob.entity.x(), mob.entity.y(), mob.entity.z());
            level.play_sound_at(None, x, y, z, sound_event("minecraft:entity.warden.agitated"), SoundSource::Hostile, 5.0, 1.0);
        }
    }
}

pub fn warden(mob: &Mob) -> &Warden {
    match &mob.kind {
        MobKind::Warden(warden) => warden,
        _ => unreachable!("a warden"),
    }
}

pub fn warden_mut(mob: &mut Mob) -> &mut Warden {
    match &mut mob.kind {
        MobKind::Warden(warden) => warden,
        _ => unreachable!("a warden"),
    }
}

pub fn is_digging_or_emerging(entity: &Entity) -> bool {
    matches!(entity.pose(), Pose::Digging | Pose::Emerging)
}

pub fn get_hurt_sound() -> &'static str {
    "minecraft:entity.warden.hurt"
}

pub fn get_death_sound() -> &'static str {
    "minecraft:entity.warden.death"
}

pub fn get_sound_volume() -> f32 {
    4.0
}

pub fn get_walk_target_value() -> f32 {
    0.0
}

pub fn get_seconds_to_disable_blocking() -> f32 {
    5.0
}

pub fn next_step(entity: &Entity) -> f32 {
    entity.move_dist + 0.55
}

pub fn get_default_dimensions(entity: &Entity) -> EntityDimensions {
    if is_digging_or_emerging(entity) { EntityDimensions::fixed(DIMENSIONS.width, 1.0) } else { DIMENSIONS }
}

pub fn chest_attachment(level: &ServerLevel, body: EntityId) -> Vec3 {
    Vec3::new(0.0, (CHEST_ATTACHMENT_Y * level.attribute_value(body, crate::world::entity::attributes::Attribute::Scale) as f32) as f64, 0.0)
}

pub fn apply_darkness_around(level: &mut ServerLevel, position: Vec3, source: Option<EntityId>, darkness_radius: i32) {
    let darkness = MobEffectInstance::with(mob_effects::DARKNESS, DARKNESS_DURATION, 0, false, false, false);
    mob_effect_util::add_effect_to_players_around(level, source, position, darkness_radius as f64, &darkness, DARKNESS_DISPLAY_LIMIT);
}

impl ServerLevel {
    pub fn is_warden(&self, id: EntityId) -> bool {
        matches!(self.mob(id).map(|mob| &mob.kind), Some(MobKind::Warden(_)))
    }

    pub fn warden_vibration_data(&self, id: EntityId) -> Option<&Data> {
        match self.mob(id).map(|mob| &mob.kind) {
            Some(MobKind::Warden(warden)) => Some(&warden.vibration_data),
            _ => None,
        }
    }

    pub fn warden_vibration_data_mut(&mut self, id: EntityId) -> Option<&mut Data> {
        match self.mob_mut(id).map(|mob| &mut mob.kind) {
            Some(MobKind::Warden(warden)) => Some(&mut warden.vibration_data),
            _ => None,
        }
    }

    pub fn warden_position_source(&self, id: EntityId) -> PositionSource {
        PositionSource::Entity { entity: id, y_offset: self.entity(id).dimensions.eye_height }
    }

    pub fn warden_listener_source(&self, id: EntityId) -> PositionSource {
        self.warden_position_source(id)
    }

    pub fn warden_handle_game_event(&mut self, id: EntityId, event: GameEvent, context: Context, source_position: Vec3) -> bool {
        self.vibration_listener_handle_game_event(VibrationUser::Warden(id), event, context, source_position)
    }

    pub fn warden_get_client_anger_level(&self, id: EntityId) -> i32 {
        match self.entity(id).data.get(DATA_CLIENT_ANGER_LEVEL) {
            DataValue::Int(anger) => *anger,
            _ => 0,
        }
    }

    fn warden_sync_client_anger_level(&mut self, id: EntityId) {
        sync_client_anger_level(self.mob_mut(id).expect("a warden"));
    }

    pub fn warden_tick(&mut self, id: EntityId) {
        self.vibration_ticker_tick(VibrationUser::Warden(id));
        if self.mob(id).is_none() {
            return;
        }
        if self.mob(id).expect("a warden").is_persistence_required() || self.requires_custom_persistence(id) {
            warden_ai::set_dig_cooldown(self, id);
        }
        self.mob_tick(id);
    }

    pub fn warden_custom_server_ai_step(&mut self, id: EntityId) {
        Brain::tick(self, id);
        if self.mob(id).is_none() {
            return;
        }
        let e = self.entity(id);
        if e.tick_count.wrapping_add(id) % DARKNESS_INTERVAL == 0 {
            let position = e.position();
            apply_darkness_around(self, position, Some(id), DARKNESS_RADIUS);
        }
        if self.entity(id).tick_count % ANGERMANAGEMENT_TICK_DELAY == 0 {
            let mut anger_management = std::mem::replace(&mut warden_mut(self.mob_mut(id).expect("a warden")).anger_management, AngerManagement::new(Vec::new()));
            anger_management.tick(self, |level, entity| level.warden_can_target_entity(id, Some(entity)));
            warden_mut(self.mob_mut(id).expect("a warden")).anger_management = anger_management;
            self.warden_sync_client_anger_level(id);
        }
        warden_ai::update_activity(self.get_brain_mut(id));
    }

    pub fn warden_play_step_sound(&mut self, id: EntityId) {
        self.entity_play_sound(id, sound_event("minecraft:entity.warden.step"), 10.0, 1.0);
    }

    pub fn warden_do_hurt_target(&mut self, id: EntityId, target: EntityId) -> bool {
        self.broadcast_entity_event(id, entity_event::START_ATTACKING);
        let pitch = self.voice_pitch(id);
        self.entity_play_sound(id, sound_event("minecraft:entity.warden.attack_impact"), 10.0, pitch);
        sonic_boom::set_cooldown(self, id, sonic_boom::COOLDOWN);
        self.mob_do_hurt_target(id, target)
    }

    pub fn warden_is_invulnerable_to(&self, id: EntityId, source: &DamageSource) -> bool {
        is_digging_or_emerging(self.entity(id)) && !source.is(&damage_type_tags::BYPASSES_INVULNERABILITY)
    }

    pub fn warden_check_spawn_obstruction(&mut self, id: EntityId) -> bool {
        let bounding_box = DIMENSIONS.make_bounding_box(self.entity(id).position());
        self.no_collision_for(id, &bounding_box)
    }

    pub fn warden_can_target_entity(&self, id: EntityId, entity: Option<EntityId>) -> bool {
        let Some(entity) = entity.filter(|&entity| self.entity_ref(entity).is_some() && self.is_living(entity)) else { return false };
        let entity_type = self.entity(entity).entity_type;
        self.is_in_level(entity)
            && entity_selector::no_creative_or_spectator(self, entity)
            && !self.is_allied_to(id, entity)
            && entity_type != registry_id(registry::ENTITY_TYPE, "minecraft:armor_stand")
            && entity_type != registry_id(registry::ENTITY_TYPE, "minecraft:warden")
            && !self.entity(entity).is_invulnerable()
            && !self.is_dead_or_dying(entity)
            && self.world_border.is_within_bounds_aabb(&self.entity(entity).bounding_box())
    }

    fn warden_play_listening_sound(&mut self, id: EntityId) {
        if self.entity(id).pose() != Pose::Roaring {
            let sound = self.warden_get_anger_level(id).get_listening_sound();
            let pitch = self.voice_pitch(id);
            self.entity_play_sound(id, sound_event(sound), 10.0, pitch);
        }
    }

    pub fn warden_get_anger_level(&self, id: EntityId) -> AngerLevel {
        get_anger_level(self.mob(id).expect("a warden"))
    }

    pub fn warden_clear_anger(&mut self, id: EntityId, entity: EntityId) {
        let mut anger_management = std::mem::replace(&mut warden_mut(self.mob_mut(id).expect("a warden")).anger_management, AngerManagement::new(Vec::new()));
        anger_management.clear_anger(self, entity);
        warden_mut(self.mob_mut(id).expect("a warden")).anger_management = anger_management;
    }

    pub fn warden_increase_anger_at_default(&mut self, id: EntityId, entity: Option<EntityId>) {
        self.warden_increase_anger_at(id, entity, DEFAULT_ANGER, true);
    }

    pub fn warden_increase_anger_at(&mut self, id: EntityId, entity: Option<EntityId>, amount: i32, play_sound: bool) {
        if self.mob(id).expect("a warden").is_no_ai() || !self.warden_can_target_entity(id, entity) {
            return;
        }
        let entity = entity.expect("a target");
        warden_ai::set_dig_cooldown(self, id);
        let maybe_switch_target = !self.get_target_from_brain(id).is_some_and(|target| self.is_player(target));
        let mut anger_management = std::mem::replace(&mut warden_mut(self.mob_mut(id).expect("a warden")).anger_management, AngerManagement::new(Vec::new()));
        let new_anger = anger_management.increase_anger(self, entity, amount);
        warden_mut(self.mob_mut(id).expect("a warden")).anger_management = anger_management;
        if self.is_player(entity) && maybe_switch_target && AngerLevel::by_anger(new_anger).is_angry() {
            self.get_brain_mut(id).erase_memory(memory_module_type::ATTACK_TARGET);
        }
        if play_sound {
            self.warden_play_listening_sound(id);
        }
    }

    pub fn warden_get_entity_angry_at(&self, id: EntityId) -> Option<EntityId> {
        if !self.warden_get_anger_level(id).is_angry() {
            return None;
        }
        warden(self.mob(id).expect("a warden")).anger_management.get_active_entity(self, |level, entity| level.warden_can_target_entity(id, Some(entity)))
    }

    pub fn warden_hurt_server(&mut self, id: EntityId, source: DamageSource, damage: f32) -> bool {
        let attacker = source.causing_entity;
        let is_direct = source.is_direct();
        let was_hurt = self.living_entity_hurt_server(id, source, damage);
        if self.mob(id).is_none() {
            return was_hurt;
        }
        if !self.mob(id).expect("a warden").is_no_ai() && !is_digging_or_emerging(self.entity(id)) {
            self.warden_increase_anger_at(id, attacker, AngerLevel::Angry.get_minimum_anger() + ON_HURT_ANGER_BOOST, false);
            if self.get_brain(id).get_memory(memory_module_type::ATTACK_TARGET).is_none()
                && let Some(living_attacker) = attacker.filter(|&attacker| self.entity_ref(attacker).is_some() && self.is_living(attacker))
                && (is_direct || self.entity(id).position().closer_than(self.entity(living_attacker).position(), 5.0))
            {
                self.warden_set_attack_target(id, living_attacker);
            }
        }
        was_hurt
    }

    pub fn warden_set_attack_target(&mut self, id: EntityId, target: EntityId) {
        let brain = self.get_brain_mut(id);
        brain.erase_memory(memory_module_type::ROAR_TARGET);
        brain.set_memory(memory_module_type::ATTACK_TARGET, target);
        brain.erase_memory(memory_module_type::CANT_REACH_WALK_TARGET_SINCE);
        sonic_boom::set_cooldown(self, id, TIME_TO_USE_MELEE_UNTIL_SONIC_BOOM);
    }

    pub fn warden_is_pushable(&mut self, id: EntityId) -> bool {
        !is_digging_or_emerging(self.entity(id)) && self.living_entity_is_pushable(id)
    }

    pub fn warden_do_push(&mut self, id: EntityId, entity: EntityId) {
        if !self.mob(id).expect("a warden").is_no_ai() && !self.get_brain(id).has_memory_value(memory_module_type::TOUCH_COOLDOWN) {
            self.get_brain_mut(id).set_memory_with_expiry(memory_module_type::TOUCH_COOLDOWN, (), TOUCH_COOLDOWN_TICKS);
            self.warden_increase_anger_at_default(id, Some(entity));
            let position = self.entity(entity).block_position();
            warden_ai::set_disturbance_location(self, id, position);
        }
        self.living_entity_do_push(id, entity);
    }

    pub fn warden_can_receive_vibration(&self, id: EntityId, pos: BlockPos, context: Context) -> bool {
        let Some(mob) = self.mob(id) else { return false };
        if mob.is_no_ai()
            || self.is_dead_or_dying(id)
            || self.get_brain(id).has_memory_value(memory_module_type::VIBRATION_COOLDOWN)
            || is_digging_or_emerging(&mob.entity)
            || !self.world_border.is_within_bounds_pos(pos)
        {
            return false;
        }
        !context.source_entity.is_some_and(|entity| self.entity_ref(entity).is_some() && self.is_living(entity) && !self.warden_can_target_entity(id, Some(entity)))
    }

    pub fn warden_on_receive_vibration(&mut self, id: EntityId, pos: BlockPos, source_entity: Option<EntityId>, projectile_owner: Option<EntityId>) {
        if self.is_dead_or_dying(id) {
            return;
        }
        self.get_brain_mut(id).set_memory_with_expiry(memory_module_type::VIBRATION_COOLDOWN, (), VIBRATION_COOLDOWN_TICKS);
        self.broadcast_entity_event(id, entity_event::TENDRILS_SHIVER);
        let pitch = self.voice_pitch(id);
        self.entity_play_sound(id, sound_event("minecraft:entity.warden.tendril_clicks"), 5.0, pitch);
        let mut suspicious_pos = pos;
        if let Some(projectile_owner) = projectile_owner.filter(|&owner| self.entity_ref(owner).is_some()) {
            if self.entity(id).position().closer_than(self.entity(projectile_owner).position(), PROJECTILE_ANGER_DISTANCE) {
                if self.get_brain(id).has_memory_value(memory_module_type::RECENT_PROJECTILE) {
                    if self.warden_can_target_entity(id, Some(projectile_owner)) {
                        suspicious_pos = self.entity(projectile_owner).block_position();
                    }
                    self.warden_increase_anger_at_default(id, Some(projectile_owner));
                } else {
                    self.warden_increase_anger_at(id, Some(projectile_owner), PROJECTILE_ANGER, true);
                }
            }
            self.get_brain_mut(id).set_memory_with_expiry(memory_module_type::RECENT_PROJECTILE, (), RECENT_PROJECTILE_TICK_THRESHOLD);
        } else {
            self.warden_increase_anger_at_default(id, source_entity);
        }
        if !self.warden_get_anger_level(id).is_angry() {
            let active_entity = warden(self.mob(id).expect("a warden")).anger_management.get_active_entity(self, |level, entity| level.warden_can_target_entity(id, Some(entity)));
            if projectile_owner.is_some() || active_entity.is_none() || active_entity == source_entity {
                warden_ai::set_disturbance_location(self, id, suspicious_pos);
            }
        }
    }
}
