use wafer_core::BlockPos;
use wafer_data::properties::{CREAKING_HEART_STATE, CreakingHeartState};
use wafer_data::{blocks, damage_type_tags, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use super::creaking_ai;
use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::entity::ai::brain::{self, Brain, Provider};
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::sensing::sensor_type::SensorType;
use crate::world::entity::attributes::CREAKING_ATTRIBUTES;
use crate::world::entity::data::{DataValue, mob_defaults};
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::{Entity, EntityDimensions, EntityId, RemovalReason, sound_event};
use crate::world::level::block::entity::BlockEntityKind;
use crate::world::level::pathfinder::path_type::PathType;

pub const DATA_CAN_MOVE: usize = 16;
pub const DATA_IS_ACTIVE: usize = 17;
pub const DATA_IS_TEARING_DOWN: usize = 18;
pub const DATA_HOME_POS: usize = 19;
const ATTACK_ANIMATION_DURATION: i32 = 15;
const ACTIVATION_RANGE_SQ: f64 = 144.0;
pub const ATTACK_INTERVAL: i32 = 40;
pub const SPEED_MULTIPLIER_WHEN_IDLING: f32 = 0.3;
pub const INVULNERABILITY_ANIMATION_DURATION: i32 = 8;
pub const TWITCH_DEATH_DURATION: i32 = 45;
const MAX_PLAYER_STUCK_COUNTER: i32 = 4;
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.9, 2.7).with_eye_height(2.3);

pub static BRAIN_PROVIDER: Provider = brain::provider(&[SensorType::NearestLivingEntities, SensorType::NearestPlayers], |_| creaking_ai::get_activities());

#[derive(Default)]
pub struct Creaking {
    attack_animation_remaining_ticks: i32,
    invulnerability_animation_remaining_ticks: i32,
    player_stuck_counter: i32,
}

fn creaking_mut(mob: &mut Mob) -> &mut Creaking {
    match &mut mob.kind {
        MobKind::Creaking(creaking) => creaking,
        _ => unreachable!("a creaking"),
    }
}

fn creaking(mob: &Mob) -> &Creaking {
    match &mob.kind {
        MobKind::Creaking(creaking) => creaking,
        _ => unreachable!("a creaking"),
    }
}

pub fn can_move(entity: &Entity) -> bool {
    matches!(entity.data.get(DATA_CAN_MOVE), DataValue::Boolean(true))
}

pub fn is_active(entity: &Entity) -> bool {
    matches!(entity.data.get(DATA_IS_ACTIVE), DataValue::Boolean(true))
}

pub fn is_tearing_down(entity: &Entity) -> bool {
    matches!(entity.data.get(DATA_IS_TEARING_DOWN), DataValue::Boolean(true))
}

pub fn get_home_pos(entity: &Entity) -> Option<BlockPos> {
    match entity.data.get(DATA_HOME_POS) {
        DataValue::OptionalBlockPos(pos) => *pos,
        _ => None,
    }
}

pub fn is_heart_bound(entity: &Entity) -> bool {
    get_home_pos(entity).is_some()
}

pub fn get_ambient_sound(entity: &Entity) -> Option<&'static str> {
    if is_active(entity) { None } else { Some("minecraft:entity.creaking.ambient") }
}

pub fn get_hurt_sound(entity: &Entity) -> &'static str {
    if is_heart_bound(entity) { "minecraft:entity.creaking.sway" } else { "minecraft:entity.hostile.hurt" }
}

pub fn get_death_sound() -> &'static str {
    "minecraft:entity.creaking.death"
}

pub fn get_walk_target_value() -> f32 {
    0.0
}

fn set_transient(mob: &mut Mob, pos: BlockPos) {
    mob.entity.data.set(DATA_HOME_POS, DataValue::OptionalBlockPos(Some(pos)));
    mob.set_pathfinding_malus(PathType::Damaging, 8.0);
    mob.set_pathfinding_malus(PathType::PowderSnow, 8.0);
    mob.set_pathfinding_malus(PathType::Lava, 8.0);
    mob.set_pathfinding_malus(PathType::Fire, 0.0);
    mob.set_pathfinding_malus(PathType::FireInNeighbor, 0.0);
}

