use super::avoid_entity_goal::AvoidEntityGoal;
use super::break_door_goal::BreakDoorGoal;
use super::breed_goal::BreedGoal;
use super::eat_block_goal::EatBlockGoal;
use super::flee_sun_goal::FleeSunGoal;
use super::float_goal::FloatGoal;
use super::follow_parent_goal::FollowParentGoal;
use super::look_at_player_goal::LookAtPlayerGoal;
use super::melee_attack_goal::MeleeAttackGoal;
use super::panic_goal::PanicGoal;
use super::random_look_around_goal::RandomLookAroundGoal;
use super::ranged_bow_attack_goal::RangedBowAttackGoal;
use super::random_stroll_goal::RandomStrollGoal;
use super::restrict_sun_goal::RestrictSunGoal;
use super::swell_goal::SwellGoal;
use super::target::hurt_by_target_goal::HurtByTargetGoal;
use super::target::nearest_attackable_target_goal::NearestAttackableTargetGoal;
use super::tempt_goal::TemptGoal;
use super::water_avoiding_random_stroll_goal::WaterAvoidingRandomStrollGoal;
use super::zombie_attack_goal::ZombieAttackGoal;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::monster::skeleton::abstract_skeleton::AbstractSkeletonMeleeGoal;
use crate::world::entity::monster::zombie::drowned::{DrownedAttackGoal, DrownedGoToBeachGoal, DrownedGoToWaterGoal, DrownedSwimUpGoal, DrownedTridentAttackGoal};
use super::ranged_attack_goal::RangedAttackGoal;
use super::target::nearest_attackable_witch_target_goal::NearestAttackableWitchTargetGoal;
use super::target::nearest_healable_raider_target_goal::NearestHealableRaiderTargetGoal;
use super::beg_goal::BegGoal;
use super::follow_owner_goal::FollowOwnerGoal;
use super::sit_when_ordered_to_goal::SitWhenOrderedToGoal;
use super::target::non_tame_random_target_goal::NonTameRandomTargetGoal;
use super::target::owner_hurt_by_target_goal::OwnerHurtByTargetGoal;
use super::target::owner_hurt_target_goal::OwnerHurtTargetGoal;
use crate::world::entity::animal::wolf::wolf::WolfAvoidEntityGoal;
use crate::world::entity::tamable_animal::TamableAnimalPanicGoal;
use crate::world::entity::monster::patrolling_monster::LongDistancePatrolGoal;
use crate::world::entity::raid::raider::{ObtainRaidLeaderBannerGoal, RaiderCelebration};
use crate::world::entity::monster::zombie::zombie::ZombieAttackTurtleEggGoal;
use super::leap_at_target_goal::LeapAtTargetGoal;
use crate::world::entity::monster::spider::spider::{SpiderAttackGoal, SpiderTargetGoal};
use super::climb_on_top_of_powder_snow_goal::ClimbOnTopOfPowderSnowGoal;
use crate::world::entity::monster::silverfish::{SilverfishMergeWithStoneGoal, SilverfishWakeUpFriendsGoal};
use super::target::reset_universal_anger_target_goal::ResetUniversalAngerTargetGoal;
use crate::world::entity::monster::enderman::{EndermanFreezeWhenLookedAt, EndermanLeaveBlockGoal, EndermanLookForPlayerGoal, EndermanTakeBlockGoal};
use crate::world::entity::monster::cubemob::abstract_cube_mob::{CubeMobAttackGoal, CubeMobFloatGoal, CubeMobKeepOnJumpingGoal, CubeMobRandomDirectionGoal};
use crate::world::entity::animal::squid::squid::{SquidFleeGoal, SquidRandomMovementGoal};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Flag {
    Move,
    Look,
    Jump,
    Target,
}

impl Flag {
    pub const VALUES: [Flag; 4] = [Flag::Move, Flag::Look, Flag::Jump, Flag::Target];

    pub const fn bit(self) -> u8 {
        1 << self as u8
    }
}

