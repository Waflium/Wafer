use wafer_core::Vec3;
use wafer_core::random::RandomSource;
use wafer_data::{EquipmentSlot, items, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use super::abstract_illager::RaiderOpenDoorGoal;
use crate::server::level::ServerLevel;
use crate::world::difficulty::Difficulty;
use crate::world::difficulty_instance::DifficultyInstance;
use crate::world::entity::ai::MobAi;
use crate::world::entity::ai::goal::avoid_entity_goal::AvoidEntityGoal;
use crate::world::entity::ai::goal::break_door_goal::BreakDoorGoal;
use crate::world::entity::ai::goal::float_goal::FloatGoal;
use crate::world::entity::ai::goal::goal::{Goal, reduced_tick_delay};
use crate::world::entity::ai::goal::look_at_player_goal::LookAtPlayerGoal;
use crate::world::entity::ai::goal::melee_attack_goal::MeleeAttackGoal;
use crate::world::entity::ai::goal::random_stroll_goal::RandomStrollGoal;
use crate::world::entity::ai::goal::target::hurt_by_target_goal::HurtByTargetGoal;
use crate::world::entity::ai::goal::target::nearest_attackable_target_goal::{NearestAttackableTargetGoal, TargetType};
use crate::world::entity::attributes::VINDICATOR_ATTRIBUTES;
use crate::world::entity::data::{DataValue, mob_defaults};
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::monster::monster;
use crate::world::entity::raid::raider::{self, HoldGroundAttackGoal, RAIDER_TYPES, Raider};
use crate::world::entity::spawn_group_data::SpawnGroupData;
use crate::world::entity::{Entity, EntityDimensions, EntityId, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::level::storage::tag_value_input;

const TAG_JOHNNY: &str = "Johnny";
const DEFAULT_JOHNNY: bool = false;
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.6, 1.95)
    .with_passenger_attachments(&[Vec3::new(0.0, 2.0, 0.0)])
    .with_vehicle_attachment(&[Vec3::new(0.0, 0.6f32 as f64, 0.0)]);

fn door_breaking_predicate(difficulty: Difficulty) -> bool {
    difficulty == Difficulty::Normal || difficulty == Difficulty::Hard
}

#[derive(Default)]
pub struct Vindicator {
    pub raider: Raider,
    is_johnny: bool,
}

fn vindicator(mob: &Mob) -> &Vindicator {
    match &mob.kind {
        MobKind::Vindicator(vindicator) => vindicator,
        _ => unreachable!("a vindicator"),
    }
}

fn vindicator_mut(mob: &mut Mob) -> &mut Vindicator {
    match &mut mob.kind {
        MobKind::Vindicator(vindicator) => vindicator,
        _ => unreachable!("a vindicator"),
    }
}

fn attackable_selector(_mob: EntityId, target: EntityId, level: &ServerLevel) -> bool {
    level.attackable(target)
}

impl Vindicator {
    pub fn create() -> Mob {
        let mut data = mob_defaults();
        data.push(DataValue::Boolean(false));
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:vindicator"), DIMENSIONS, data);
        let mut mob = Mob::new(entity, LivingEntity::new(&VINDICATOR_ATTRIBUTES), MobKind::Vindicator(Vindicator { raider: Raider::default(), is_johnny: DEFAULT_JOHNNY }));
        mob.xp_reward = monster::XP_REWARD;
        mob
    }

    pub fn register_goals(ai: &mut MobAi) {
        Raider::register_goals(ai);
        let float_goal = FloatGoal::new(&mut ai.navigation);
        let goal_selector = &mut ai.goal_selector;
        goal_selector.add_goal(0, Goal::Float(float_goal));
        goal_selector.add_goal(1, Goal::AvoidEntity(AvoidEntityGoal::new("minecraft:creaking", 8.0, 1.0, 1.2)));
        goal_selector.add_goal(2, Goal::VindicatorBreakDoor(VindicatorBreakDoorGoal::new()));
        goal_selector.add_goal(3, Goal::RaiderOpenDoor(RaiderOpenDoorGoal::new()));
        goal_selector.add_goal(4, Goal::HoldGroundAttack(HoldGroundAttackGoal::new(10.0)));
        goal_selector.add_goal(5, Goal::MeleeAttack(MeleeAttackGoal::new(1.0, false)));
        let target_selector = &mut ai.target_selector;
        target_selector.add_goal(1, Goal::HurtByTarget(HurtByTargetGoal::new(RAIDER_TYPES).set_alert_others(&[])));
        target_selector.add_goal(2, Goal::NearestAttackableTarget(NearestAttackableTargetGoal::new(TargetType::Player, true)));
        target_selector.add_goal(3, Goal::NearestAttackableTarget(NearestAttackableTargetGoal::new(TargetType::AbstractVillager, true)));
        target_selector.add_goal(3, Goal::NearestAttackableTarget(NearestAttackableTargetGoal::new(TargetType::IronGolem, true)));
        target_selector.add_goal(4, Goal::VindicatorJohnnyAttack(VindicatorJohnnyAttackGoal::new()));
        let goal_selector = &mut ai.goal_selector;
        goal_selector.add_goal(8, Goal::RandomStroll(RandomStrollGoal::new(0.6)));
        goal_selector.add_goal(9, Goal::LookAtPlayer(LookAtPlayerGoal::new(3.0).with_probability(1.0)));
        goal_selector.add_goal(10, Goal::LookAtPlayer(LookAtPlayerGoal::of_type(TargetType::Mob, 8.0)));
    }

    pub fn add_additional_save_data(mob: &Mob, out: &mut Vec<(String, Nbt)>) {
        let vindicator = vindicator(mob);
        vindicator.raider.add_additional_save_data(out);
        if vindicator.is_johnny {
            out.push((TAG_JOHNNY.into(), Nbt::Byte(1)));
        }
    }

    pub fn read_additional_save_data(mob: &mut Mob, input: &Nbt) {
        let vindicator = vindicator_mut(mob);
        vindicator.raider.read_additional_save_data(input);
        vindicator.is_johnny = tag_value_input::get_i64(input, TAG_JOHNNY).map_or(DEFAULT_JOHNNY, |v| v != 0);
    }

    pub fn on_custom_name_set(mob: &mut Mob, name: Option<&Nbt>) {
        let is_johnny = vindicator(mob).is_johnny;
        if !is_johnny
            && let Some(name) = name
            && crate::network::chat::component_serialization::decode(name).is_ok_and(|name| name.get_string() == TAG_JOHNNY)
        {
            vindicator_mut(mob).is_johnny = true;
        }
    }

    pub fn get_ambient_sound() -> &'static str {
        "minecraft:entity.vindicator.ambient"
    }

    pub fn get_hurt_sound() -> &'static str {
        "minecraft:entity.vindicator.hurt"
    }

    pub fn get_death_sound() -> &'static str {
        "minecraft:entity.vindicator.death"
    }

    pub fn get_celebrate_sound() -> &'static str {
        "minecraft:entity.vindicator.celebrate"
    }
}