impl Creaking {
    pub fn create() -> Mob {
        let mut data = mob_defaults();
        data.extend([DataValue::Boolean(true), DataValue::Boolean(false), DataValue::Boolean(false), DataValue::OptionalBlockPos(None)]);
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:creaking"), DIMENSIONS, data);
        let mut mob = Mob::new(entity, LivingEntity::new(&CREAKING_ATTRIBUTES), MobKind::Creaking(Creaking::default()));
        mob.ai.navigation.set_can_float(true);
        mob.xp_reward = 0;
        mob
    }

    pub fn make_brain(mob: &mut Mob, packed_brain: brain::Packed) -> Brain {
        BRAIN_PROVIDER.make_brain(mob, packed_brain)
    }

    pub fn add_additional_save_data(mob: &Mob, output: &mut Vec<(String, Nbt)>) {
        if let Some(home_pos) = get_home_pos(&mob.entity) {
            output.push(("home_pos".into(), Nbt::IntArray(vec![home_pos.x, home_pos.y, home_pos.z])));
        }
    }

    pub fn read_additional_save_data(mob: &mut Mob, input: &Nbt) {
        if let Some(Nbt::IntArray(pos)) = input.get("home_pos")
            && let [x, y, z] = pos[..]
        {
            set_transient(mob, BlockPos::new(x, y, z));
        }
    }
}

impl ServerLevel {
    pub fn is_creaking(&self, id: EntityId) -> bool {
        matches!(self.mob(id).map(|mob| &mob.kind), Some(MobKind::Creaking(_)))
    }

    pub fn creaking_can_move(&self, id: EntityId) -> bool {
        can_move(self.entity(id))
    }

    pub fn creaking_set_transient(&mut self, id: EntityId, pos: BlockPos) {
        set_transient(self.mob_mut(id).expect("a creaking"), pos);
    }

    pub fn creaking_do_hurt_target(&mut self, id: EntityId, target: EntityId) -> bool {
        if !self.is_living(target) {
            return false;
        }
        creaking_mut(self.mob_mut(id).expect("a creaking")).attack_animation_remaining_ticks = ATTACK_ANIMATION_DURATION;
        self.broadcast_entity_event(id, 4);
        self.mob_do_hurt_target(id, target)
    }

    pub fn creaking_hurt_server(&mut self, id: EntityId, source: DamageSource, damage: f32) -> bool {
        let Some(home_pos) = get_home_pos(self.entity(id)) else { return self.living_entity_hurt_server(id, source, damage) };
        if source.is(&damage_type_tags::BYPASSES_INVULNERABILITY) {
            return self.living_entity_hurt_server(id, source, damage);
        }
        if !self.is_invulnerable_to(id, &source) && creaking(self.mob(id).expect("a creaking")).invulnerability_animation_remaining_ticks <= 0 && !self.is_dead_or_dying(id) {
            let responsible_player = self.creaking_blame_source_for_damage(id, &source);
            let direct_entity = source.direct_entity;
            if !direct_entity.is_some_and(|entity| self.is_living(entity) || self.is_projectile(entity)) && responsible_player.is_none() {
                return false;
            }
            creaking_mut(self.mob_mut(id).expect("a creaking")).invulnerability_animation_remaining_ticks = INVULNERABILITY_ANIMATION_DURATION;
            self.broadcast_entity_event(id, 66);
            if self.get_block_entity(home_pos).is_some_and(|entity| matches!(entity.kind, BlockEntityKind::CreakingHeart(_))) && self.creaking_heart_is_protector(home_pos, id) {
                if responsible_player.is_some() {
                    self.creaking_heart_creaking_hurt(home_pos);
                }
                self.play_hurt_sound(id, &source);
            }
            true
        } else {
            false
        }
    }

