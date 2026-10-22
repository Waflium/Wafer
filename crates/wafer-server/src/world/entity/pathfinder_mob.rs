use wafer_core::BlockPos;

use super::EntityId;
use super::ai::goal::goal::Goal;
use super::mob::MobKind;
use crate::server::level::ServerLevel;

pub fn get_walk_target_value() -> f32 {
    0.0
}

impl ServerLevel {
    pub fn get_walk_target_value(&self, id: EntityId, pos: BlockPos) -> f32 {
        match self.mob(id).expect("a pathfinder mob").kind {
            MobKind::MushroomCow(_) => self.mushroom_cow_get_walk_target_value(pos),
            MobKind::Zombie(_)
            | MobKind::ZombieVillager(_)
            | MobKind::Husk(_)
            | MobKind::Drowned(_)
            | MobKind::Creeper(_)
            | MobKind::Spider(_)
            | MobKind::CaveSpider(_)
            | MobKind::Skeleton(_)
            | MobKind::Stray(_)
            | MobKind::WitherSkeleton(_)
            | MobKind::Bogged(_)
            | MobKind::Endermite(_) => self.monster_get_walk_target_value(pos),
            MobKind::Silverfish(_) => self.silverfish_get_walk_target_value(pos),
            MobKind::Giant(_) => self.giant_get_walk_target_value(pos),
            MobKind::Enderman(_) => crate::world::entity::monster::enderman::Enderman::get_walk_target_value(),
            MobKind::Witch(_) | MobKind::Vindicator(_) | MobKind::Evoker(_) | MobKind::Illusioner(_) | MobKind::Vex(_) | MobKind::Ravager(_) => self.monster_get_walk_target_value(pos),
            MobKind::Pillager(_) => super::monster::illager::pillager::Pillager::get_walk_target_value(),
            MobKind::Blaze(_) | MobKind::ZombifiedPiglin(_) => self.monster_get_walk_target_value(pos),
            MobKind::Ghast(_) | MobKind::Phantom(_) => get_walk_target_value(),
            MobKind::Slime(_) | MobKind::MagmaCube(_) => get_walk_target_value(),
            MobKind::Squid(_) | MobKind::GlowSquid(_) => get_walk_target_value(),
            MobKind::IronGolem(_) | MobKind::SnowGolem(_) => get_walk_target_value(),
            MobKind::Cod(_) | MobKind::Salmon(_) | MobKind::Pufferfish(_) | MobKind::TropicalFish(_) => get_walk_target_value(),
            MobKind::Turtle(_) => self.turtle_get_walk_target_value(id, pos),
            _ => self.animal_get_walk_target_value(pos),
        }
    }

    pub fn is_path_finding(&self, id: EntityId) -> bool {
        !self.mob(id).expect("a pathfinder mob").ai.navigation.is_done()
    }

    pub fn is_pathfinder_mob(&self, id: EntityId) -> bool {
        self.mob(id).is_some_and(|mob| !matches!(mob.kind, MobKind::Ghast(_) | MobKind::Slime(_) | MobKind::MagmaCube(_) | MobKind::Bat(_) | MobKind::Phantom(_)))
    }

    pub fn is_panicking(&self, id: EntityId) -> bool {
        let mob = self.mob(id).expect("a pathfinder mob");
        let brain = &mob.living.brain;
        if !brain.is_brain_dead() && brain.has_memory_value(super::ai::memory::memory_module_type::IS_PANICKING) {
            return brain.get_memory(super::ai::memory::memory_module_type::IS_PANICKING).is_some();
        }
        mob.ai.goal_selector.get_available_goals().iter().any(|wrapped_goal| wrapped_goal.is_running() && matches!(wrapped_goal.get_goal(), Goal::Panic(_) | Goal::TamableAnimalPanic(_) | Goal::FoxPanic(_) | Goal::RabbitPanic(_) | Goal::TurtlePanic(_)))
    }
}
