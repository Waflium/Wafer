use wafer_core::random::RandomSource;
use wafer_core::{BlockPos, Vec3, mth};
use wafer_data::{EquipmentSlot, damage_types, items, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::difficulty_instance::DifficultyInstance;
use crate::world::entity::ai::MobAi;
use crate::world::entity::ai::control::move_control::{MoveControlKind, Operation};
use crate::world::entity::ai::goal::float_goal::FloatGoal;
use crate::world::entity::ai::goal::goal::{Goal, reduced_tick_delay};
use crate::world::entity::ai::goal::look_at_player_goal::LookAtPlayerGoal;
use crate::world::entity::ai::goal::target::hurt_by_target_goal::HurtByTargetGoal;
use crate::world::entity::ai::goal::target::nearest_attackable_target_goal::{NearestAttackableTargetGoal, TargetType};
use crate::world::entity::ai::goal::target::target_goal::TargetGoal;
use crate::world::entity::ai::targeting::targeting_conditions::TargetingConditions;
use crate::world::entity::attributes::VEX_ATTRIBUTES;
use crate::world::entity::data::{DataValue, mob_defaults};
use crate::world::entity::entity_reference::EntityReference;
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::raid::raider::RAIDER_TYPES;
use crate::world::entity::spawn_group_data::SpawnGroupData;
use crate::world::entity::{Entity, EntityDimensions, EntityId, sound_event};
use crate::world::item::ItemStack;
use crate::world::level::BlockGetter;
use crate::world::level::storage::tag_value_input;

pub const DATA_FLAGS_ID: usize = 16;
const FLAG_IS_CHARGING: i8 = 1;
const XP_REWARD: i32 = 3;
pub const TICKS_PER_FLAP: i32 = 4;
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.4, 0.8)
    .with_eye_height(0.51875)
    .with_passenger_attachments(&[Vec3::new(0.0, 0.7375f32 as f64, 0.0)])
    .with_vehicle_attachment(&[Vec3::new(0.0, -0.04f32 as f64, 0.0)]);

#[derive(Default)]
pub struct Vex {
    owner: Option<EntityReference>,
    bound_origin: Option<BlockPos>,
    has_limited_life: bool,
    limited_life_ticks: i32,
}

fn vex(mob: &Mob) -> &Vex {
    match &mob.kind {
        MobKind::Vex(vex) => vex,
        _ => unreachable!("a vex"),
    }
}

fn vex_mut(mob: &mut Mob) -> &mut Vex {
    match &mut mob.kind {
        MobKind::Vex(vex) => vex,
        _ => unreachable!("a vex"),
    }
}

fn get_vex_flag(entity: &Entity, flag: i8) -> bool {
    let flags = match entity.data.get(DATA_FLAGS_ID) {
        DataValue::Byte(flags) => *flags,
        _ => 0,
    };
    flags & flag != 0
}

fn set_vex_flag(entity: &mut Entity, flag: i8, value: bool) {
    let mut flags = match entity.data.get(DATA_FLAGS_ID) {
        DataValue::Byte(flags) => *flags,
        _ => 0,
    };
    if value {
        flags |= flag;
    } else {
        flags &= !flag;
    }
    entity.data.set(DATA_FLAGS_ID, DataValue::Byte(flags));
}

pub fn is_charging(entity: &Entity) -> bool {
    get_vex_flag(entity, FLAG_IS_CHARGING)
}

pub fn set_is_charging(entity: &mut Entity, value: bool) {
    set_vex_flag(entity, FLAG_IS_CHARGING, value);
}

impl Vex {
    pub fn create() -> Mob {
        let mut data = mob_defaults();
        data.push(DataValue::Byte(0));
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:vex"), DIMENSIONS, data);
        let mut mob = Mob::new(entity, LivingEntity::new(&VEX_ATTRIBUTES), MobKind::Vex(Vex::default()));
        mob.ai.move_control.kind = MoveControlKind::Vex;
        mob.xp_reward = XP_REWARD;
        mob
    }

    pub fn register_goals(ai: &mut MobAi) {
        let float_goal = FloatGoal::new(&mut ai.navigation);
        let goal_selector = &mut ai.goal_selector;
        goal_selector.add_goal(0, Goal::Float(float_goal));
        goal_selector.add_goal(4, Goal::VexChargeAttack(VexChargeAttackGoal));
        goal_selector.add_goal(8, Goal::VexRandomMove(VexRandomMoveGoal));
        goal_selector.add_goal(9, Goal::LookAtPlayer(LookAtPlayerGoal::new(3.0).with_probability(1.0)));
        goal_selector.add_goal(10, Goal::LookAtPlayer(LookAtPlayerGoal::of_type(TargetType::Mob, 8.0)));
        let target_selector = &mut ai.target_selector;
        target_selector.add_goal(1, Goal::HurtByTarget(HurtByTargetGoal::new(RAIDER_TYPES).set_alert_others(&[])));
        target_selector.add_goal(2, Goal::VexCopyOwnerTarget(VexCopyOwnerTargetGoal::new()));
        target_selector.add_goal(3, Goal::NearestAttackableTarget(NearestAttackableTargetGoal::new(TargetType::Player, true)));
    }

