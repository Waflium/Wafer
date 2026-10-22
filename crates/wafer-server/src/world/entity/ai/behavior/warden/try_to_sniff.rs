use crate::server::level::ServerLevel;
use crate::util::valueproviders::IntProvider;
use crate::world::entity::EntityId;
use crate::world::entity::ai::behavior::declarative::behavior_builder;
use crate::world::entity::ai::behavior::one_shot::OneShot;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::data::Pose;

const SNIFF_COOLDOWN: IntProvider = IntProvider::Uniform { min_inclusive: 100, max_inclusive: 200 };

pub fn create() -> OneShot {
    behavior_builder::create(|i| {
        let sniffing = i.registered(memory_module_type::IS_SNIFFING);
        let walk_target = i.registered(memory_module_type::WALK_TARGET);
        let cooldown = i.absent(memory_module_type::SNIFF_COOLDOWN);
        let _attackable = i.present(memory_module_type::NEAREST_ATTACKABLE);
        let _disturbance = i.absent(memory_module_type::DISTURBANCE_LOCATION);
        move |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            sniffing.set(level, body, ());
            let time_to_live = SNIFF_COOLDOWN.sample(&mut level.random);
            cooldown.set_with_expiry(level, body, (), time_to_live as i64);
            walk_target.erase(level, body);
            level.set_entity_pose(body, Pose::Sniffing);
            true
        }
    })
}
