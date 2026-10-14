use super::monster::zombie::husk::HuskGroupData;
use super::monster::zombie::zombie::ZombieGroupData;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SpawnGroupData {
    Zombie(ZombieGroupData),
    Husk(HuskGroupData),
}