pub enum Goal {
    Float(FloatGoal),
    Panic(PanicGoal),
    Breed(BreedGoal),
    Tempt(TemptGoal),
    FollowParent(FollowParentGoal),
    EatBlock(EatBlockGoal),
    WaterAvoidingRandomStroll(WaterAvoidingRandomStrollGoal),
    LookAtPlayer(LookAtPlayerGoal),
    RandomLookAround(RandomLookAroundGoal),
    MeleeAttack(MeleeAttackGoal),
    ZombieAttack(ZombieAttackGoal),
    ZombieAttackTurtleEgg(ZombieAttackTurtleEggGoal),
    BreakDoor(BreakDoorGoal),
    RestrictSun(RestrictSunGoal),
    FleeSun(FleeSunGoal),
    Swell(SwellGoal),
    AvoidEntity(AvoidEntityGoal),
    NearestAttackableTarget(NearestAttackableTargetGoal),
    HurtByTarget(HurtByTargetGoal),
    LeapAtTarget(LeapAtTargetGoal),
    SpiderAttack(SpiderAttackGoal),
    SpiderTarget(SpiderTargetGoal),
    RangedBowAttack(RangedBowAttackGoal),
    AbstractSkeletonMelee(AbstractSkeletonMeleeGoal),
    RandomStroll(RandomStrollGoal),
    DrownedGoToWater(DrownedGoToWaterGoal),
    DrownedTridentAttack(DrownedTridentAttackGoal),
    DrownedAttack(DrownedAttackGoal),
    DrownedGoToBeach(DrownedGoToBeachGoal),
    DrownedSwimUp(DrownedSwimUpGoal),
    ClimbOnTopOfPowderSnow(ClimbOnTopOfPowderSnowGoal),
    SilverfishWakeUpFriends,
    SilverfishMergeWithStone(SilverfishMergeWithStoneGoal),
    ResetUniversalAngerTarget(ResetUniversalAngerTargetGoal),
    EndermanFreezeWhenLookedAt(EndermanFreezeWhenLookedAt),
    EndermanLeaveBlock(EndermanLeaveBlockGoal),
    EndermanLookForPlayer(EndermanLookForPlayerGoal),
    EndermanTakeBlock(EndermanTakeBlockGoal),
    RangedAttack(RangedAttackGoal),
    LongDistancePatrol(LongDistancePatrolGoal),
    ObtainRaidLeaderBanner(ObtainRaidLeaderBannerGoal),
    RaiderCelebration(RaiderCelebration),
    NearestHealableRaiderTarget(NearestHealableRaiderTargetGoal),
    NearestAttackableWitchTarget(NearestAttackableWitchTargetGoal),
    CubeMobAttack(CubeMobAttackGoal),
    CubeMobFloat(CubeMobFloatGoal),
    CubeMobKeepOnJumping(CubeMobKeepOnJumpingGoal),
    CubeMobRandomDirection(CubeMobRandomDirectionGoal),
    TamableAnimalPanic(TamableAnimalPanicGoal),
    SitWhenOrderedTo(SitWhenOrderedToGoal),
    WolfAvoidEntity(WolfAvoidEntityGoal),
    FollowOwner(FollowOwnerGoal),
    Beg(BegGoal),
    OwnerHurtByTarget(OwnerHurtByTargetGoal),
    OwnerHurtTarget(OwnerHurtTargetGoal),
    NonTameRandomTarget(NonTameRandomTargetGoal),
    SquidRandomMovement(SquidRandomMovementGoal),
    SquidFlee(SquidFleeGoal),
}

