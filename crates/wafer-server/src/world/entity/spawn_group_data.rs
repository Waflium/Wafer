use super::ageable_mob::AgeableMobGroupData;
use super::animal::axolotl::axolotl::AxolotlGroupData;
use super::animal::equine::horse::HorseGroupData;
use super::animal::fox::fox::FoxGroupData;
use super::animal::rabbit::rabbit::RabbitGroupData;
use super::animal::wolf::wolf::WolfPackData;
use super::animal::fish::abstract_schooling_fish::SchoolSpawnGroupData;
use super::animal::fish::tropical_fish::TropicalFishGroupData;
use super::monster::spider::spider::SpiderEffectsGroupData;
use super::monster::zombie::husk::HuskGroupData;
use super::monster::zombie::zombie::ZombieGroupData;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum SpawnGroupData {
    AgeableMob(AgeableMobGroupData),
    Zombie(ZombieGroupData),
    Husk(HuskGroupData),
    SpiderEffects(SpiderEffectsGroupData),
    WolfPack(WolfPackData),
    School(SchoolSpawnGroupData),
    TropicalFish(TropicalFishGroupData),
    Horse(HorseGroupData),
    Axolotl(AxolotlGroupData),
    Fox(FoxGroupData),
    Rabbit(RabbitGroupData),
}

impl SpawnGroupData {
    pub fn as_ageable_mob_group_data_mut(&mut self) -> Option<&mut AgeableMobGroupData> {
        match self {
            SpawnGroupData::AgeableMob(data) => Some(data),
            SpawnGroupData::WolfPack(data) => Some(&mut data.ageable_mob_group_data),
            SpawnGroupData::Horse(data) => Some(&mut data.ageable_mob_group_data),
            SpawnGroupData::Axolotl(data) => Some(&mut data.ageable_mob_group_data),
            SpawnGroupData::Fox(data) => Some(&mut data.ageable_mob_group_data),
            SpawnGroupData::Rabbit(data) => Some(&mut data.ageable_mob_group_data),
            _ => None,
        }
    }
}
