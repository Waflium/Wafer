use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::behavior::block_pos_tracker::BlockPosTracker;
use crate::world::entity::ai::behavior::declarative::behavior_builder;
use crate::world::entity::ai::behavior::one_shot::OneShot;
use crate::world::entity::ai::behavior::position_tracker::PositionTracker;
use crate::world::entity::ai::memory::memory_module_type;

pub fn create() -> OneShot {
    behavior_builder::create(|i| {
        let look_target = i.registered(memory_module_type::LOOK_TARGET);
        let disturbance = i.registered(memory_module_type::DISTURBANCE_LOCATION);
        let roar_target = i.registered(memory_module_type::ROAR_TARGET);
        let _attack_target = i.absent(memory_module_type::ATTACK_TARGET);
        move |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            let target = roar_target.try_get(level, body).map(|target| level.entity(target).block_position()).or_else(|| disturbance.try_get(level, body));
            let Some(target) = target else { return false };
            look_target.set(level, body, PositionTracker::BlockPos(BlockPosTracker::new(target)));
            true
        }
    })
}