    fn creaking_blame_source_for_damage(&mut self, id: EntityId, source: &DamageSource) -> Option<EntityId> {
        self.resolve_mob_responsible_for_damage(id, source);
        self.resolve_player_responsible_for_damage(id, source)
    }

    pub fn creaking_is_pushable(&mut self, id: EntityId) -> bool {
        self.living_entity_is_pushable(id) && self.creaking_can_move(id)
    }

    pub fn creaking_custom_server_ai_step(&mut self, id: EntityId) {
        Brain::tick(self, id);
        if self.mob(id).is_none() {
            return;
        }
        creaking_ai::update_activity(self, id);
    }

    pub fn creaking_ai_step(&mut self, id: EntityId) {
        let creaking = creaking_mut(self.mob_mut(id).expect("a creaking"));
        if creaking.invulnerability_animation_remaining_ticks > 0 {
            creaking.invulnerability_animation_remaining_ticks -= 1;
        }
        if creaking.attack_animation_remaining_ticks > 0 {
            creaking.attack_animation_remaining_ticks -= 1;
        }
        let can_move = self.creaking_can_move(id);
        let now_can_move = self.creaking_check_can_move(id);
        if now_can_move != can_move {
            if now_can_move {
                self.make_sound(id, sound_event("minecraft:entity.creaking.unfreeze"));
            } else {
                self.stop_in_place(id);
                self.make_sound(id, sound_event("minecraft:entity.creaking.freeze"));
            }
        }
        self.entity_mut(id).data.set(DATA_CAN_MOVE, DataValue::Boolean(now_can_move));
        self.monster_ai_step(id);
    }

    pub fn creaking_tick(&mut self, id: EntityId) {
        if let Some(home_pos) = get_home_pos(self.entity(id)) {
            let has_protection_from_creaking_heart =
                self.get_block_entity(home_pos).is_some_and(|entity| matches!(entity.kind, BlockEntityKind::CreakingHeart(_))) && self.creaking_heart_is_protector(home_pos, id);
            if !has_protection_from_creaking_heart {
                self.set_health(id, 0.0);
            }
        }
        self.mob_tick(id);
    }

    pub fn creaking_tick_death(&mut self, id: EntityId) -> bool {
        let e = self.entity(id);
        if is_heart_bound(e) && is_tearing_down(e) {
            let living = self.living_mut(id);
            living.death_time += 1;
            if living.death_time > TWITCH_DEATH_DURATION && !self.entity(id).is_removed() {
                self.creaking_tear_down(id);
            }
            return true;
        }
        false
    }

    pub fn creaking_tear_down(&mut self, id: EntityId) {
        let box_ = self.entity(id).bounding_box();
        let center = box_.center();
        let spread = (box_.x_size() * 0.3, box_.y_size() * 0.3, box_.z_size() * 0.3);
        let pale_oak_wood = blocks::PALE_OAK_WOOD.default_state();
        self.send_particles("minecraft:block_crumble", |w| w.varint(pale_oak_wood.id() as i32), center.x, center.y, center.z, 100, spread, 0.0);
        let heart = blocks::CREAKING_HEART.default_state().set(CREAKING_HEART_STATE, CreakingHeartState::Awake);
        self.send_particles("minecraft:block_crumble", |w| w.varint(heart.id() as i32), center.x, center.y, center.z, 10, spread, 0.0);
        self.make_sound(id, sound_event(get_death_sound()));
        self.remove(id, RemovalReason::Discarded);
    }

    pub fn creaking_death_effects(&mut self, id: EntityId, source: DamageSource) {
        self.creaking_blame_source_for_damage(id, &source);
        self.die(id, source);
        self.make_sound(id, sound_event("minecraft:entity.creaking.twitch"));
    }

    pub fn creaking_set_tearing_down(&mut self, id: EntityId) {
        self.entity_mut(id).data.set(DATA_IS_TEARING_DOWN, DataValue::Boolean(true));
    }

