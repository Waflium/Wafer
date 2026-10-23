use wafer_core::random::RandomSource;
use wafer_core::{BlockPos, Vec3};
use wafer_data::EquipmentSlot;
use wafer_protocol::nbt::Nbt;

use crate::server::level::ServerLevel;
use crate::world::difficulty_instance::DifficultyInstance;
use crate::world::entity::EntityId;
use crate::world::entity::ai::MobAi;
use crate::world::entity::ai::goal::goal::Goal;
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::entity_type::EntityType;
use crate::world::entity::monster::monster;
use crate::world::entity::mob::{self, Mob};
use crate::world::entity::raid::raid;
use crate::world::entity::spawn_group_data::SpawnGroupData;
use crate::world::level::chunk::HeightmapType;
use crate::world::level::light_layer::LightLayer;
use crate::world::level::server_level_accessor::ServerLevelAccessor;
use crate::world::level::storage::tag_value_input;

const DEFAULT_PATROL_LEADER: bool = false;
const DEFAULT_PATROLLING: bool = false;
const NAVIGATION_FAILED_COOLDOWN: i64 = 200;

pub struct PatrollingMonster {
    patrol_target: Option<BlockPos>,
    patrol_leader: bool,
    patrolling: bool,
}

impl Default for PatrollingMonster {
    fn default() -> PatrollingMonster {
        PatrollingMonster { patrol_target: None, patrol_leader: DEFAULT_PATROL_LEADER, patrolling: DEFAULT_PATROLLING }
    }
}

impl PatrollingMonster {
    pub fn register_goals(ai: &mut MobAi) {
        ai.goal_selector.add_goal(4, Goal::LongDistancePatrol(LongDistancePatrolGoal::new(0.7, 0.595)));
    }

    pub fn add_additional_save_data(&self, out: &mut Vec<(String, Nbt)>) {
        if let Some(patrol_target) = self.patrol_target {
            out.push(("patrol_target".into(), Nbt::IntArray(vec![patrol_target.x, patrol_target.y, patrol_target.z])));
        }
        out.push(("PatrolLeader".into(), Nbt::Byte(self.patrol_leader as i8)));
        out.push(("Patrolling".into(), Nbt::Byte(self.patrolling as i8)));
    }

    pub fn read_additional_save_data(&mut self, input: &Nbt) {
        self.patrol_target = tag_value_input::read(input, "patrol_target", crate::util::extra_codecs::block_pos);
        let boolean = |key: &str, default: bool| tag_value_input::get_i64(input, key).map_or(default, |v| v != 0);
        self.patrol_leader = boolean("PatrolLeader", DEFAULT_PATROL_LEADER);
        self.patrolling = boolean("Patrolling", DEFAULT_PATROLLING);
    }

    pub fn remove_when_far_away(&self, dist_sqr: f64) -> bool {
        !self.patrolling || dist_sqr > 16384.0
    }

    pub fn set_patrol_target(&mut self, target: BlockPos) {
        self.patrol_target = Some(target);
        self.patrolling = true;
    }

    pub fn get_patrol_target(&self) -> Option<BlockPos> {
        self.patrol_target
    }

    pub fn has_patrol_target(&self) -> bool {
        self.patrol_target.is_some()
    }

    pub fn set_patrol_leader(&mut self, is_leader: bool) {
        self.patrol_leader = is_leader;
        self.patrolling = true;
    }

    pub fn is_patrol_leader(&self) -> bool {
        self.patrol_leader
    }

    pub fn is_patrolling(&self) -> bool {
        self.patrolling
    }

    pub fn set_patrolling(&mut self, value: bool) {
        self.patrolling = value;
    }
}

pub fn check_patrolling_monster_spawn_rules(entity_type: &EntityType, level: &mut (impl ServerLevelAccessor + ?Sized), spawn_reason: EntitySpawnReason, pos: BlockPos) -> bool {
    if level.get_brightness(LightLayer::Block, pos) > 8 { false } else { monster::check_any_light_monster_spawn_rules(entity_type, level, spawn_reason, pos) }
}

pub fn finalize_spawn(
    level: &mut (impl ServerLevelAccessor + ?Sized),
    mob: &mut Mob,
    difficulty: &DifficultyInstance,
    spawn_reason: EntitySpawnReason,
    group_data: Option<SpawnGroupData>,
) -> Option<SpawnGroupData> {
    if spawn_reason != EntitySpawnReason::Patrol
        && spawn_reason != EntitySpawnReason::Event
        && spawn_reason != EntitySpawnReason::Structure
        && level.get_random().next_float() < 0.06
        && mob.can_be_leader()
    {
        mob.patrolling_monster_mut().expect("a patrolling monster").patrol_leader = true;
    }
    if mob.patrolling_monster().expect("a patrolling monster").is_patrol_leader() {
        mob.equipment[EquipmentSlot::Head as usize] = Some(raid::get_ominous_banner_instance());
        mob.set_drop_chance(EquipmentSlot::Head, 2.0);
    }
    if spawn_reason == EntitySpawnReason::Patrol {
        mob.patrolling_monster_mut().expect("a patrolling monster").patrolling = true;
    }
    mob::mob_finalize_spawn(level, mob, difficulty, spawn_reason, group_data)
}

