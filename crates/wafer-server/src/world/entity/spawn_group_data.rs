use super::ageable_mob::AgeableMobGroupData;
use super::monster::zombie::husk::HuskGroupData;
use super::monster::zombie::zombie::ZombieGroupData;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum SpawnGroupData {
    AgeableMob(AgeableMobGroupData),
    Zombie(ZombieGroupData),
    Husk(HuskGroupData),
}