    pub fn read_additional_save_data(mob: &mut Mob, input: &Nbt) {
        let vex = vex_mut(mob);
        vex.bound_origin = tag_value_input::read(input, "bound_pos", crate::util::extra_codecs::block_pos);
        match tag_value_input::get_i64(input, "life_ticks") {
            Some(life_ticks) => vex.set_limited_life(life_ticks as i32),
            None => vex.has_limited_life = false,
        }
        vex.owner = EntityReference::read(input, "owner");
    }

    pub fn add_additional_save_data(mob: &Mob, output: &mut Vec<(String, Nbt)>) {
        let vex = vex(mob);
        if let Some(bound_origin) = vex.bound_origin {
            output.push(("bound_pos".into(), Nbt::IntArray(vec![bound_origin.x, bound_origin.y, bound_origin.z])));
        }
        if vex.has_limited_life {
            output.push(("life_ticks".into(), Nbt::Int(vex.limited_life_ticks)));
        }
        EntityReference::store(vex.owner, output, "owner");
    }

    pub fn get_owner_reference(mob: &Mob) -> Option<EntityReference> {
        vex(mob).owner
    }

    pub fn get_bound_origin(mob: &Mob) -> Option<BlockPos> {
        vex(mob).bound_origin
    }

    pub fn set_bound_origin(mob: &mut Mob, bound_origin: Option<BlockPos>) {
        vex_mut(mob).bound_origin = bound_origin;
    }

    pub fn set_owner(mob: &mut Mob, owner: u128) {
        vex_mut(mob).owner = Some(EntityReference::of(owner));
    }

    fn set_limited_life(&mut self, life_ticks: i32) {
        self.has_limited_life = true;
        self.limited_life_ticks = life_ticks;
    }

    pub fn set_limited_life_ticks(mob: &mut Mob, life_ticks: i32) {
        vex_mut(mob).set_limited_life(life_ticks);
    }

    pub fn get_ambient_sound() -> &'static str {
        "minecraft:entity.vex.ambient"
    }

    pub fn get_death_sound() -> &'static str {
        "minecraft:entity.vex.death"
    }

    pub fn get_hurt_sound() -> &'static str {
        "minecraft:entity.vex.hurt"
    }

    pub fn get_light_level_dependent_magic_value() -> f32 {
        1.0
    }
}

impl ServerLevel {
    pub fn is_vex(&self, id: EntityId) -> bool {
        matches!(self.mob(id).map(|mob| &mob.kind), Some(MobKind::Vex(_)))
    }

    pub fn vex_is_affected_by_blocks(&self, id: EntityId) -> bool {
        !self.entity(id).is_removed()
    }

    pub fn vex_tick(&mut self, id: EntityId) {
        self.entity_mut(id).no_physics = true;
        self.mob_tick(id);
        let Some(mob) = self.mob_mut(id) else { return };
        mob.entity.no_physics = false;
        mob.entity.set_no_gravity(true);
        let vex = vex_mut(mob);
        if vex.has_limited_life {
            vex.limited_life_ticks -= 1;
            if vex.limited_life_ticks <= 0 {
                vex.limited_life_ticks = 20;
                self.hurt(id, DamageSource::new(damage_types::STARVE), 1.0);
            }
        }
    }

    pub fn vex_finalize_spawn(&mut self, id: EntityId, difficulty: &DifficultyInstance, spawn_reason: EntitySpawnReason, group_data: Option<SpawnGroupData>) -> Option<SpawnGroupData> {
        self.vex_populate_default_equipment_slots(id);
        self.populate_default_equipment_enchantments(id, difficulty);
        self.mob_finalize_spawn(id, difficulty, spawn_reason, group_data)
    }

    pub fn vex_populate_default_equipment_slots(&mut self, id: EntityId) {
        self.living_set_item_slot(id, EquipmentSlot::Mainhand, ItemStack::new(items::IRON_SWORD, 1));
        self.mob_mut(id).expect("a vex").set_drop_chance(EquipmentSlot::Mainhand, 0.0);
    }