impl ServerLevel {
    pub fn vindicator_custom_server_ai_step(&mut self, id: EntityId) {
        let pos = self.entity(id).block_position();
        let can_open_doors = self.get_raid_at(pos).is_some();
        let mob = self.mob_mut(id).expect("a vindicator");
        if !mob.is_no_ai() && mob.ai.navigation.can_navigate_ground() {
            mob.ai.navigation.set_can_open_doors(can_open_doors);
        }
    }

    pub fn vindicator_finalize_spawn(&mut self, id: EntityId, difficulty: &DifficultyInstance, spawn_reason: EntitySpawnReason, group_data: Option<SpawnGroupData>) -> Option<SpawnGroupData> {
        let (index, _, entity) = self.entities.shift_remove_full(&id).expect("the vindicator to finalize");
        let LevelEntity::Mob(mut mob) = entity else { unreachable!("a mob") };
        let spawn_group_data = raider::finalize_spawn(self, &mut mob, difficulty, spawn_reason, group_data);
        mob.ai.navigation.set_can_open_doors(true);
        self.entities.shift_insert(index, id, LevelEntity::Mob(mob));
        self.vindicator_populate_default_equipment_slots(id);
        self.populate_default_equipment_enchantments(id, difficulty);
        spawn_group_data
    }

    pub fn vindicator_populate_default_equipment_slots(&mut self, id: EntityId) {
        if self.raider(id).get_current_raid_id().is_none() {
            self.living_set_item_slot(id, EquipmentSlot::Mainhand, ItemStack::new(items::IRON_AXE, 1));
        }
    }
}

pub struct VindicatorBreakDoorGoal {
    pub break_door_goal: BreakDoorGoal,
}

impl VindicatorBreakDoorGoal {
    fn new() -> VindicatorBreakDoorGoal {
        VindicatorBreakDoorGoal { break_door_goal: BreakDoorGoal::with_seconds(6, door_breaking_predicate) }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        level.raider_has_active_raid(mob) && level.entity_mut(mob).random.next_int_bounded(reduced_tick_delay(10)) == 0 && self.break_door_goal.can_use(level, mob)
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        level.raider_has_active_raid(mob) && self.break_door_goal.can_continue_to_use(level, mob)
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.break_door_goal.start(level, mob);
        level.living_mut(mob).no_action_time = 0;
    }
}

pub struct VindicatorJohnnyAttackGoal {
    pub nearest_attackable_target_goal: NearestAttackableTargetGoal,
}

impl VindicatorJohnnyAttackGoal {
    fn new() -> VindicatorJohnnyAttackGoal {
        VindicatorJohnnyAttackGoal { nearest_attackable_target_goal: NearestAttackableTargetGoal::with(TargetType::LivingEntity, 0, true, true, Some(attackable_selector)) }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        vindicator(level.mob(mob).expect("a vindicator")).is_johnny && self.nearest_attackable_target_goal.can_use(level, mob)
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.nearest_attackable_target_goal.start(level, mob);
        level.living_mut(mob).no_action_time = 0;
    }
}
