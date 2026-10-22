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
use super::llama_follow_caravan_goal::LlamaFollowCaravanGoal;
use super::look_at_trading_player_goal::LookAtTradingPlayerGoal;
use super::trade_with_player_goal::TradeWithPlayerGoal;
use super::use_item_goal::UseItemGoal;
use crate::world::entity::monster::guardian::GuardianAttackGoal;
use crate::world::entity::monster::phantom::{PhantomAttackPlayerTargetGoal, PhantomAttackStrategyGoal, PhantomCircleAroundAnchorGoal, PhantomSweepAttackGoal};
use crate::world::entity::animal::equine::trader_llama::TraderLlamaDefendWanderingTraderGoal;
use crate::world::entity::npc::wanderingtrader::wandering_trader::WanderToPositionGoal;
use super::ranged_crossbow_attack_goal::RangedCrossbowAttackGoal;
use super::spear_use_goal::SpearUseGoal;
use crate::world::entity::monster::illager::illusioner::{IllusionerBlindnessSpellGoal, IllusionerMirrorSpellGoal};
use crate::world::entity::monster::illager::spellcaster_illager::SpellcasterCastingSpellGoal;
use crate::world::entity::monster::illager::evoker::{EvokerAttackSpellGoal, EvokerCastingSpellGoal, EvokerSummonSpellGoal, EvokerWololoSpellGoal};
use crate::world::entity::animal::equine::skeleton_horse::SkeletonTrapGoal;
use crate::world::entity::animal::panda::panda::{
    PandaAttackGoal, PandaAvoidGoal, PandaBreedGoal, PandaHurtByTargetGoal, PandaLieOnBackGoal, PandaLookAtPlayerGoal, PandaPanicGoal, PandaRollGoal, PandaSitGoal, PandaSneezeGoal,
};
use crate::world::entity::animal::polarbear::polar_bear::{PolarBearAttackPlayersGoal, PolarBearHurtByTargetGoal, PolarBearMeleeAttackGoal};
use crate::world::entity::monster::vex::{VexChargeAttackGoal, VexCopyOwnerTargetGoal, VexRandomMoveGoal};
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
use crate::world::entity::monster::illager::abstract_illager::RaiderOpenDoorGoal;
use crate::world::entity::monster::illager::vindicator::{VindicatorBreakDoorGoal, VindicatorJohnnyAttackGoal};
use crate::world::entity::raid::raider::{HoldGroundAttackGoal, ObtainRaidLeaderBannerGoal, PathfindToRaidGoal, RaiderCelebration, RaiderMoveThroughVillageGoal};
use crate::world::entity::monster::zombie::zombie::ZombieAttackTurtleEggGoal;
use super::leap_at_target_goal::LeapAtTargetGoal;
use crate::world::entity::monster::spider::spider::{SpiderAttackGoal, SpiderTargetGoal};
use super::climb_on_top_of_powder_snow_goal::ClimbOnTopOfPowderSnowGoal;
use crate::world::entity::monster::silverfish::{SilverfishMergeWithStoneGoal, SilverfishWakeUpFriendsGoal};
use super::target::reset_universal_anger_target_goal::ResetUniversalAngerTargetGoal;
use crate::world::entity::monster::enderman::{EndermanFreezeWhenLookedAt, EndermanLeaveBlockGoal, EndermanLookForPlayerGoal, EndermanTakeBlockGoal};
use crate::world::entity::monster::cubemob::abstract_cube_mob::{CubeMobAttackGoal, CubeMobFloatGoal, CubeMobKeepOnJumpingGoal, CubeMobRandomDirectionGoal};
use crate::world::entity::animal::squid::squid::{SquidFleeGoal, SquidRandomMovementGoal};
use super::golem_random_stroll_in_village_goal::GolemRandomStrollInVillageGoal;
use super::move_back_to_village_goal::MoveBackToVillageGoal;
use super::move_towards_target_goal::MoveTowardsTargetGoal;
use super::offer_flower_goal::OfferFlowerGoal;
use super::target::defend_village_target_goal::DefendVillageTargetGoal;
use super::follow_flock_leader_goal::FollowFlockLeaderGoal;
use crate::world::entity::animal::fish::abstract_fish::FishSwimGoal;
use crate::world::entity::animal::fish::pufferfish::PufferfishPuffGoal;
use super::cat_lie_on_block_goal::CatLieOnBlockGoal;
use super::cat_sit_on_block_goal::CatSitOnBlockGoal;
use super::ocelot_attack_goal::OcelotAttackGoal;
use crate::world::entity::animal::feline::cat::{CatAvoidEntityGoal, CatRelaxOnOwnerGoal, CatTemptGoal};
use crate::world::entity::animal::feline::ocelot::{OcelotAvoidEntityGoal, OcelotTemptGoal};
use super::random_stand_goal::RandomStandGoal;
use super::run_around_like_crazy_goal::RunAroundLikeCrazyGoal;
use crate::world::entity::animal::equine::abstract_horse::MountPanicGoal;
use crate::world::entity::animal::fox::fox::{
    DefendTrustedTargetGoal, FaceplantGoal, FoxBreedGoal, FoxEatBerriesGoal, FoxFloatGoal, FoxFollowParentGoal, FoxLookAtPlayerGoal, FoxMeleeAttackGoal, FoxPanicGoal,
    FoxPounceGoal, FoxSearchForItemsGoal, FoxStrollThroughVillageGoal, PerchAndSearchGoal, SeekShelterGoal, SleepGoal, StalkPreyGoal,
};
use super::move_towards_restriction_goal::MoveTowardsRestrictionGoal;
use crate::world::entity::monster::blaze::BlazeAttackGoal;
use crate::world::entity::monster::ghast::{self, GhastShootFireballGoal, RandomFloatAroundGoal};
use crate::world::entity::animal::rabbit::rabbit::{RabbitAvoidEntityGoal, RabbitPanicGoal, RaidGardenGoal};
use crate::world::entity::animal::turtle::turtle::{
    TurtleBreedGoal, TurtleGoHomeGoal, TurtleGoToWaterGoal, TurtleLayEggGoal, TurtlePanicGoal, TurtleRandomStrollGoal, TurtleTravelGoal,
};

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
    RangedCrossbowAttack(RangedCrossbowAttackGoal),
    SpearUse(SpearUseGoal),
    SpellcasterCastingSpell(SpellcasterCastingSpellGoal),
    EvokerCastingSpell(EvokerCastingSpellGoal),
    IllusionerMirrorSpell(IllusionerMirrorSpellGoal),
    IllusionerBlindnessSpell(IllusionerBlindnessSpellGoal),
    EvokerSummonSpell(EvokerSummonSpellGoal),
    EvokerAttackSpell(EvokerAttackSpellGoal),
    EvokerWololoSpell(EvokerWololoSpellGoal),
    VexChargeAttack(VexChargeAttackGoal),
    VexRandomMove(VexRandomMoveGoal),
    VexCopyOwnerTarget(VexCopyOwnerTargetGoal),
    SkeletonTrap(SkeletonTrapGoal),
    PolarBearMeleeAttack(PolarBearMeleeAttackGoal),
    PolarBearHurtByTarget(PolarBearHurtByTargetGoal),
    PandaPanic(PandaPanicGoal),
    PandaBreed(PandaBreedGoal),
    PandaAttack(PandaAttackGoal),
    PandaAvoid(PandaAvoidGoal),
    PandaSit(PandaSitGoal),
    PandaLieOnBack(PandaLieOnBackGoal),
    PandaSneeze(PandaSneezeGoal),
    PandaLookAtPlayer(PandaLookAtPlayerGoal),
    PandaRoll(PandaRollGoal),
    PandaHurtByTarget(PandaHurtByTargetGoal),
    PolarBearAttackPlayers(PolarBearAttackPlayersGoal),
    LlamaFollowCaravan(LlamaFollowCaravanGoal),
    LlamaHurtByTarget(HurtByTargetGoal),
    TraderLlamaDefendWanderingTarget(TraderLlamaDefendWanderingTraderGoal),
    UseItem(UseItemGoal),
    TradeWithPlayer(TradeWithPlayerGoal),
    LookAtTradingPlayer(LookAtTradingPlayerGoal),
    WanderToPosition(WanderToPositionGoal),
    PhantomAttackStrategy(PhantomAttackStrategyGoal),
    PhantomSweepAttack(PhantomSweepAttackGoal),
    PhantomCircleAroundAnchor(PhantomCircleAroundAnchorGoal),
    PhantomAttackPlayerTarget(PhantomAttackPlayerTargetGoal),
    GuardianAttack(GuardianAttackGoal),
    Interact(LookAtPlayerGoal),
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
    PathfindToRaid(PathfindToRaidGoal),
    RaiderMoveThroughVillage(RaiderMoveThroughVillageGoal),
    HoldGroundAttack(HoldGroundAttackGoal),
    RaiderOpenDoor(RaiderOpenDoorGoal),
    VindicatorBreakDoor(VindicatorBreakDoorGoal),
    VindicatorJohnnyAttack(VindicatorJohnnyAttackGoal),
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
    MoveTowardsTarget(MoveTowardsTargetGoal),
    MoveBackToVillage(MoveBackToVillageGoal),
    GolemRandomStrollInVillage(GolemRandomStrollInVillageGoal),
    OfferFlower(OfferFlowerGoal),
    DefendVillageTarget(DefendVillageTargetGoal),
    FishSwim(FishSwimGoal),
    FollowFlockLeader(FollowFlockLeaderGoal),
    PufferfishPuff(PufferfishPuffGoal),
    CatAvoidEntity(CatAvoidEntityGoal),
    CatRelaxOnOwner(CatRelaxOnOwnerGoal),
    CatTempt(CatTemptGoal),
    CatLieOnBlock(CatLieOnBlockGoal),
    CatSitOnBlock(CatSitOnBlockGoal),
    StriderGoToLava(crate::world::entity::monster::strider::StriderGoToLavaGoal),
    OcelotAttack(OcelotAttackGoal),
    OcelotAvoidEntity(OcelotAvoidEntityGoal),
    OcelotTempt(OcelotTemptGoal),
    RunAroundLikeCrazy(RunAroundLikeCrazyGoal),
    RandomStand(RandomStandGoal),
    MountPanic(MountPanicGoal),
    FoxFloat(FoxFloatGoal),
    Faceplant(FaceplantGoal),
    FoxPanic(FoxPanicGoal),
    FoxBreed(FoxBreedGoal),
    StalkPrey(StalkPreyGoal),
    FoxPounce(FoxPounceGoal),
    SeekShelter(SeekShelterGoal),
    FoxMeleeAttack(FoxMeleeAttackGoal),
    Sleep(SleepGoal),
    FoxFollowParent(FoxFollowParentGoal),
    FoxStrollThroughVillage(FoxStrollThroughVillageGoal),
    FoxEatBerries(FoxEatBerriesGoal),
    FoxSearchForItems(FoxSearchForItemsGoal),
    FoxLookAtPlayer(FoxLookAtPlayerGoal),
    PerchAndSearch(PerchAndSearchGoal),
    DefendTrustedTarget(DefendTrustedTargetGoal),
    MoveTowardsRestriction(MoveTowardsRestrictionGoal),
    BlazeAttack(BlazeAttackGoal),
    RandomFloatAround(RandomFloatAroundGoal),
    GhastLook,
    GhastShootFireball(GhastShootFireballGoal),
    RabbitPanic(RabbitPanicGoal),
    RabbitAvoidEntity(RabbitAvoidEntityGoal),
    RaidGarden(RaidGardenGoal),
    TurtlePanic(TurtlePanicGoal),
    TurtleBreed(TurtleBreedGoal),
    TurtleLayEgg(TurtleLayEggGoal),
    TurtleGoToWater(TurtleGoToWaterGoal),
    TurtleGoHome(TurtleGoHomeGoal),
    TurtleTravel(TurtleTravelGoal),
    TurtleRandomStroll(TurtleRandomStrollGoal),
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
            Goal::LookAtPlayer(goal) | Goal::Interact(goal) => goal.can_use(level, mob),
            Goal::PhantomAttackStrategy(goal) => goal.can_use(level, mob),
            Goal::PhantomSweepAttack(goal) => goal.can_use(level, mob),
            Goal::PhantomCircleAroundAnchor(goal) => goal.can_use(level, mob),
            Goal::PhantomAttackPlayerTarget(goal) => goal.can_use(level, mob),
            Goal::GuardianAttack(goal) => goal.can_use(level, mob),
            Goal::LlamaFollowCaravan(goal) => goal.can_use(level, mob),
            Goal::LlamaHurtByTarget(goal) => goal.can_use(level, mob),
            Goal::TraderLlamaDefendWanderingTarget(goal) => goal.can_use(level, mob),
            Goal::UseItem(goal) => goal.can_use(level, mob),
            Goal::TradeWithPlayer(goal) => goal.can_use(level, mob),
            Goal::LookAtTradingPlayer(goal) => goal.can_use(level, mob),
            Goal::WanderToPosition(goal) => goal.can_use(level, mob),
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
            Goal::RangedCrossbowAttack(goal) => goal.can_use(level, mob),
            Goal::SpearUse(goal) => goal.can_use(level, mob),
            Goal::AbstractSkeletonMelee(goal) => goal.melee_attack_goal.can_use(level, mob),
            Goal::RandomStroll(goal) => {
                if level.guardian_take_random_stroll_trigger(mob) {
                    goal.trigger();
                }
                goal.can_use(level, mob, RandomStrollGoal::get_position)
            }
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
            Goal::PathfindToRaid(goal) => goal.can_use(level, mob),
            Goal::RaiderMoveThroughVillage(goal) => goal.can_use(level, mob),
            Goal::HoldGroundAttack(goal) => goal.can_use(level, mob),
            Goal::RaiderOpenDoor(goal) => goal.can_use(level, mob),
            Goal::VindicatorBreakDoor(goal) => goal.can_use(level, mob),
            Goal::VindicatorJohnnyAttack(goal) => goal.can_use(level, mob),
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
            Goal::MoveTowardsTarget(goal) => goal.can_use(level, mob),
            Goal::MoveBackToVillage(goal) => goal.can_use(level, mob),
            Goal::GolemRandomStrollInVillage(goal) => goal.can_use(level, mob),
            Goal::OfferFlower(goal) => goal.can_use(level, mob),
            Goal::DefendVillageTarget(goal) => goal.can_use(level, mob),
            Goal::FishSwim(goal) => goal.can_use(level, mob),
            Goal::FollowFlockLeader(goal) => goal.can_use(level, mob),
            Goal::PufferfishPuff(goal) => goal.can_use(level, mob),
            Goal::CatAvoidEntity(goal) => goal.can_use(level, mob),
            Goal::CatRelaxOnOwner(goal) => goal.can_use(level, mob),
            Goal::CatTempt(goal) => goal.can_use(level, mob),
            Goal::CatLieOnBlock(goal) => goal.can_use(level, mob),
            Goal::CatSitOnBlock(goal) => goal.can_use(level, mob),
            Goal::StriderGoToLava(goal) => goal.can_use(level, mob),
            Goal::OcelotAttack(goal) => goal.can_use(level, mob),
            Goal::OcelotAvoidEntity(goal) => goal.can_use(level, mob),
            Goal::OcelotTempt(goal) => goal.tempt_goal.can_use(level, mob),
            Goal::RunAroundLikeCrazy(goal) => goal.can_use(level, mob),
            Goal::RandomStand(goal) => goal.can_use(level, mob),
            Goal::MountPanic(goal) => goal.can_use(level, mob),
            Goal::FoxFloat(goal) => goal.can_use(level, mob),
            Goal::Faceplant(goal) => goal.can_use(level, mob),
            Goal::FoxPanic(goal) => goal.can_use(level, mob),
            Goal::FoxBreed(goal) => goal.breed_goal.can_use(level, mob),
            Goal::StalkPrey(goal) => goal.can_use(level, mob),
            Goal::FoxPounce(goal) => goal.can_use(level, mob),
            Goal::SeekShelter(goal) => goal.can_use(level, mob),
            Goal::FoxMeleeAttack(goal) => goal.can_use(level, mob),
            Goal::Sleep(goal) => goal.can_use(level, mob),
            Goal::FoxFollowParent(goal) => goal.can_use(level, mob),
            Goal::FoxStrollThroughVillage(goal) => goal.can_use(level, mob),
            Goal::FoxEatBerries(goal) => goal.can_use(level, mob),
            Goal::FoxSearchForItems(goal) => goal.can_use(level, mob),
            Goal::FoxLookAtPlayer(goal) => goal.can_use(level, mob),
            Goal::PerchAndSearch(goal) => goal.can_use(level, mob),
            Goal::DefendTrustedTarget(goal) => goal.can_use(level, mob),
            Goal::MoveTowardsRestriction(goal) => goal.can_use(level, mob),
            Goal::BlazeAttack(goal) => goal.can_use(level, mob),
            Goal::RandomFloatAround(goal) => goal.can_use(level, mob),
            Goal::GhastLook => true,
            Goal::GhastShootFireball(goal) => goal.can_use(level, mob),
            Goal::EvokerCastingSpell(goal) => goal.casting_spell_goal.can_use(level, mob),
            Goal::SpellcasterCastingSpell(goal) => goal.can_use(level, mob),
            Goal::IllusionerMirrorSpell(goal) => goal.can_use(level, mob),
            Goal::IllusionerBlindnessSpell(goal) => goal.can_use(level, mob),
            Goal::EvokerSummonSpell(goal) => goal.can_use(level, mob),
            Goal::EvokerAttackSpell(goal) => goal.use_spell_goal.can_use(level, mob),
            Goal::EvokerWololoSpell(goal) => goal.can_use(level, mob),
            Goal::VexChargeAttack(goal) => goal.can_use(level, mob),
            Goal::VexRandomMove(goal) => goal.can_use(level, mob),
            Goal::VexCopyOwnerTarget(goal) => goal.can_use(level, mob),
            Goal::SkeletonTrap(goal) => goal.can_use(level, mob),
            Goal::PolarBearMeleeAttack(goal) => goal.melee_attack_goal.can_use(level, mob),
            Goal::PolarBearHurtByTarget(goal) => goal.hurt_by_target_goal.can_use(level, mob),
            Goal::PandaPanic(goal) => goal.panic_goal.can_use(level, mob),
            Goal::PandaBreed(goal) => goal.can_use(level, mob),
            Goal::PandaAttack(goal) => goal.can_use(level, mob),
            Goal::PandaAvoid(goal) => goal.can_use(level, mob),
            Goal::PandaSit(goal) => goal.can_use(level, mob),
            Goal::PandaLieOnBack(goal) => goal.can_use(level, mob),
            Goal::PandaSneeze(goal) => goal.can_use(level, mob),
            Goal::PandaLookAtPlayer(goal) => goal.can_use(level, mob),
            Goal::PandaRoll(goal) => goal.can_use(level, mob),
            Goal::PandaHurtByTarget(goal) => goal.hurt_by_target_goal.can_use(level, mob),
            Goal::PolarBearAttackPlayers(goal) => goal.can_use(level, mob),
            Goal::RabbitPanic(goal) => goal.panic_goal.can_use(level, mob),
            Goal::RabbitAvoidEntity(goal) => goal.can_use(level, mob),
            Goal::RaidGarden(goal) => goal.can_use(level, mob),
            Goal::TurtlePanic(goal) => goal.can_use(level, mob),
            Goal::TurtleBreed(goal) => goal.can_use(level, mob),
            Goal::TurtleLayEgg(goal) => goal.can_use(level, mob),
            Goal::TurtleGoToWater(goal) => goal.can_use(level, mob),
            Goal::TurtleGoHome(goal) => goal.can_use(level, mob),
            Goal::TurtleTravel(goal) => goal.can_use(level, mob),
            Goal::TurtleRandomStroll(goal) => goal.can_use(level, mob),
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
            Goal::LookAtPlayer(goal) | Goal::Interact(goal) => goal.can_continue_to_use(level, mob),
            Goal::PhantomAttackStrategy(goal) => goal.can_use(level, mob),
            Goal::PhantomSweepAttack(goal) => goal.can_continue_to_use(level, mob),
            Goal::PhantomCircleAroundAnchor(goal) => goal.can_use(level, mob),
            Goal::PhantomAttackPlayerTarget(goal) => goal.can_continue_to_use(level, mob),
            Goal::GuardianAttack(goal) => goal.can_continue_to_use(level, mob),
            Goal::LlamaFollowCaravan(goal) => goal.can_continue_to_use(level, mob),
            Goal::LlamaHurtByTarget(goal) => level.llama_hurt_by_target_can_continue_to_use(goal, mob),
            Goal::TraderLlamaDefendWanderingTarget(goal) => goal.target_goal.can_continue_to_use(level, mob),
            Goal::UseItem(goal) => goal.can_continue_to_use(level, mob),
            Goal::TradeWithPlayer(goal) => goal.can_use(level, mob),
            Goal::LookAtTradingPlayer(goal) => goal.look_at_player_goal.can_continue_to_use(level, mob),
            Goal::WanderToPosition(goal) => goal.can_use(level, mob),
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
            Goal::RangedCrossbowAttack(goal) => goal.can_continue_to_use(level, mob),
            Goal::SpearUse(goal) => goal.can_continue_to_use(level, mob),
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
            Goal::PathfindToRaid(goal) => goal.can_continue_to_use(level, mob),
            Goal::RaiderMoveThroughVillage(_) => false,
            Goal::HoldGroundAttack(goal) => goal.can_use(level, mob),
            Goal::RaiderOpenDoor(goal) => goal.open_door_goal.can_continue_to_use(),
            Goal::VindicatorBreakDoor(goal) => goal.can_continue_to_use(level, mob),
            Goal::VindicatorJohnnyAttack(goal) => goal.nearest_attackable_target_goal.target_goal.can_continue_to_use(level, mob),
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
            Goal::MoveTowardsTarget(goal) => goal.can_continue_to_use(level, mob),
            Goal::MoveBackToVillage(goal) => goal.random_stroll_goal.can_continue_to_use(level, mob),
            Goal::GolemRandomStrollInVillage(goal) => goal.random_stroll_goal.can_continue_to_use(level, mob),
            Goal::OfferFlower(goal) => goal.can_continue_to_use(),
            Goal::DefendVillageTarget(goal) => goal.target_goal.can_continue_to_use(level, mob),
            Goal::FishSwim(goal) => goal.random_swimming_goal.random_stroll_goal.can_continue_to_use(level, mob),
            Goal::FollowFlockLeader(goal) => goal.can_continue_to_use(level, mob),
            Goal::PufferfishPuff(goal) => goal.can_use(level, mob),
            Goal::CatAvoidEntity(goal) => goal.can_continue_to_use(level, mob),
            Goal::CatRelaxOnOwner(goal) => goal.can_continue_to_use(level, mob),
            Goal::CatTempt(goal) => goal.can_continue_to_use(level, mob),
            Goal::CatLieOnBlock(goal) => goal.can_continue_to_use(level),
            Goal::CatSitOnBlock(goal) => goal.can_continue_to_use(level),
            Goal::StriderGoToLava(goal) => goal.can_continue_to_use(level, mob),
            Goal::OcelotAttack(goal) => goal.can_continue_to_use(level, mob),
            Goal::OcelotAvoidEntity(goal) => goal.can_continue_to_use(level, mob),
            Goal::OcelotTempt(goal) => goal.can_continue_to_use(level, mob),
            Goal::RunAroundLikeCrazy(goal) => goal.can_continue_to_use(level, mob),
            Goal::RandomStand(goal) => goal.can_continue_to_use(),
            Goal::MountPanic(goal) => goal.panic_goal.can_continue_to_use(level, mob),
            Goal::FoxFloat(goal) => goal.can_use(level, mob),
            Goal::Faceplant(goal) => goal.can_continue_to_use(level, mob),
            Goal::FoxPanic(goal) => goal.panic_goal.can_continue_to_use(level, mob),
            Goal::FoxBreed(goal) => goal.breed_goal.can_continue_to_use(level, mob),
            Goal::StalkPrey(goal) => goal.can_use(level, mob),
            Goal::FoxPounce(goal) => goal.can_continue_to_use(level, mob),
            Goal::SeekShelter(goal) => goal.flee_sun_goal.can_continue_to_use(level, mob),
            Goal::FoxMeleeAttack(goal) => goal.melee_attack_goal.can_continue_to_use(level, mob),
            Goal::Sleep(goal) => goal.can_continue_to_use(level, mob),
            Goal::FoxFollowParent(goal) => goal.can_continue_to_use(level, mob),
            Goal::FoxStrollThroughVillage(goal) => goal.can_continue_to_use(level, mob),
            Goal::FoxEatBerries(goal) => goal.can_continue_to_use(level),
            Goal::FoxSearchForItems(goal) => goal.can_use(level, mob),
            Goal::FoxLookAtPlayer(goal) => goal.can_continue_to_use(level, mob),
            Goal::PerchAndSearch(goal) => goal.can_continue_to_use(),
            Goal::DefendTrustedTarget(goal) => goal.nearest_attackable_target_goal.target_goal.can_continue_to_use(level, mob),
            Goal::MoveTowardsRestriction(goal) => goal.can_continue_to_use(level, mob),
            Goal::BlazeAttack(goal) => goal.can_use(level, mob),
            Goal::RandomFloatAround(_) => false,
            Goal::GhastLook => true,
            Goal::GhastShootFireball(goal) => goal.can_use(level, mob),
            Goal::EvokerCastingSpell(goal) => goal.casting_spell_goal.can_use(level, mob),
            Goal::SpellcasterCastingSpell(goal) => goal.can_use(level, mob),
            Goal::IllusionerMirrorSpell(goal) => goal.use_spell_goal.can_continue_to_use(level, mob),
            Goal::IllusionerBlindnessSpell(goal) => goal.use_spell_goal.can_continue_to_use(level, mob),
            Goal::EvokerSummonSpell(goal) => goal.use_spell_goal.can_continue_to_use(level, mob),
            Goal::EvokerAttackSpell(goal) => goal.use_spell_goal.can_continue_to_use(level, mob),
            Goal::EvokerWololoSpell(goal) => goal.can_continue_to_use(level, mob),
            Goal::VexChargeAttack(goal) => goal.can_continue_to_use(level, mob),
            Goal::VexRandomMove(_) => false,
            Goal::VexCopyOwnerTarget(goal) => goal.target_goal.can_continue_to_use(level, mob),
            Goal::SkeletonTrap(goal) => goal.can_use(level, mob),
            Goal::PolarBearMeleeAttack(goal) => goal.melee_attack_goal.can_continue_to_use(level, mob),
            Goal::PolarBearHurtByTarget(goal) => goal.hurt_by_target_goal.target_goal.can_continue_to_use(level, mob),
            Goal::PandaPanic(goal) => goal.can_continue_to_use(level, mob),
            Goal::PandaBreed(goal) => goal.breed_goal.can_continue_to_use(level, mob),
            Goal::PandaAttack(goal) => goal.melee_attack_goal.can_continue_to_use(level, mob),
            Goal::PandaAvoid(goal) => goal.avoid_entity_goal.can_continue_to_use(level, mob),
            Goal::PandaSit(goal) => goal.can_continue_to_use(level, mob),
            Goal::PandaLieOnBack(goal) => goal.can_continue_to_use(level, mob),
            Goal::PandaSneeze(_) | Goal::PandaRoll(_) => false,
            Goal::PandaLookAtPlayer(goal) => goal.can_continue_to_use(level, mob),
            Goal::PandaHurtByTarget(goal) => goal.can_continue_to_use(level, mob),
            Goal::PolarBearAttackPlayers(goal) => goal.nearest_attackable_target_goal.target_goal.can_continue_to_use(level, mob),
            Goal::RabbitPanic(goal) => goal.panic_goal.can_continue_to_use(level, mob),
            Goal::RabbitAvoidEntity(goal) => goal.avoid_entity_goal.can_continue_to_use(level, mob),
            Goal::RaidGarden(goal) => goal.can_continue_to_use(level),
            Goal::TurtlePanic(goal) => goal.panic_goal.can_continue_to_use(level, mob),
            Goal::TurtleBreed(goal) => goal.breed_goal.can_continue_to_use(level, mob),
            Goal::TurtleLayEgg(goal) => goal.can_continue_to_use(level, mob),
            Goal::TurtleGoToWater(goal) => goal.can_continue_to_use(level, mob),
            Goal::TurtleGoHome(goal) => goal.can_continue_to_use(level, mob),
            Goal::TurtleTravel(goal) => goal.can_continue_to_use(level, mob),
            Goal::TurtleRandomStroll(goal) => goal.random_stroll_goal.can_continue_to_use(level, mob),
        }
    }

    pub fn is_interruptable(&self) -> bool {
        !matches!(self, Goal::FoxPounce(_) | Goal::PandaRoll(_))
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        match self {
            Goal::Float(_) | Goal::Breed(_) => {}
            Goal::Panic(goal) => goal.start(level, mob),
            Goal::Tempt(goal) => goal.start(level),
            Goal::FollowParent(goal) => goal.start(),
            Goal::EatBlock(goal) => goal.start(level, mob),
            Goal::WaterAvoidingRandomStroll(goal) => goal.random_stroll_goal.start(level, mob),
            Goal::LookAtPlayer(goal) | Goal::Interact(goal) => goal.start(level, mob),
            Goal::PhantomAttackStrategy(goal) => goal.start(level, mob),
            Goal::PhantomCircleAroundAnchor(goal) => goal.start(level, mob),
            Goal::PhantomSweepAttack(_) | Goal::PhantomAttackPlayerTarget(_) => {}
            Goal::GuardianAttack(goal) => goal.start(level, mob),
            Goal::LlamaFollowCaravan(_) => {}
            Goal::LlamaHurtByTarget(goal) => goal.start(level, mob),
            Goal::TraderLlamaDefendWanderingTarget(goal) => goal.start(level, mob),
            Goal::UseItem(goal) => goal.start(level, mob),
            Goal::TradeWithPlayer(goal) => goal.start(level, mob),
            Goal::LookAtTradingPlayer(goal) => goal.look_at_player_goal.start(level, mob),
            Goal::WanderToPosition(_) => {}
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
            Goal::RangedCrossbowAttack(_) => {}
            Goal::SpearUse(goal) => goal.start(level, mob),
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
            Goal::PathfindToRaid(_) | Goal::RaiderMoveThroughVillage(_) => {}
            Goal::HoldGroundAttack(goal) => goal.start(level, mob),
            Goal::RaiderOpenDoor(goal) => goal.open_door_goal.start(level, mob),
            Goal::VindicatorBreakDoor(goal) => goal.start(level, mob),
            Goal::VindicatorJohnnyAttack(goal) => goal.start(level, mob),
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
            Goal::MoveTowardsTarget(goal) => goal.start(level, mob),
            Goal::MoveBackToVillage(goal) => goal.random_stroll_goal.start(level, mob),
            Goal::GolemRandomStrollInVillage(goal) => goal.random_stroll_goal.start(level, mob),
            Goal::OfferFlower(goal) => goal.start(level, mob),
            Goal::DefendVillageTarget(goal) => goal.start(level, mob),
            Goal::FishSwim(goal) => goal.random_swimming_goal.random_stroll_goal.start(level, mob),
            Goal::FollowFlockLeader(goal) => goal.start(),
            Goal::PufferfishPuff(goal) => goal.start(level, mob),
            Goal::CatAvoidEntity(goal) => goal.avoid_entity_goal.start(level, mob),
            Goal::CatRelaxOnOwner(goal) => goal.start(level, mob),
            Goal::CatTempt(goal) => goal.tempt_goal.start(level),
            Goal::CatLieOnBlock(goal) => goal.start(level, mob),
            Goal::CatSitOnBlock(goal) => goal.start(level, mob),
            Goal::StriderGoToLava(goal) => goal.start(level, mob),
            Goal::OcelotAttack(_) => {}
            Goal::OcelotAvoidEntity(goal) => goal.avoid_entity_goal.start(level, mob),
            Goal::OcelotTempt(goal) => goal.tempt_goal.start(level),
            Goal::RunAroundLikeCrazy(goal) => goal.start(level, mob),
            Goal::RandomStand(goal) => goal.start(level, mob),
            Goal::MountPanic(goal) => goal.panic_goal.start(level, mob),
            Goal::FoxFloat(goal) => goal.start(level, mob),
            Goal::Faceplant(goal) => goal.start(),
            Goal::FoxPanic(goal) => goal.panic_goal.start(level, mob),
            Goal::FoxBreed(goal) => goal.start(level, mob),
            Goal::StalkPrey(goal) => goal.start(level, mob),
            Goal::FoxPounce(goal) => goal.start(level, mob),
            Goal::SeekShelter(goal) => goal.start(level, mob),
            Goal::FoxMeleeAttack(goal) => goal.start(level, mob),
            Goal::Sleep(goal) => goal.start(level, mob),
            Goal::FoxFollowParent(goal) => goal.start(level, mob),
            Goal::FoxStrollThroughVillage(goal) => goal.start(level, mob),
            Goal::FoxEatBerries(goal) => goal.start(level, mob),
            Goal::FoxSearchForItems(goal) => goal.start(level, mob),
            Goal::FoxLookAtPlayer(goal) => goal.look_at_player_goal.start(level, mob),
            Goal::PerchAndSearch(goal) => goal.start(level, mob),
            Goal::DefendTrustedTarget(goal) => goal.start(level, mob),
            Goal::MoveTowardsRestriction(goal) => goal.start(level, mob),
            Goal::BlazeAttack(goal) => goal.start(),
            Goal::RandomFloatAround(goal) => goal.start(level, mob),
            Goal::GhastLook => {}
            Goal::GhastShootFireball(goal) => goal.start(),
            Goal::EvokerCastingSpell(goal) => goal.casting_spell_goal.start(level, mob),
            Goal::SpellcasterCastingSpell(goal) => goal.start(level, mob),
            Goal::IllusionerMirrorSpell(goal) => goal.start(level, mob),
            Goal::IllusionerBlindnessSpell(goal) => goal.start(level, mob),
            Goal::EvokerSummonSpell(goal) => goal.start(level, mob),
            Goal::EvokerAttackSpell(goal) => goal.start(level, mob),
            Goal::EvokerWololoSpell(goal) => goal.start(level, mob),
            Goal::VexChargeAttack(goal) => goal.start(level, mob),
            Goal::VexRandomMove(_) => {}
            Goal::VexCopyOwnerTarget(goal) => goal.start(level, mob),
            Goal::SkeletonTrap(_) => {}
            Goal::PolarBearMeleeAttack(goal) => goal.melee_attack_goal.start(level, mob),
            Goal::PolarBearHurtByTarget(goal) => goal.start(level, mob),
            Goal::PandaPanic(goal) => goal.panic_goal.start(level, mob),
            Goal::PandaBreed(_) => {}
            Goal::PandaAttack(goal) => goal.melee_attack_goal.start(level, mob),
            Goal::PandaAvoid(goal) => goal.avoid_entity_goal.start(level, mob),
            Goal::PandaSit(goal) => goal.start(level, mob),
            Goal::PandaLieOnBack(goal) => goal.start(level, mob),
            Goal::PandaSneeze(goal) => goal.start(level, mob),
            Goal::PandaLookAtPlayer(goal) => goal.look_at_player_goal.start(level, mob),
            Goal::PandaRoll(goal) => goal.start(level, mob),
            Goal::PandaHurtByTarget(goal) => goal.start(level, mob),
            Goal::PolarBearAttackPlayers(goal) => goal.nearest_attackable_target_goal.start(level, mob),
            Goal::RabbitPanic(goal) => goal.panic_goal.start(level, mob),
            Goal::RabbitAvoidEntity(goal) => goal.avoid_entity_goal.start(level, mob),
            Goal::RaidGarden(goal) => goal.move_to_block_goal.start(level, mob),
            Goal::TurtlePanic(goal) => goal.panic_goal.start(level, mob),
            Goal::TurtleBreed(_) => {}
            Goal::TurtleLayEgg(goal) => goal.move_to_block_goal.start(level, mob),
            Goal::TurtleGoToWater(goal) => goal.move_to_block_goal.start(level, mob),
            Goal::TurtleGoHome(goal) => goal.start(level, mob),
            Goal::TurtleTravel(goal) => goal.start(level, mob),
            Goal::TurtleRandomStroll(goal) => goal.random_stroll_goal.start(level, mob),
        }
    }

    pub fn stop(&mut self, level: &mut ServerLevel, mob: EntityId) {
        match self {
            Goal::Float(_) | Goal::RandomLookAround(_) | Goal::FleeSun(_) | Goal::StriderGoToLava(_) => {}
            Goal::Panic(goal) => goal.stop(),
            Goal::Breed(goal) => goal.stop(),
            Goal::Tempt(goal) => goal.stop(level, mob),
            Goal::FollowParent(goal) => goal.stop(),
            Goal::EatBlock(goal) => goal.stop(),
            Goal::WaterAvoidingRandomStroll(goal) => goal.random_stroll_goal.stop(level, mob),
            Goal::LookAtPlayer(goal) | Goal::Interact(goal) => goal.stop(),
            Goal::PhantomAttackStrategy(goal) => goal.stop(level, mob),
            Goal::PhantomSweepAttack(goal) => goal.stop(level, mob),
            Goal::PhantomCircleAroundAnchor(_) | Goal::PhantomAttackPlayerTarget(_) => {}
            Goal::GuardianAttack(goal) => goal.stop(level, mob),
            Goal::LlamaFollowCaravan(goal) => goal.stop(level, mob),
            Goal::LlamaHurtByTarget(goal) => goal.target_goal.stop(level, mob),
            Goal::TraderLlamaDefendWanderingTarget(goal) => goal.target_goal.stop(level, mob),
            Goal::UseItem(goal) => goal.stop(level, mob),
            Goal::TradeWithPlayer(goal) => goal.stop(level, mob),
            Goal::LookAtTradingPlayer(goal) => goal.look_at_player_goal.stop(),
            Goal::WanderToPosition(goal) => goal.stop(level, mob),
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
            Goal::RangedCrossbowAttack(goal) => goal.stop(level, mob),
            Goal::SpearUse(goal) => goal.stop(level, mob),
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
            Goal::PathfindToRaid(_) | Goal::RaiderMoveThroughVillage(_) => {}
            Goal::HoldGroundAttack(goal) => goal.stop(level, mob),
            Goal::RaiderOpenDoor(goal) => goal.open_door_goal.stop(level, mob),
            Goal::VindicatorBreakDoor(goal) => goal.break_door_goal.stop(level, mob),
            Goal::VindicatorJohnnyAttack(goal) => goal.nearest_attackable_target_goal.target_goal.stop(level, mob),
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
            Goal::MoveTowardsTarget(goal) => goal.stop(),
            Goal::MoveBackToVillage(goal) => goal.random_stroll_goal.stop(level, mob),
            Goal::GolemRandomStrollInVillage(goal) => goal.random_stroll_goal.stop(level, mob),
            Goal::OfferFlower(goal) => goal.stop(level, mob),
            Goal::DefendVillageTarget(goal) => goal.target_goal.stop(level, mob),
            Goal::FishSwim(goal) => goal.random_swimming_goal.random_stroll_goal.stop(level, mob),
            Goal::FollowFlockLeader(goal) => goal.stop(level, mob),
            Goal::PufferfishPuff(goal) => goal.stop(level, mob),
            Goal::CatAvoidEntity(goal) => goal.avoid_entity_goal.stop(),
            Goal::CatRelaxOnOwner(goal) => goal.stop(level, mob),
            Goal::CatTempt(goal) => goal.tempt_goal.stop(level, mob),
            Goal::CatLieOnBlock(goal) => goal.stop(level, mob),
            Goal::CatSitOnBlock(goal) => goal.stop(level, mob),
            Goal::OcelotAttack(goal) => goal.stop(level, mob),
            Goal::OcelotAvoidEntity(goal) => goal.avoid_entity_goal.stop(),
            Goal::OcelotTempt(goal) => goal.tempt_goal.stop(level, mob),
            Goal::RunAroundLikeCrazy(_) | Goal::RandomStand(_) => {}
            Goal::MountPanic(goal) => goal.panic_goal.stop(),
            Goal::FoxFloat(_) | Goal::SeekShelter(_) | Goal::FoxStrollThroughVillage(_) | Goal::FoxEatBerries(_) | Goal::FoxSearchForItems(_) => {}
            Goal::Faceplant(goal) => goal.stop(level, mob),
            Goal::FoxPanic(goal) => goal.panic_goal.stop(),
            Goal::FoxBreed(goal) => goal.breed_goal.stop(),
            Goal::StalkPrey(goal) => goal.stop(level, mob),
            Goal::FoxPounce(goal) => goal.stop(level, mob),
            Goal::FoxMeleeAttack(goal) => goal.melee_attack_goal.stop(level, mob),
            Goal::Sleep(goal) => goal.stop(level, mob),
            Goal::FoxFollowParent(goal) => goal.follow_parent_goal.stop(),
            Goal::FoxLookAtPlayer(goal) => goal.look_at_player_goal.stop(),
            Goal::PerchAndSearch(goal) => goal.stop(level, mob),
            Goal::DefendTrustedTarget(goal) => goal.nearest_attackable_target_goal.target_goal.stop(level, mob),
            Goal::MoveTowardsRestriction(_) | Goal::RandomFloatAround(_) | Goal::GhastLook => {}
            Goal::EvokerCastingSpell(goal) => goal.casting_spell_goal.stop(level, mob),
            Goal::SpellcasterCastingSpell(goal) => goal.stop(level, mob),
            Goal::IllusionerMirrorSpell(_) | Goal::IllusionerBlindnessSpell(_) => {}
            Goal::EvokerSummonSpell(_) | Goal::EvokerAttackSpell(_) | Goal::VexRandomMove(_) => {}
            Goal::EvokerWololoSpell(goal) => goal.stop(level, mob),
            Goal::VexChargeAttack(goal) => goal.stop(level, mob),
            Goal::VexCopyOwnerTarget(goal) => goal.target_goal.stop(level, mob),
            Goal::SkeletonTrap(_) => {}
            Goal::PolarBearMeleeAttack(goal) => goal.stop(level, mob),
            Goal::PolarBearHurtByTarget(goal) => goal.hurt_by_target_goal.target_goal.stop(level, mob),
            Goal::PandaPanic(goal) => goal.panic_goal.stop(),
            Goal::PandaBreed(goal) => goal.breed_goal.stop(),
            Goal::PandaAttack(goal) => goal.melee_attack_goal.stop(level, mob),
            Goal::PandaAvoid(goal) => goal.avoid_entity_goal.stop(),
            Goal::PandaSit(goal) => goal.stop(level, mob),
            Goal::PandaLieOnBack(goal) => goal.stop(level, mob),
            Goal::PandaSneeze(_) | Goal::PandaRoll(_) => {}
            Goal::PandaLookAtPlayer(goal) => goal.look_at_player_goal.stop(),
            Goal::PandaHurtByTarget(goal) => goal.hurt_by_target_goal.target_goal.stop(level, mob),
            Goal::PolarBearAttackPlayers(goal) => goal.nearest_attackable_target_goal.target_goal.stop(level, mob),
            Goal::BlazeAttack(goal) => goal.stop(level, mob),
            Goal::GhastShootFireball(goal) => goal.stop(level, mob),
            Goal::RabbitPanic(goal) => goal.panic_goal.stop(),
            Goal::RabbitAvoidEntity(goal) => goal.avoid_entity_goal.stop(),
            Goal::RaidGarden(_) | Goal::TurtleLayEgg(_) | Goal::TurtleGoToWater(_) => {}
            Goal::TurtlePanic(goal) => goal.panic_goal.stop(),
            Goal::TurtleBreed(goal) => goal.breed_goal.stop(),
            Goal::TurtleGoHome(goal) => goal.stop(level, mob),
            Goal::TurtleTravel(goal) => goal.stop(level, mob),
            Goal::TurtleRandomStroll(goal) => goal.random_stroll_goal.stop(level, mob),
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
                | Goal::RangedCrossbowAttack(_)
                | Goal::GuardianAttack(_)
                | Goal::AbstractSkeletonMelee(_)
                | Goal::DrownedTridentAttack(_)
                | Goal::DrownedAttack(_)
                | Goal::DrownedGoToBeach(_)
                | Goal::ClimbOnTopOfPowderSnow(_)
                | Goal::RangedAttack(_)
                | Goal::CubeMobAttack(_)
                | Goal::CubeMobFloat(_)
                | Goal::SquidFlee(_)
                | Goal::CatLieOnBlock(_)
                | Goal::CatSitOnBlock(_)
                | Goal::StriderGoToLava(_)
                | Goal::OcelotAttack(_)
                | Goal::RandomStand(_)
                | Goal::FoxFloat(_)
                | Goal::FoxMeleeAttack(_)
                | Goal::FoxEatBerries(_)
                | Goal::BlazeAttack(_)
                | Goal::GhastLook
                | Goal::GhastShootFireball(_)
                | Goal::VexChargeAttack(_)
                | Goal::PolarBearMeleeAttack(_)
                | Goal::PandaAttack(_)
                | Goal::RaidGarden(_)
                | Goal::TurtleLayEgg(_)
                | Goal::TurtleGoToWater(_)
                | Goal::HoldGroundAttack(_)
                | Goal::RaiderOpenDoor(_)
                | Goal::VindicatorBreakDoor(_)
        )
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let requires_update_every_tick = self.requires_update_every_tick();
        let adjusted_tick_delay = |ticks: i32| if requires_update_every_tick { ticks } else { reduced_tick_delay(ticks) };
        match self {
            Goal::Float(goal) => goal.tick(level, mob),
            Goal::Panic(_) | Goal::WaterAvoidingRandomStroll(_) | Goal::RestrictSun(_) | Goal::FleeSun(_) | Goal::NearestAttackableTarget(_) | Goal::HurtByTarget(_) => {}
            Goal::Breed(goal) => goal.tick(level, mob),
            Goal::Tempt(goal) => goal.tick(level, mob),
            Goal::FollowParent(goal) => goal.tick(level, mob),
            Goal::EatBlock(goal) => goal.tick(level, mob),
            Goal::LookAtPlayer(goal) | Goal::Interact(goal) => goal.tick(level, mob),
            Goal::PhantomAttackStrategy(goal) => goal.tick(level, mob),
            Goal::PhantomSweepAttack(goal) => goal.tick(level, mob),
            Goal::PhantomCircleAroundAnchor(goal) => goal.tick(level, mob),
            Goal::PhantomAttackPlayerTarget(_) => {}
            Goal::GuardianAttack(goal) => goal.tick(level, mob),
            Goal::LlamaFollowCaravan(goal) => goal.tick(level, mob),
            Goal::LlamaHurtByTarget(_) | Goal::TraderLlamaDefendWanderingTarget(_) | Goal::UseItem(_) | Goal::TradeWithPlayer(_) => {}
            Goal::LookAtTradingPlayer(goal) => goal.look_at_player_goal.tick(level, mob),
            Goal::WanderToPosition(goal) => goal.tick(level, mob),
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
            Goal::MoveTowardsTarget(_) | Goal::MoveBackToVillage(_) | Goal::GolemRandomStrollInVillage(_) | Goal::DefendVillageTarget(_) => {}
            Goal::OfferFlower(goal) => goal.tick(level, mob),
            Goal::CatAvoidEntity(goal) => goal.avoid_entity_goal.tick(level, mob),
            Goal::CatRelaxOnOwner(goal) => goal.tick(level, mob, adjusted_tick_delay(16)),
            Goal::CatTempt(goal) => goal.tick(level, mob, adjusted_tick_delay(600), adjusted_tick_delay(500)),
            Goal::CatLieOnBlock(goal) => goal.tick(level, mob),
            Goal::CatSitOnBlock(goal) => goal.tick(level, mob),
            Goal::StriderGoToLava(goal) => goal.tick(level, mob),
            Goal::OcelotAttack(goal) => goal.tick(level, mob),
            Goal::OcelotAvoidEntity(goal) => goal.avoid_entity_goal.tick(level, mob),
            Goal::OcelotTempt(goal) => goal.tempt_goal.tick(level, mob),
            Goal::SpiderAttack(goal) => goal.melee_attack_goal.tick(level, mob),
            Goal::RangedBowAttack(goal) => goal.tick(level, mob),
            Goal::RangedCrossbowAttack(goal) => goal.tick(level, mob),
            Goal::SpearUse(goal) => goal.tick(level, mob),
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
            Goal::PathfindToRaid(_) | Goal::RaiderMoveThroughVillage(_) | Goal::VindicatorJohnnyAttack(_) => {}
            Goal::HoldGroundAttack(goal) => goal.tick(level, mob),
            Goal::RaiderOpenDoor(goal) => goal.open_door_goal.tick(level, mob),
            Goal::VindicatorBreakDoor(goal) => goal.break_door_goal.tick(level, mob),
            Goal::NearestHealableRaiderTarget(_) | Goal::NearestAttackableWitchTarget(_) => {}
            Goal::TamableAnimalPanic(goal) => goal.tick(level, mob),
            Goal::WolfAvoidEntity(goal) => goal.tick(level, mob),
            Goal::FollowOwner(goal) => goal.tick(level, mob),
            Goal::Beg(goal) => goal.tick(level, mob),
            Goal::SitWhenOrderedTo(_) | Goal::OwnerHurtByTarget(_) | Goal::OwnerHurtTarget(_) | Goal::NonTameRandomTarget(_) => {}
            Goal::FishSwim(_) | Goal::PufferfishPuff(_) => {}
            Goal::FollowFlockLeader(goal) => goal.tick(level, mob),
            Goal::RunAroundLikeCrazy(goal) => goal.tick(level, mob),
            Goal::RandomStand(_) | Goal::MountPanic(_) => {}
            Goal::FoxFloat(goal) => goal.float_goal.tick(level, mob),
            Goal::Faceplant(goal) => goal.tick(),
            Goal::FoxPanic(_) | Goal::SeekShelter(_) | Goal::Sleep(_) | Goal::DefendTrustedTarget(_) => {}
            Goal::FoxBreed(goal) => goal.tick(level, mob),
            Goal::StalkPrey(goal) => goal.tick(level, mob),
            Goal::FoxPounce(goal) => goal.tick(level, mob),
            Goal::FoxMeleeAttack(goal) => goal.tick(level, mob),
            Goal::FoxFollowParent(goal) => goal.follow_parent_goal.tick(level, mob),
            Goal::FoxStrollThroughVillage(goal) => goal.stroll_through_village_goal.tick(level, mob),
            Goal::FoxEatBerries(goal) => goal.tick(level, mob),
            Goal::FoxSearchForItems(goal) => goal.tick(level, mob),
            Goal::FoxLookAtPlayer(goal) => goal.look_at_player_goal.tick(level, mob),
            Goal::PerchAndSearch(goal) => goal.tick(level, mob),
            Goal::MoveTowardsRestriction(_) | Goal::RandomFloatAround(_) => {}
            Goal::BlazeAttack(goal) => goal.tick(level, mob),
            Goal::GhastLook => ghast::ghast_look_goal_tick(level, mob),
            Goal::EvokerCastingSpell(goal) => goal.tick(level, mob),
            Goal::SpellcasterCastingSpell(goal) => goal.tick(level, mob),
            Goal::IllusionerMirrorSpell(goal) => goal.tick(level, mob),
            Goal::IllusionerBlindnessSpell(goal) => goal.tick(level, mob),
            Goal::EvokerSummonSpell(goal) => goal.tick(level, mob),
            Goal::EvokerAttackSpell(goal) => goal.tick(level, mob),
            Goal::EvokerWololoSpell(goal) => goal.tick(level, mob),
            Goal::VexChargeAttack(goal) => goal.tick(level, mob),
            Goal::VexRandomMove(goal) => goal.tick(level, mob),
            Goal::VexCopyOwnerTarget(_) => {}
            Goal::SkeletonTrap(goal) => goal.tick(level, mob),
            Goal::PolarBearMeleeAttack(goal) => goal.tick(level, mob),
            Goal::PolarBearHurtByTarget(_) | Goal::PolarBearAttackPlayers(_) => {}
            Goal::PandaPanic(_) => {}
            Goal::PandaBreed(goal) => goal.breed_goal.tick(level, mob),
            Goal::PandaAttack(goal) => goal.melee_attack_goal.tick(level, mob),
            Goal::PandaAvoid(goal) => goal.avoid_entity_goal.tick(level, mob),
            Goal::PandaSit(goal) => goal.tick(level, mob),
            Goal::PandaLieOnBack(_) | Goal::PandaSneeze(_) | Goal::PandaRoll(_) | Goal::PandaHurtByTarget(_) => {}
            Goal::PandaLookAtPlayer(goal) => goal.tick(level, mob),
            Goal::GhastShootFireball(goal) => goal.tick(level, mob),
            Goal::RabbitPanic(goal) => goal.tick(level, mob),
            Goal::RabbitAvoidEntity(goal) => goal.avoid_entity_goal.tick(level, mob),
            Goal::RaidGarden(goal) => goal.tick(level, mob),
            Goal::TurtlePanic(_) | Goal::TurtleRandomStroll(_) => {}
            Goal::TurtleBreed(goal) => goal.tick(level, mob),
            Goal::TurtleLayEgg(goal) => goal.tick(level, mob, adjusted_tick_delay(200)),
            Goal::TurtleGoToWater(goal) => goal.tick(level, mob),
            Goal::TurtleGoHome(goal) => goal.tick(level, mob),
            Goal::TurtleTravel(goal) => goal.tick(level, mob),
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
            | Goal::DrownedGoToWater(_) => Flag::Move.bit(),
            Goal::RandomStroll(goal) => goal.flags,
            Goal::MoveTowardsRestriction(goal) => goal.flags,
            Goal::GuardianAttack(_) => Flag::Move.bit() | Flag::Look.bit(),
            Goal::Breed(_)
            | Goal::Tempt(_)
            | Goal::RandomLookAround(_)
            | Goal::MeleeAttack(_)
            | Goal::ZombieAttack(_)
            | Goal::RangedBowAttack(_)
            | Goal::RangedCrossbowAttack(_)
            | Goal::SpearUse(_)
            | Goal::AbstractSkeletonMelee(_)
            | Goal::DrownedTridentAttack(_)
            | Goal::DrownedAttack(_) => Flag::Move.bit() | Flag::Look.bit(),
            Goal::FollowParent(_) | Goal::BreakDoor(_) | Goal::RestrictSun(_) | Goal::DrownedSwimUp(_) => 0,
            Goal::EatBlock(_) => Flag::Move.bit() | Flag::Look.bit() | Flag::Jump.bit(),
            Goal::ZombieAttackTurtleEgg(_) | Goal::DrownedGoToBeach(_) => Flag::Move.bit() | Flag::Jump.bit(),
            Goal::LookAtPlayer(_) | Goal::LookAtTradingPlayer(_) => Flag::Look.bit(),
            Goal::PhantomSweepAttack(_) | Goal::PhantomCircleAroundAnchor(_) => Flag::Move.bit(),
            Goal::PhantomAttackStrategy(_) | Goal::PhantomAttackPlayerTarget(_) => 0,
            Goal::Interact(_) => Flag::Look.bit() | Flag::Move.bit(),
            Goal::LlamaFollowCaravan(_) | Goal::WanderToPosition(_) => Flag::Move.bit(),
            Goal::LlamaHurtByTarget(_) | Goal::TraderLlamaDefendWanderingTarget(_) => Flag::Target.bit(),
            Goal::UseItem(_) => 0,
            Goal::TradeWithPlayer(_) => Flag::Jump.bit() | Flag::Move.bit(),
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
            Goal::PathfindToRaid(_) | Goal::RaiderMoveThroughVillage(_) | Goal::VindicatorBreakDoor(_) => Flag::Move.bit(),
            Goal::HoldGroundAttack(_) => Flag::Move.bit() | Flag::Look.bit(),
            Goal::RaiderOpenDoor(_) => 0,
            Goal::VindicatorJohnnyAttack(_) => Flag::Target.bit(),
            Goal::NearestHealableRaiderTarget(_) | Goal::NearestAttackableWitchTarget(_) => Flag::Target.bit(),
            Goal::CubeMobAttack(_) | Goal::CubeMobRandomDirection(_) => Flag::Look.bit(),
            Goal::MoveTowardsTarget(_) | Goal::MoveBackToVillage(_) | Goal::GolemRandomStrollInVillage(_) => Flag::Move.bit(),
            Goal::OfferFlower(_) => Flag::Move.bit() | Flag::Look.bit(),
            Goal::DefendVillageTarget(_) => Flag::Target.bit(),
            Goal::CubeMobFloat(_) | Goal::CubeMobKeepOnJumping(_) => Flag::Jump.bit() | Flag::Move.bit(),
            Goal::TamableAnimalPanic(_) | Goal::WolfAvoidEntity(_) => Flag::Move.bit(),
            Goal::SitWhenOrderedTo(_) => Flag::Jump.bit() | Flag::Move.bit(),
            Goal::FollowOwner(_) => Flag::Move.bit() | Flag::Look.bit(),
            Goal::Beg(_) => Flag::Look.bit(),
            Goal::OwnerHurtByTarget(_) | Goal::OwnerHurtTarget(_) | Goal::NonTameRandomTarget(_) => Flag::Target.bit(),
            Goal::SquidRandomMovement(_) | Goal::SquidFlee(_) => 0,
            Goal::FishSwim(_) => Flag::Move.bit(),
            Goal::FollowFlockLeader(_) | Goal::PufferfishPuff(_) => 0,
            Goal::CatAvoidEntity(_) | Goal::OcelotAvoidEntity(_) => Flag::Move.bit(),
            Goal::CatRelaxOnOwner(_) => 0,
            Goal::CatTempt(_) | Goal::OcelotTempt(_) | Goal::OcelotAttack(_) => Flag::Move.bit() | Flag::Look.bit(),
            Goal::CatLieOnBlock(_) | Goal::CatSitOnBlock(_) | Goal::StriderGoToLava(_) => Flag::Move.bit() | Flag::Jump.bit(),
            Goal::RunAroundLikeCrazy(_) | Goal::MountPanic(_) => Flag::Move.bit(),
            Goal::RandomStand(_) => 0,
            Goal::FoxFloat(_) => Flag::Jump.bit(),
            Goal::Faceplant(_) | Goal::Sleep(_) => Flag::Look.bit() | Flag::Jump.bit() | Flag::Move.bit(),
            Goal::FoxPanic(_) | Goal::SeekShelter(_) | Goal::FoxStrollThroughVillage(_) | Goal::FoxSearchForItems(_) => Flag::Move.bit(),
            Goal::FoxBreed(_) | Goal::StalkPrey(_) | Goal::FoxMeleeAttack(_) | Goal::PerchAndSearch(_) => Flag::Move.bit() | Flag::Look.bit(),
            Goal::FoxPounce(_) | Goal::FoxEatBerries(_) => Flag::Move.bit() | Flag::Jump.bit(),
            Goal::FoxFollowParent(_) => 0,
            Goal::FoxLookAtPlayer(_) => Flag::Look.bit(),
            Goal::DefendTrustedTarget(_) => Flag::Target.bit(),
            Goal::RandomFloatAround(_) => Flag::Move.bit(),
            Goal::BlazeAttack(_) => Flag::Move.bit() | Flag::Look.bit(),
            Goal::GhastLook => Flag::Look.bit(),
            Goal::EvokerCastingSpell(_) | Goal::SpellcasterCastingSpell(_) => Flag::Move.bit() | Flag::Look.bit(),
            Goal::IllusionerMirrorSpell(_) | Goal::IllusionerBlindnessSpell(_) => 0,
            Goal::EvokerSummonSpell(_) | Goal::EvokerAttackSpell(_) | Goal::EvokerWololoSpell(_) | Goal::VexCopyOwnerTarget(_) | Goal::SkeletonTrap(_) => 0,
            Goal::PolarBearMeleeAttack(_) => Flag::Move.bit() | Flag::Look.bit(),
            Goal::PolarBearHurtByTarget(_) | Goal::PolarBearAttackPlayers(_) => Flag::Target.bit(),
            Goal::PandaPanic(_) | Goal::PandaAvoid(_) | Goal::PandaSit(_) => Flag::Move.bit(),
            Goal::PandaBreed(_) | Goal::PandaAttack(_) => Flag::Move.bit() | Flag::Look.bit(),
            Goal::PandaLieOnBack(_) | Goal::PandaSneeze(_) => 0,
            Goal::PandaLookAtPlayer(_) => Flag::Look.bit(),
            Goal::PandaRoll(_) => Flag::Move.bit() | Flag::Look.bit() | Flag::Jump.bit(),
            Goal::PandaHurtByTarget(_) => Flag::Target.bit(),
            Goal::VexChargeAttack(_) | Goal::VexRandomMove(_) => Flag::Move.bit(),
            Goal::GhastShootFireball(_) => 0,
            Goal::RabbitPanic(_) | Goal::RabbitAvoidEntity(_) | Goal::TurtlePanic(_) | Goal::TurtleRandomStroll(_) => Flag::Move.bit(),
            Goal::RaidGarden(_) | Goal::TurtleLayEgg(_) | Goal::TurtleGoToWater(_) => Flag::Move.bit() | Flag::Jump.bit(),
            Goal::TurtleBreed(_) => Flag::Move.bit() | Flag::Look.bit(),
            Goal::TurtleGoHome(_) | Goal::TurtleTravel(_) => 0,
        }
    }

    pub fn adjusted_tick_delay(&self, ticks: i32) -> i32 {
        if self.requires_update_every_tick() { ticks } else { reduced_tick_delay(ticks) }
    }
}

pub fn reduced_tick_delay(ticks: i32) -> i32 {
    (ticks + 1) / 2
}