impl ServerLevel {
    pub fn can_join_patrol(&self, id: EntityId) -> bool {
        if self.mob(id).and_then(Mob::raider).is_some() { self.raider_can_join_patrol(id) } else { true }
    }

    pub fn find_patrol_target(&mut self, id: EntityId) {
        let mob = self.mob_mut(id).expect("a patrolling monster");
        let block_position = mob.entity.block_position();
        let x = -500 + mob.entity.random.next_int_bounded(1000);
        let z = -500 + mob.entity.random.next_int_bounded(1000);
        let patrolling_monster = mob.patrolling_monster_mut().expect("a patrolling monster");
        patrolling_monster.patrol_target = Some(block_position.offset(x, 0, z));
        patrolling_monster.patrolling = true;
    }

    fn patrolling_monster(&self, id: EntityId) -> &PatrollingMonster {
        self.mob(id).and_then(Mob::patrolling_monster).expect("a patrolling monster")
    }
}

pub struct LongDistancePatrolGoal {
    speed_modifier: f64,
    leader_speed_modifier: f64,
    cooldown_until: i64,
}

impl LongDistancePatrolGoal {
    pub fn new(speed_modifier: f64, leader_speed_modifier: f64) -> LongDistancePatrolGoal {
        LongDistancePatrolGoal { speed_modifier, leader_speed_modifier, cooldown_until: -1 }
    }

    pub fn can_use(&self, level: &ServerLevel, mob: EntityId) -> bool {
        let is_on_cooldown = level.game_time < self.cooldown_until;
        let patrolling_monster = level.patrolling_monster(mob);
        patrolling_monster.is_patrolling()
            && level.get_target(mob).is_none()
            && !level.has_controlling_passenger(mob)
            && patrolling_monster.has_patrol_target()
            && !is_on_cooldown
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let patrol_leader = level.patrolling_monster(mob).is_patrol_leader();
        if !level.mob(mob).expect("a mob").ai.navigation.is_done() {
            return;
        }
        let companions = self.find_patrol_companions(level, mob);
        let position = level.entity(mob).position();
        if level.patrolling_monster(mob).is_patrolling() && companions.is_empty() {
            level.mob_mut(mob).and_then(Mob::patrolling_monster_mut).expect("a patrolling monster").set_patrolling(false);
        } else if patrol_leader && level.patrolling_monster(mob).get_patrol_target().expect("a patrol target").closer_to_center_than(position, 10.0) {
            level.find_patrol_target(mob);
        } else {
            let long_distance_target = Vec3::at_bottom_center_of(level.patrolling_monster(mob).get_patrol_target().expect("a patrol target"));
            let self_vector = position;
            let distance = self_vector.subtract(long_distance_target);
            let long_distance_target = distance.y_rot(90.0).scale(0.4).add_vec(long_distance_target);
            let move_target = long_distance_target.subtract(self_vector).normalize().scale(10.0).add_vec(self_vector);
            let path_target = BlockPos::containing(move_target.x, move_target.y, move_target.z);
            let path_target = level.get_heightmap_pos(HeightmapType::MotionBlockingNoLeaves, path_target);
            let speed_modifier = if patrol_leader { self.leader_speed_modifier } else { self.speed_modifier };
            let moved = level.get_navigation(mob, |navigation, level| navigation.move_to(level, mob, path_target.x as f64, path_target.y as f64, path_target.z as f64, speed_modifier));
            if !moved {
                self.move_randomly(level, mob);
                self.cooldown_until = level.game_time + NAVIGATION_FAILED_COOLDOWN;
            } else if patrol_leader {
                for companion in companions {
                    level.mob_mut(companion).and_then(Mob::patrolling_monster_mut).expect("a patrolling monster").set_patrol_target(path_target);
                }
            }
        }
    }

    fn find_patrol_companions(&self, level: &ServerLevel, mob: EntityId) -> Vec<EntityId> {
        let area = level.entity(mob).bounding_box().inflate(16.0, 16.0, 16.0);
        level
            .get_entities(None, &area)
            .into_iter()
            .filter(|&entity| level.mob(entity).is_some_and(|other| other.patrolling_monster().is_some()) && level.can_join_patrol(entity) && entity != mob)
            .collect()
    }

    fn move_randomly(&self, level: &mut ServerLevel, mob: EntityId) -> bool {
        let m = level.mob_mut(mob).expect("a mob");
        let block_position = m.entity.block_position();
        let x = -8 + m.entity.random.next_int_bounded(16);
        let z = -8 + m.entity.random.next_int_bounded(16);
        let path_target = level.get_heightmap_pos(HeightmapType::MotionBlockingNoLeaves, block_position.offset(x, 0, z));
        let speed_modifier = self.speed_modifier;
        level.get_navigation(mob, |navigation, level| navigation.move_to(level, mob, path_target.x as f64, path_target.y as f64, path_target.z as f64, speed_modifier))
    }
}