    pub fn creaking_fire_immune(&self, id: EntityId) -> bool {
        is_heart_bound(self.entity(id))
    }

    pub fn creaking_can_use_portal(&self, id: EntityId) -> bool {
        !is_heart_bound(self.entity(id))
    }

    pub fn creaking_player_is_stuck_in_you(&mut self, id: EntityId) -> bool {
        let players = self.get_brain(id).get_memory(memory_module_type::NEAREST_PLAYERS).cloned().unwrap_or_default();
        if players.is_empty() {
            creaking_mut(self.mob_mut(id).expect("a creaking")).player_stuck_counter = 0;
            return false;
        }
        let own_box = self.entity(id).bounding_box();
        for player in players {
            let eye = self.entity(player).eye_position();
            if own_box.contains(eye.x, eye.y, eye.z) {
                let creaking = creaking_mut(self.mob_mut(id).expect("a creaking"));
                creaking.player_stuck_counter += 1;
                return creaking.player_stuck_counter > MAX_PLAYER_STUCK_COUNTER;
            }
        }
        creaking_mut(self.mob_mut(id).expect("a creaking")).player_stuck_counter = 0;
        false
    }

    pub fn creaking_play_attack_sound(&mut self, id: EntityId) {
        self.make_sound(id, sound_event("minecraft:entity.creaking.attack"));
    }

    pub fn creaking_play_step_sound(&mut self, id: EntityId) {
        self.entity_play_sound(id, sound_event("minecraft:entity.creaking.step"), 0.15, 1.0);
    }

    fn creaking_check_can_move(&mut self, id: EntityId) -> bool {
        let players = self.get_brain(id).get_memory(memory_module_type::NEAREST_PLAYERS).cloned().unwrap_or_default();
        let active = is_active(self.entity(id));
        if players.is_empty() {
            if active {
                self.creaking_deactivate(id);
            }
            return true;
        }
        let mut has_potential_target = false;
        for player in players {
            if self.can_attack(id, player) && !self.is_allied_to(id, player) {
                has_potential_target = true;
                let e = self.entity(id);
                let scale = self.attribute_value(id, crate::world::entity::attributes::Attribute::Scale);
                let gaze_heights = [e.eye_y(), e.y() + 0.5 * scale, (e.eye_y() + e.y()) / 2.0];
                if (!active || self.player_not_wearing_disguise_item(player)) && self.is_looking_at_me(id, player, 0.5, false, true, &gaze_heights) {
                    if active {
                        return false;
                    }
                    if self.entity(player).distance_to_sqr(self.entity(id)) < ACTIVATION_RANGE_SQ {
                        self.creaking_activate(id, player);
                        return false;
                    }
                }
            }
        }
        if !has_potential_target && active {
            self.creaking_deactivate(id);
        }
        true
    }

    fn creaking_activate(&mut self, id: EntityId, player: EntityId) {
        self.get_brain_mut(id).set_memory(memory_module_type::ATTACK_TARGET, player);
        self.make_sound(id, sound_event("minecraft:entity.creaking.activate"));
        self.entity_mut(id).data.set(DATA_IS_ACTIVE, DataValue::Boolean(true));
    }

    fn creaking_deactivate(&mut self, id: EntityId) {
        self.get_brain_mut(id).erase_memory(memory_module_type::ATTACK_TARGET);
        self.make_sound(id, sound_event("minecraft:entity.creaking.deactivate"));
        self.entity_mut(id).data.set(DATA_IS_ACTIVE, DataValue::Boolean(false));
    }

    pub fn creaking_home_node_evaluator_blocks(&self, mob: EntityId, mob_position: BlockPos, x: i32, y: i32, z: i32) -> bool {
        let Some(home_pos) = get_home_pos(self.entity(mob)) else { return false };
        let home_distance = home_pos.dist_sqr(BlockPos::new(x, y, z));
        home_distance > 1024.0 && home_distance >= home_pos.dist_sqr(mob_position)
    }
}
