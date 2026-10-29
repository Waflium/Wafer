use wafer_core::random::RandomSource;

use super::abstract_fish::AbstractFish;
use crate::server::level::ServerLevel;
use crate::world::difficulty_instance::DifficultyInstance;
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::EntityId;
use crate::world::entity::ai::MobAi;
use crate::world::entity::ai::goal::follow_flock_leader_goal::FollowFlockLeaderGoal;
use crate::world::entity::ai::goal::goal::Goal;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::spawn_group_data::SpawnGroupData;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct SchoolSpawnGroupData {
    pub leader: EntityId,
}

pub struct AbstractSchoolingFish {
    pub leader: Option<EntityId>,
    pub school_size: i32,
}

impl Default for AbstractSchoolingFish {
    fn default() -> AbstractSchoolingFish {
        AbstractSchoolingFish { leader: None, school_size: 1 }
    }
}

impl AbstractSchoolingFish {
    pub fn register_goals(ai: &mut MobAi, random: &mut impl RandomSource) {
        AbstractFish::register_goals(ai);
        ai.goal_selector.add_goal(5, Goal::FollowFlockLeader(FollowFlockLeaderGoal::new(random)));
    }
}

impl Mob {
    pub fn abstract_schooling_fish(&self) -> Option<&AbstractSchoolingFish> {
        match &self.kind {
            MobKind::Cod(cod) => Some(&cod.abstract_schooling_fish),
            MobKind::Salmon(salmon) => Some(&salmon.abstract_schooling_fish),
            MobKind::TropicalFish(tropical_fish) => Some(&tropical_fish.abstract_schooling_fish),
            _ => None,
        }
    }

    pub fn abstract_schooling_fish_mut(&mut self) -> Option<&mut AbstractSchoolingFish> {
        match &mut self.kind {
            MobKind::Cod(cod) => Some(&mut cod.abstract_schooling_fish),
            MobKind::Salmon(salmon) => Some(&mut salmon.abstract_schooling_fish),
            MobKind::TropicalFish(tropical_fish) => Some(&mut tropical_fish.abstract_schooling_fish),
            _ => None,
        }
    }

    pub fn get_max_school_size(&self) -> i32 {
        match self.kind {
            MobKind::Salmon(_) => super::salmon::Salmon::get_max_school_size(),
            _ => self.abstract_fish_get_max_spawn_cluster_size(),
        }
    }

    pub fn abstract_schooling_fish_get_max_spawn_cluster_size(&self) -> i32 {
        self.get_max_school_size()
    }
}

impl ServerLevel {
    pub fn is_abstract_schooling_fish(&self, id: EntityId) -> bool {
        self.mob(id).is_some_and(|mob| mob.abstract_schooling_fish().is_some())
    }

    fn schooling_fish(&self, id: EntityId) -> &AbstractSchoolingFish {
        self.mob(id).and_then(Mob::abstract_schooling_fish).expect("a schooling fish")
    }

    fn schooling_fish_mut(&mut self, id: EntityId) -> &mut AbstractSchoolingFish {
        self.mob_mut(id).and_then(Mob::abstract_schooling_fish_mut).expect("a schooling fish")
    }

    pub fn abstract_schooling_fish_can_random_swim(&self, id: EntityId) -> bool {
        !self.is_follower(id)
    }

    pub fn is_follower(&self, id: EntityId) -> bool {
        self.schooling_fish(id).leader.is_some_and(|leader| self.is_alive(leader))
    }

    pub fn start_following(&mut self, id: EntityId, leader: EntityId) -> EntityId {
        self.schooling_fish_mut(id).leader = Some(leader);
        self.add_follower(leader);
        leader
    }

    pub fn stop_following(&mut self, id: EntityId) {
        if let Some(leader) = self.schooling_fish(id).leader {
            self.remove_follower(leader);
        }
        self.schooling_fish_mut(id).leader = None;
    }

    fn add_follower(&mut self, id: EntityId) {
        if let Some(fish) = self.mob_mut(id).and_then(Mob::abstract_schooling_fish_mut) {
            fish.school_size += 1;
        }
    }

    fn remove_follower(&mut self, id: EntityId) {
        if let Some(fish) = self.mob_mut(id).and_then(Mob::abstract_schooling_fish_mut) {
            fish.school_size -= 1;
        }
    }

    pub fn can_be_followed(&self, id: EntityId) -> bool {
        self.has_followers(id) && self.schooling_fish(id).school_size < self.mob(id).expect("a schooling fish").get_max_school_size()
    }

    pub fn abstract_schooling_fish_tick(&mut self, id: EntityId) {
        self.mob_tick(id);
        self.then_tick(move |level| level.abstract_schooling_fish_tick_end(id));
    }

    fn abstract_schooling_fish_tick_end(&mut self, id: EntityId) {
        if self.has_followers(id) && self.random.next_int_bounded(200) == 1 {
            let area = self.entity(id).bounding_box().inflate(8.0, 8.0, 8.0);
            let neighbors = self.get_entities_of_class_same_kind(id, &area);
            if neighbors.len() <= 1 {
                self.schooling_fish_mut(id).school_size = 1;
            }
        }
    }

    pub fn has_followers(&self, id: EntityId) -> bool {
        self.schooling_fish(id).school_size > 1
    }

    pub fn in_range_of_leader(&self, id: EntityId) -> bool {
        let leader = self.schooling_fish(id).leader.expect("a leader");
        self.entity(id).distance_to_sqr(self.entity(leader)) <= 121.0
    }

    pub fn path_to_leader(&mut self, id: EntityId) {
        if self.is_follower(id) {
            let leader = self.schooling_fish(id).leader.expect("a leader");
            self.get_navigation(id, |navigation, level| navigation.move_to_entity(level, id, leader, 1.0));
        }
    }

    pub fn add_followers(&mut self, id: EntityId, abstract_schooling_fish_stream: Vec<EntityId>) {
        let limit = (self.mob(id).expect("a schooling fish").get_max_school_size() - self.schooling_fish(id).school_size).max(0) as usize;
        for other_fish in abstract_schooling_fish_stream.into_iter().take(limit).filter(|&fish| fish != id) {
            self.start_following(other_fish, id);
        }
    }

    pub fn abstract_schooling_fish_finalize_spawn(
        &mut self,
        id: EntityId,
        difficulty: &DifficultyInstance,
        spawn_reason: EntitySpawnReason,
        group_data: Option<SpawnGroupData>,
    ) -> Option<SpawnGroupData> {
        self.mob_finalize_spawn(id, difficulty, spawn_reason, group_data);
        match group_data {
            None => Some(SpawnGroupData::School(SchoolSpawnGroupData { leader: id })),
            Some(SpawnGroupData::School(school)) => {
                self.start_following(id, school.leader);
                group_data
            }
            Some(SpawnGroupData::TropicalFish(tropical_fish)) => {
                self.start_following(id, tropical_fish.school.leader);
                group_data
            }
            Some(_) => group_data,
        }
    }
}