    pub fn vex_move_control_tick(&mut self, id: EntityId) {
        let target = self.get_target(id);
        let target_pos = target.map(|target| self.entity(target).position());
        let mob = self.mob_mut(id).expect("a vex");
        let control = &mob.ai.move_control;
        if control.operation != Operation::MoveTo {
            return;
        }
        let e = &mut mob.entity;
        let delta = Vec3::new(control.wanted_x - e.x(), control.wanted_y - e.y(), control.wanted_z - e.z());
        let delta_length = delta.length();
        if delta_length < e.bounding_box().size() {
            mob.ai.move_control.operation = Operation::Wait;
            e.set_delta_movement(e.delta_movement.scale(0.5));
        } else {
            e.set_delta_movement(e.delta_movement.add_vec(delta.scale(control.speed_modifier * 0.05 / delta_length)));
            match target_pos {
                None => {
                    let movement = e.delta_movement;
                    e.set_y_rot(-(mth::atan2(movement.x, movement.z) as f32) * (180.0 / std::f64::consts::PI as f32));
                }
                Some(target_pos) => {
                    let tx = target_pos.x - e.x();
                    let tz = target_pos.z - e.z();
                    e.set_y_rot(-(mth::atan2(tx, tz) as f32) * (180.0 / std::f64::consts::PI as f32));
                }
            }
            let y_rot = e.y_rot;
            mob.living.y_body_rot = y_rot;
        }
    }
}

pub struct VexChargeAttackGoal;

impl VexChargeAttackGoal {
    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        let Some(target) = level.get_target(mob).filter(|&target| level.is_alive(target)) else { return false };
        if level.mob(mob).expect("a vex").ai.move_control.has_wanted() || level.entity_mut(mob).random.next_int_bounded(reduced_tick_delay(7)) != 0 {
            return false;
        }
        let t = level.entity(target);
        level.entity(mob).distance_to_sqr_xyz(t.x(), t.y(), t.z()) > 4.0
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        let m = level.mob(mob).expect("a vex");
        m.ai.move_control.has_wanted() && is_charging(&m.entity) && level.get_target(mob).is_some_and(|target| level.is_alive(target))
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        if let Some(attack_target) = level.get_target(mob) {
            let eye_position = level.entity(attack_target).eye_position();
            level.mob_mut(mob).expect("a vex").ai.move_control.set_wanted_position(eye_position.x, eye_position.y, eye_position.z, 1.0);
        }
        set_is_charging(level.entity_mut(mob), true);
        level.entity_play_sound(mob, sound_event("minecraft:entity.vex.charge"), 1.0, 1.0);
    }

    pub fn stop(&mut self, level: &mut ServerLevel, mob: EntityId) {
        set_is_charging(level.entity_mut(mob), false);
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let Some(attack_target) = level.get_target(mob) else { return };
        if level.entity(mob).bounding_box().intersects(&level.entity(attack_target).bounding_box()) {
            level.do_hurt_target(mob, attack_target);
            set_is_charging(level.entity_mut(mob), false);
        } else {
            let t = level.entity(attack_target);
            let distance = level.entity(mob).distance_to_sqr_xyz(t.x(), t.y(), t.z());
            if distance < 9.0 {
                let eye_position = level.entity(attack_target).eye_position();
                level.mob_mut(mob).expect("a vex").ai.move_control.set_wanted_position(eye_position.x, eye_position.y, eye_position.z, 1.0);
            }
        }
    }
}

pub struct VexCopyOwnerTargetGoal {
    pub target_goal: TargetGoal,
}

impl VexCopyOwnerTargetGoal {
    fn new() -> VexCopyOwnerTargetGoal {
        VexCopyOwnerTargetGoal { target_goal: TargetGoal::new(false, false) }
    }

    fn owner_target(level: &ServerLevel, mob: EntityId) -> Option<EntityId> {
        level.get_owner(mob).filter(|&owner| level.is_mob(owner)).and_then(|owner| level.get_target(owner))
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        let copy_owner_targeting = TargetingConditions::for_non_combat().ignore_line_of_sight().ignore_invisibility_testing();
        let owner_target = Self::owner_target(level, mob);
        owner_target.is_some() && self.target_goal.can_attack(level, mob, owner_target, &copy_owner_targeting)
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let owner_target = Self::owner_target(level, mob);
        level.set_target(mob, owner_target);
        self.target_goal.start();
    }
}

pub struct VexRandomMoveGoal;

impl VexRandomMoveGoal {
    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        !level.mob(mob).expect("a vex").ai.move_control.has_wanted() && level.entity_mut(mob).random.next_int_bounded(reduced_tick_delay(7)) == 0
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let bound_origin = Vex::get_bound_origin(level.mob(mob).expect("a vex")).unwrap_or_else(|| level.entity(mob).block_position());
        for _ in 0..3 {
            let random = &mut level.entity_mut(mob).random;
            let dx = random.next_int_bounded(15) - 7;
            let dy = random.next_int_bounded(11) - 5;
            let dz = random.next_int_bounded(15) - 7;
            let test_pos = bound_origin.offset(dx, dy, dz);
            if level.block_state(test_pos).is_air() {
                let (x, y, z) = (test_pos.x as f64 + 0.5, test_pos.y as f64 + 0.5, test_pos.z as f64 + 0.5);
                level.mob_mut(mob).expect("a vex").ai.move_control.set_wanted_position(x, y, z, 0.25);
                if level.get_target(mob).is_none() {
                    level.mob_mut(mob).expect("a vex").ai.look_control.set_look_at(x, y, z, 180.0, 20.0);
                }
                break;
            }
        }
    }
}