impl Goal {
    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        match self {
            Goal::Float(goal) => goal.can_use(level, mob),
            Goal::Panic(goal) => goal.can_use(level, mob),
            Goal::Breed(goal) => goal.can_use(level, mob),
            Goal::Tempt(goal) => goal.can_use(level, mob),
            Goal::FollowParent(goal) => goal.can_use(level, mob),
            Goal::EatBlock(goal) => goal.can_use(level, mob),
            Goal::WaterAvoidingRandomStroll(goal) => goal.can_use(level, mob),
            Goal::LookAtPlayer(goal) => goal.can_use(level, mob),
            Goal::RandomLookAround(goal) => goal.can_use(level, mob),
            Goal::MeleeAttack(goal) => goal.can_use(level, mob),
            Goal::ZombieAttack(goal) => goal.melee_attack_goal.can_use(level, mob),
            Goal::ZombieAttackTurtleEgg(goal) => goal.remove_block_goal.can_use(level, mob),
            Goal::BreakDoor(goal) => goal.can_use(level, mob),
            Goal::RestrictSun(goal) => goal.can_use(level, mob),
            Goal::FleeSun(goal) => goal.can_use(level, mob),
            Goal::Swell(goal) => goal.can_use(level, mob),
            Goal::AvoidEntity(goal) => goal.can_use(level, mob),
            Goal::NearestAttackableTarget(goal) => goal.can_use(level, mob),
            Goal::HurtByTarget(goal) => goal.can_use(level, mob),
            Goal::LeapAtTarget(goal) => goal.can_use(level, mob),
            Goal::SpiderAttack(goal) => goal.can_use(level, mob),
            Goal::SpiderTarget(goal) => goal.can_use(level, mob),
            Goal::RangedBowAttack(goal) => goal.can_use(level, mob),
            Goal::AbstractSkeletonMelee(goal) => goal.melee_attack_goal.can_use(level, mob),
            Goal::RandomStroll(goal) => goal.can_use(level, mob, RandomStrollGoal::get_position),
            Goal::DrownedGoToWater(goal) => goal.can_use(level, mob),
            Goal::DrownedTridentAttack(goal) => goal.can_use(level, mob),
            Goal::DrownedAttack(goal) => goal.can_use(level, mob),
            Goal::DrownedGoToBeach(goal) => goal.can_use(level, mob),
            Goal::DrownedSwimUp(goal) => goal.can_use(level, mob),
            Goal::ClimbOnTopOfPowderSnow(goal) => goal.can_use(level, mob),
            Goal::SilverfishWakeUpFriends => SilverfishWakeUpFriendsGoal::can_use(level, mob),
            Goal::SilverfishMergeWithStone(goal) => goal.can_use(level, mob),
            Goal::ResetUniversalAngerTarget(goal) => goal.can_use(level, mob),
            Goal::EndermanFreezeWhenLookedAt(goal) => goal.can_use(level, mob),
            Goal::EndermanLeaveBlock(goal) => goal.can_use(level, mob),
            Goal::EndermanLookForPlayer(goal) => goal.can_use(level, mob),
            Goal::EndermanTakeBlock(goal) => goal.can_use(level, mob),
            Goal::RangedAttack(goal) => goal.can_use(level, mob),
            Goal::LongDistancePatrol(goal) => goal.can_use(level, mob),
            Goal::ObtainRaidLeaderBanner(goal) => goal.can_use(level, mob),
            Goal::RaiderCelebration(goal) => goal.can_use(level, mob),
            Goal::NearestHealableRaiderTarget(goal) => goal.can_use(level, mob),
            Goal::NearestAttackableWitchTarget(goal) => goal.can_use(level, mob),
            Goal::CubeMobAttack(goal) => goal.can_use(level, mob),
            Goal::CubeMobFloat(goal) => goal.can_use(level, mob),
            Goal::CubeMobKeepOnJumping(goal) => goal.can_use(level, mob),
            Goal::CubeMobRandomDirection(goal) => goal.can_use(level, mob),
            Goal::TamableAnimalPanic(goal) => goal.panic_goal.can_use(level, mob),
            Goal::SitWhenOrderedTo(goal) => goal.can_use(level, mob),
            Goal::WolfAvoidEntity(goal) => goal.can_use(level, mob),
            Goal::FollowOwner(goal) => goal.can_use(level, mob),
            Goal::Beg(goal) => goal.can_use(level, mob),
            Goal::OwnerHurtByTarget(goal) => goal.can_use(level, mob),
            Goal::OwnerHurtTarget(goal) => goal.can_use(level, mob),
            Goal::NonTameRandomTarget(goal) => goal.can_use(level, mob),
            Goal::SquidRandomMovement(goal) => goal.can_use(),
            Goal::SquidFlee(goal) => goal.can_use(level, mob),
        }
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        match self {
            Goal::Float(goal) => goal.can_use(level, mob),
            Goal::Panic(goal) => goal.can_continue_to_use(level, mob),
            Goal::Breed(goal) => goal.can_continue_to_use(level, mob),
            Goal::Tempt(goal) => goal.can_continue_to_use(level, mob),
            Goal::FollowParent(goal) => goal.can_continue_to_use(level, mob),
            Goal::EatBlock(goal) => goal.can_continue_to_use(),
            Goal::WaterAvoidingRandomStroll(goal) => goal.random_stroll_goal.can_continue_to_use(level, mob),
            Goal::LookAtPlayer(goal) => goal.can_continue_to_use(level, mob),
            Goal::RandomLookAround(goal) => goal.can_continue_to_use(),
            Goal::MeleeAttack(goal) => goal.can_continue_to_use(level, mob),
            Goal::ZombieAttack(goal) => goal.melee_attack_goal.can_continue_to_use(level, mob),
            Goal::ZombieAttackTurtleEgg(goal) => goal.remove_block_goal.can_continue_to_use(level),
            Goal::BreakDoor(goal) => goal.can_continue_to_use(level, mob),
            Goal::RestrictSun(goal) => goal.can_use(level, mob),
            Goal::FleeSun(goal) => goal.can_continue_to_use(level, mob),
            Goal::Swell(goal) => goal.can_use(level, mob),
            Goal::AvoidEntity(goal) => goal.can_continue_to_use(level, mob),
            Goal::NearestAttackableTarget(goal) => goal.target_goal.can_continue_to_use(level, mob),
            Goal::HurtByTarget(goal) => goal.target_goal.can_continue_to_use(level, mob),
            Goal::LeapAtTarget(goal) => goal.can_continue_to_use(level, mob),
            Goal::SpiderAttack(goal) => goal.can_continue_to_use(level, mob),
            Goal::SpiderTarget(goal) => goal.nearest_attackable_target_goal.target_goal.can_continue_to_use(level, mob),
            Goal::RangedBowAttack(goal) => goal.can_continue_to_use(level, mob),
            Goal::AbstractSkeletonMelee(goal) => goal.melee_attack_goal.can_continue_to_use(level, mob),
            Goal::RandomStroll(goal) => goal.can_continue_to_use(level, mob),
            Goal::DrownedGoToWater(goal) => goal.can_continue_to_use(level, mob),
            Goal::DrownedTridentAttack(goal) => goal.ranged_attack_goal.can_continue_to_use(level, mob),
            Goal::DrownedAttack(goal) => goal.can_continue_to_use(level, mob),
            Goal::DrownedGoToBeach(goal) => goal.can_continue_to_use(level),
            Goal::DrownedSwimUp(goal) => goal.can_continue_to_use(level, mob),
            Goal::ClimbOnTopOfPowderSnow(goal) => goal.can_use(level, mob),
            Goal::SilverfishWakeUpFriends => SilverfishWakeUpFriendsGoal::can_use(level, mob),
            Goal::SilverfishMergeWithStone(goal) => goal.can_continue_to_use(level, mob),
            Goal::ResetUniversalAngerTarget(goal) => goal.can_use(level, mob),
            Goal::EndermanFreezeWhenLookedAt(goal) => goal.can_use(level, mob),
            Goal::EndermanLeaveBlock(goal) => goal.can_use(level, mob),
            Goal::EndermanLookForPlayer(goal) => goal.can_continue_to_use(level, mob),
            Goal::EndermanTakeBlock(goal) => goal.can_use(level, mob),
            Goal::RangedAttack(goal) => goal.can_continue_to_use(level, mob),
            Goal::LongDistancePatrol(goal) => goal.can_use(level, mob),
            Goal::ObtainRaidLeaderBanner(goal) => goal.can_continue_to_use(level, mob),
            Goal::RaiderCelebration(goal) => goal.can_use(level, mob),
            Goal::NearestHealableRaiderTarget(goal) => goal.nearest_attackable_target_goal.target_goal.can_continue_to_use(level, mob),
            Goal::NearestAttackableWitchTarget(goal) => goal.nearest_attackable_target_goal.target_goal.can_continue_to_use(level, mob),
            Goal::CubeMobAttack(goal) => goal.can_continue_to_use(level, mob),
            Goal::CubeMobFloat(goal) => goal.can_use(level, mob),
            Goal::CubeMobKeepOnJumping(goal) => goal.can_use(level, mob),
            Goal::CubeMobRandomDirection(goal) => goal.can_use(level, mob),
            Goal::TamableAnimalPanic(goal) => goal.panic_goal.can_continue_to_use(level, mob),
            Goal::SitWhenOrderedTo(goal) => goal.can_continue_to_use(level, mob),
            Goal::WolfAvoidEntity(goal) => goal.avoid_entity_goal.can_continue_to_use(level, mob),
            Goal::FollowOwner(goal) => goal.can_continue_to_use(level, mob),
            Goal::Beg(goal) => goal.can_continue_to_use(level, mob),
            Goal::OwnerHurtByTarget(goal) => goal.target_goal.can_continue_to_use(level, mob),
            Goal::OwnerHurtTarget(goal) => goal.target_goal.can_continue_to_use(level, mob),
            Goal::NonTameRandomTarget(goal) => goal.can_continue_to_use(level, mob),
            Goal::SquidRandomMovement(goal) => goal.can_use(),
            Goal::SquidFlee(goal) => goal.can_use(level, mob),
        }
    }

    pub fn is_interruptable(&self) -> bool {
        true
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        match self {
            Goal::Float(_) | Goal::Breed(_) => {}
            Goal::Panic(goal) => goal.start(level, mob),
            Goal::Tempt(goal) => goal.start(level),
            Goal::FollowParent(goal) => goal.start(),
            Goal::EatBlock(goal) => goal.start(level, mob),
            Goal::WaterAvoidingRandomStroll(goal) => goal.random_stroll_goal.start(level, mob),
            Goal::LookAtPlayer(goal) => goal.start(level, mob),
            Goal::RandomLookAround(goal) => goal.start(level, mob),
            Goal::MeleeAttack(goal) => goal.start(level, mob),
            Goal::ZombieAttack(goal) => goal.start(level, mob),
            Goal::ZombieAttackTurtleEgg(goal) => goal.remove_block_goal.start(level, mob),
            Goal::BreakDoor(goal) => goal.start(level, mob),
            Goal::RestrictSun(goal) => goal.start(level, mob),
            Goal::FleeSun(goal) => goal.start(level, mob),
            Goal::Swell(goal) => goal.start(level, mob),
            Goal::AvoidEntity(goal) => goal.start(level, mob),
            Goal::NearestAttackableTarget(goal) => goal.start(level, mob),
            Goal::HurtByTarget(goal) => goal.start(level, mob),
            Goal::LeapAtTarget(goal) => goal.start(level, mob),
            Goal::SpiderAttack(goal) => goal.melee_attack_goal.start(level, mob),
            Goal::SpiderTarget(goal) => goal.nearest_attackable_target_goal.start(level, mob),
            Goal::RangedBowAttack(goal) => goal.start(level, mob),
            Goal::AbstractSkeletonMelee(goal) => goal.start(level, mob),
            Goal::RandomStroll(goal) => goal.start(level, mob),
            Goal::DrownedGoToWater(goal) => goal.start(level, mob),
            Goal::DrownedTridentAttack(goal) => goal.start(level, mob),
            Goal::DrownedAttack(goal) => goal.zombie_attack_goal.start(level, mob),
            Goal::DrownedGoToBeach(goal) => goal.start(level, mob),
            Goal::DrownedSwimUp(goal) => goal.start(level, mob),
            Goal::ClimbOnTopOfPowderSnow(_) | Goal::SilverfishWakeUpFriends => {}
            Goal::SilverfishMergeWithStone(goal) => goal.start(level, mob),
            Goal::ResetUniversalAngerTarget(goal) => goal.start(level, mob),
            Goal::EndermanFreezeWhenLookedAt(goal) => goal.start(level, mob),
            Goal::EndermanLeaveBlock(_) | Goal::EndermanTakeBlock(_) => {}
            Goal::EndermanLookForPlayer(goal) => goal.start(level, mob),
            Goal::RangedAttack(_) | Goal::LongDistancePatrol(_) => {}
            Goal::ObtainRaidLeaderBanner(goal) => goal.start(level, mob),
            Goal::RaiderCelebration(goal) => goal.start(level, mob),
            Goal::NearestHealableRaiderTarget(goal) => goal.start(level, mob),
            Goal::NearestAttackableWitchTarget(goal) => goal.nearest_attackable_target_goal.start(level, mob),
            Goal::CubeMobAttack(goal) => goal.start(),
            Goal::CubeMobFloat(_) | Goal::CubeMobKeepOnJumping(_) | Goal::CubeMobRandomDirection(_) => {}
            Goal::TamableAnimalPanic(goal) => goal.panic_goal.start(level, mob),
            Goal::SitWhenOrderedTo(goal) => goal.start(level, mob),
            Goal::WolfAvoidEntity(goal) => goal.start(level, mob),
            Goal::FollowOwner(goal) => goal.start(level, mob),
            Goal::Beg(goal) => goal.start(level, mob),
            Goal::OwnerHurtByTarget(goal) => goal.start(level, mob),
            Goal::OwnerHurtTarget(goal) => goal.start(level, mob),
            Goal::NonTameRandomTarget(goal) => goal.nearest_attackable_target_goal.start(level, mob),
            Goal::SquidRandomMovement(_) => {}
            Goal::SquidFlee(goal) => goal.start(),
        }
    }

    pub fn stop(&mut self, level: &mut ServerLevel, mob: EntityId) {
        match self {
            Goal::Float(_) | Goal::RandomLookAround(_) | Goal::FleeSun(_) => {}
            Goal::Panic(goal) => goal.stop(),
            Goal::Breed(goal) => goal.stop(),
            Goal::Tempt(goal) => goal.stop(level, mob),
            Goal::FollowParent(goal) => goal.stop(),
            Goal::EatBlock(goal) => goal.stop(),
            Goal::WaterAvoidingRandomStroll(goal) => goal.random_stroll_goal.stop(level, mob),
            Goal::LookAtPlayer(goal) => goal.stop(),
            Goal::MeleeAttack(goal) => goal.stop(level, mob),
            Goal::ZombieAttack(goal) => goal.stop(level, mob),
            Goal::ZombieAttackTurtleEgg(goal) => goal.remove_block_goal.stop(level, mob),
            Goal::BreakDoor(goal) => goal.stop(level, mob),
            Goal::RestrictSun(goal) => goal.stop(level, mob),
            Goal::Swell(goal) => goal.stop(),
            Goal::AvoidEntity(goal) => goal.stop(),
            Goal::NearestAttackableTarget(goal) => goal.target_goal.stop(level, mob),
            Goal::HurtByTarget(goal) => goal.target_goal.stop(level, mob),
            Goal::LeapAtTarget(_) => {}
            Goal::SpiderAttack(goal) => goal.melee_attack_goal.stop(level, mob),
            Goal::SpiderTarget(goal) => goal.nearest_attackable_target_goal.target_goal.stop(level, mob),
            Goal::RangedBowAttack(goal) => goal.stop(level, mob),
            Goal::AbstractSkeletonMelee(goal) => goal.stop(level, mob),
            Goal::RandomStroll(goal) => goal.stop(level, mob),
            Goal::DrownedGoToWater(_) | Goal::DrownedGoToBeach(_) => {}
            Goal::DrownedTridentAttack(goal) => goal.stop(level, mob),
            Goal::DrownedAttack(goal) => goal.zombie_attack_goal.stop(level, mob),
            Goal::DrownedSwimUp(goal) => goal.stop(level, mob),
            Goal::ClimbOnTopOfPowderSnow(_) | Goal::SilverfishWakeUpFriends => {}
            Goal::SilverfishMergeWithStone(goal) => goal.random_stroll_goal.stop(level, mob),
            Goal::ResetUniversalAngerTarget(_) | Goal::EndermanFreezeWhenLookedAt(_) | Goal::EndermanLeaveBlock(_) | Goal::EndermanTakeBlock(_) => {}
            Goal::EndermanLookForPlayer(goal) => goal.stop(level, mob),
            Goal::RangedAttack(goal) => goal.stop(),
            Goal::LongDistancePatrol(_) => {}
            Goal::ObtainRaidLeaderBanner(goal) => goal.stop(),
            Goal::RaiderCelebration(goal) => goal.stop(level, mob),
            Goal::NearestHealableRaiderTarget(goal) => goal.nearest_attackable_target_goal.target_goal.stop(level, mob),
            Goal::NearestAttackableWitchTarget(goal) => goal.nearest_attackable_target_goal.target_goal.stop(level, mob),
            Goal::CubeMobAttack(_) | Goal::CubeMobFloat(_) | Goal::CubeMobKeepOnJumping(_) | Goal::CubeMobRandomDirection(_) => {}
            Goal::TamableAnimalPanic(goal) => goal.panic_goal.stop(),
            Goal::SitWhenOrderedTo(goal) => goal.stop(level, mob),
            Goal::WolfAvoidEntity(goal) => goal.avoid_entity_goal.stop(),
            Goal::FollowOwner(goal) => goal.stop(level, mob),
            Goal::Beg(goal) => goal.stop(level, mob),
            Goal::OwnerHurtByTarget(goal) => goal.target_goal.stop(level, mob),
            Goal::OwnerHurtTarget(goal) => goal.target_goal.stop(level, mob),
            Goal::NonTameRandomTarget(goal) => goal.nearest_attackable_target_goal.target_goal.stop(level, mob),
            Goal::SquidRandomMovement(_) | Goal::SquidFlee(_) => {}
        }
    }

    pub fn requires_update_every_tick(&self) -> bool {
        matches!(
            self,
            Goal::Float(_)
                | Goal::RandomLookAround(_)
                | Goal::MeleeAttack(_)
                | Goal::ZombieAttack(_)
                | Goal::ZombieAttackTurtleEgg(_)
                | Goal::BreakDoor(_)
                | Goal::Swell(_)
                | Goal::SpiderAttack(_)
                | Goal::RangedBowAttack(_)
                | Goal::AbstractSkeletonMelee(_)
                | Goal::DrownedTridentAttack(_)
                | Goal::DrownedAttack(_)
                | Goal::DrownedGoToBeach(_)
                | Goal::ClimbOnTopOfPowderSnow(_)
                | Goal::RangedAttack(_)
                | Goal::CubeMobAttack(_)
                | Goal::CubeMobFloat(_)
                | Goal::SquidFlee(_)
        )
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        match self {
            Goal::Float(goal) => goal.tick(level, mob),
            Goal::Panic(_) | Goal::WaterAvoidingRandomStroll(_) | Goal::RestrictSun(_) | Goal::FleeSun(_) | Goal::NearestAttackableTarget(_) | Goal::HurtByTarget(_) => {}
            Goal::Breed(goal) => goal.tick(level, mob),
            Goal::Tempt(goal) => goal.tick(level, mob),
            Goal::FollowParent(goal) => goal.tick(level, mob),
            Goal::EatBlock(goal) => goal.tick(level, mob),
            Goal::LookAtPlayer(goal) => goal.tick(level, mob),
            Goal::RandomLookAround(goal) => goal.tick(level, mob),
            Goal::MeleeAttack(goal) => goal.tick(level, mob),
            Goal::ZombieAttack(goal) => goal.tick(level, mob),
            Goal::ZombieAttackTurtleEgg(goal) => goal.tick(level, mob),
            Goal::BreakDoor(goal) => goal.tick(level, mob),
            Goal::Swell(goal) => goal.tick(level, mob),
            Goal::AvoidEntity(goal) => goal.tick(level, mob),
            Goal::LeapAtTarget(_) | Goal::SpiderTarget(_) => {}
            Goal::CubeMobAttack(goal) => goal.tick(level, mob),
            Goal::CubeMobFloat(goal) => goal.tick(level, mob),
            Goal::CubeMobKeepOnJumping(goal) => goal.tick(level, mob),
            Goal::CubeMobRandomDirection(goal) => goal.tick(level, mob),
            Goal::SquidRandomMovement(goal) => goal.tick(level, mob),
            Goal::SquidFlee(goal) => goal.tick(level, mob),
            Goal::SpiderAttack(goal) => goal.melee_attack_goal.tick(level, mob),
            Goal::RangedBowAttack(goal) => goal.tick(level, mob),
            Goal::AbstractSkeletonMelee(goal) => goal.melee_attack_goal.tick(level, mob),
            Goal::RandomStroll(_) | Goal::DrownedGoToWater(_) => {}
            Goal::DrownedTridentAttack(goal) => goal.ranged_attack_goal.tick(level, mob),
            Goal::DrownedAttack(goal) => goal.zombie_attack_goal.tick(level, mob),
            Goal::DrownedGoToBeach(goal) => goal.tick(level, mob),
            Goal::DrownedSwimUp(goal) => goal.tick(level, mob),
            Goal::ClimbOnTopOfPowderSnow(goal) => goal.tick(level, mob),
            Goal::SilverfishWakeUpFriends => SilverfishWakeUpFriendsGoal::tick(level, mob),
            Goal::SilverfishMergeWithStone(_) => {}
            Goal::ResetUniversalAngerTarget(_) => {}
            Goal::EndermanFreezeWhenLookedAt(goal) => goal.tick(level, mob),
            Goal::EndermanLeaveBlock(goal) => goal.tick(level, mob),
            Goal::EndermanLookForPlayer(goal) => goal.tick(level, mob),
            Goal::EndermanTakeBlock(goal) => goal.tick(level, mob),
            Goal::RangedAttack(goal) => goal.tick(level, mob),
            Goal::LongDistancePatrol(goal) => goal.tick(level, mob),
            Goal::ObtainRaidLeaderBanner(goal) => goal.tick(level, mob),
            Goal::RaiderCelebration(goal) => goal.tick(level, mob),
            Goal::NearestHealableRaiderTarget(_) | Goal::NearestAttackableWitchTarget(_) => {}
            Goal::TamableAnimalPanic(goal) => goal.tick(level, mob),
            Goal::WolfAvoidEntity(goal) => goal.tick(level, mob),
            Goal::FollowOwner(goal) => goal.tick(level, mob),
            Goal::Beg(goal) => goal.tick(level, mob),
            Goal::SitWhenOrderedTo(_) | Goal::OwnerHurtByTarget(_) | Goal::OwnerHurtTarget(_) | Goal::NonTameRandomTarget(_) => {}
        }
    }

    pub fn get_flags(&self) -> u8 {
        match self {
            Goal::Float(_) => Flag::Jump.bit(),
            Goal::Panic(_)
            | Goal::WaterAvoidingRandomStroll(_)
            | Goal::FleeSun(_)
            | Goal::Swell(_)
            | Goal::AvoidEntity(_)
            | Goal::RandomStroll(_)
            | Goal::DrownedGoToWater(_) => Flag::Move.bit(),
            Goal::Breed(_)
            | Goal::Tempt(_)
            | Goal::RandomLookAround(_)
            | Goal::MeleeAttack(_)
            | Goal::ZombieAttack(_)
            | Goal::RangedBowAttack(_)
            | Goal::AbstractSkeletonMelee(_)
            | Goal::DrownedTridentAttack(_)
            | Goal::DrownedAttack(_) => Flag::Move.bit() | Flag::Look.bit(),
            Goal::FollowParent(_) | Goal::BreakDoor(_) | Goal::RestrictSun(_) | Goal::DrownedSwimUp(_) => 0,
            Goal::EatBlock(_) => Flag::Move.bit() | Flag::Look.bit() | Flag::Jump.bit(),
            Goal::ZombieAttackTurtleEgg(_) | Goal::DrownedGoToBeach(_) => Flag::Move.bit() | Flag::Jump.bit(),
            Goal::LookAtPlayer(_) => Flag::Look.bit(),
            Goal::NearestAttackableTarget(_) | Goal::HurtByTarget(_) => Flag::Target.bit(),
            Goal::LeapAtTarget(_) => Flag::Jump.bit() | Flag::Move.bit(),
            Goal::SpiderAttack(_) => Flag::Move.bit() | Flag::Look.bit(),
            Goal::SpiderTarget(_) => Flag::Target.bit(),
            Goal::ClimbOnTopOfPowderSnow(_) => Flag::Jump.bit(),
            Goal::SilverfishWakeUpFriends => 0,
            Goal::SilverfishMergeWithStone(_) => Flag::Move.bit(),
            Goal::ResetUniversalAngerTarget(_) | Goal::EndermanLeaveBlock(_) | Goal::EndermanTakeBlock(_) => 0,
            Goal::EndermanFreezeWhenLookedAt(_) => Flag::Jump.bit() | Flag::Move.bit(),
            Goal::EndermanLookForPlayer(_) => Flag::Target.bit(),
            Goal::RangedAttack(_) => Flag::Move.bit() | Flag::Look.bit(),
            Goal::LongDistancePatrol(_) | Goal::ObtainRaidLeaderBanner(_) | Goal::RaiderCelebration(_) => Flag::Move.bit(),
            Goal::NearestHealableRaiderTarget(_) | Goal::NearestAttackableWitchTarget(_) => Flag::Target.bit(),
            Goal::CubeMobAttack(_) | Goal::CubeMobRandomDirection(_) => Flag::Look.bit(),
            Goal::CubeMobFloat(_) | Goal::CubeMobKeepOnJumping(_) => Flag::Jump.bit() | Flag::Move.bit(),
            Goal::TamableAnimalPanic(_) | Goal::WolfAvoidEntity(_) => Flag::Move.bit(),
            Goal::SitWhenOrderedTo(_) => Flag::Jump.bit() | Flag::Move.bit(),
            Goal::FollowOwner(_) => Flag::Move.bit() | Flag::Look.bit(),
            Goal::Beg(_) => Flag::Look.bit(),
            Goal::OwnerHurtByTarget(_) | Goal::OwnerHurtTarget(_) | Goal::NonTameRandomTarget(_) => Flag::Target.bit(),
            Goal::SquidRandomMovement(_) | Goal::SquidFlee(_) => 0,
        }
    }

    pub fn adjusted_tick_delay(&self, ticks: i32) -> i32 {
        if self.requires_update_every_tick() { ticks } else { reduced_tick_delay(ticks) }
    }
}

pub fn reduced_tick_delay(ticks: i32) -> i32 {
    (ticks + 1) / 2
}
