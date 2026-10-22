use wafer_core::{BlockPos, Vec3, mth};
use wafer_data::mob_effects;
use wafer_protocol::nbt::Nbt;

use super::ender_dragon_phase::EnderDragonPhase;
use crate::core::particles::particle_types;
use crate::server::level::ServerLevel;
use crate::world::effect::MobEffectInstance;
use crate::world::entity::area_effect_cloud::AreaEffectCloud;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::level::BlockGetter;

const FLAME_DURATION: i32 = 200;
const SITTING_FLAME_ATTACKS_COUNT: i32 = 4;
const WARMUP_TIME: i32 = 10;

#[derive(Default)]
pub struct DragonSittingFlamingPhase {
    pub flame_ticks: i32,
    pub flame_count: i32,
    pub flame: Option<EntityId>,
}

impl DragonSittingFlamingPhase {
    pub fn begin(&mut self) {
        self.flame_ticks = 0;
        self.flame_count += 1;
    }

    pub fn reset_flame_count(&mut self) {
        self.flame_count = 0;
    }
}

impl ServerLevel {
    fn sitting_flaming(&mut self, dragon: EntityId) -> &mut DragonSittingFlamingPhase {
        &mut self.dragon_phase_manager_mut(dragon).sitting_flaming
    }

    pub fn dragon_sitting_flaming_do_server_tick(&mut self, dragon: EntityId) {
        let phase = self.sitting_flaming(dragon);
        phase.flame_ticks += 1;
        if phase.flame_ticks >= FLAME_DURATION {
            if phase.flame_count >= SITTING_FLAME_ATTACKS_COUNT {
                self.dragon_set_phase(dragon, EnderDragonPhase::Takeoff);
            } else {
                self.dragon_set_phase(dragon, EnderDragonPhase::SittingScanning);
            }
        } else if phase.flame_ticks == WARMUP_TIME {
            let head = self.entity(self.ender_dragon(dragon).head());
            let d = self.entity(dragon);
            let look = Vec3::new(head.x() - d.x(), 0.0, head.z() - d.z()).normalize();
            let radius = 5.0f32;
            let x = head.x() + look.x * radius as f64 / 2.0;
            let z = head.z() + look.z * radius as f64 / 2.0;
            let initial_y = head.y() + head.dimensions.height as f64 * 0.5;
            let mut y = initial_y;
            let mut pos = BlockPos::containing(x, y, z);
            while self.block_state(pos).is_air() {
                y -= 1.0;
                if y < 0.0 {
                    y = initial_y;
                    break;
                }
                pos = BlockPos::containing(x, y, z);
            }
            y = (mth::floor(y) + 1) as f64;
            let mut flame = AreaEffectCloud::new(x, y, z);
            flame.set_owner(Some(self.entity(dragon).uuid));
            flame.set_radius(radius);
            flame.duration = FLAME_DURATION;
            let particle = particle_types::parse("minecraft:dragon_breath", &Nbt::Compound(vec![("power".into(), Nbt::Float(1.0))])).expect("a dragon breath particle");
            flame.set_custom_particle(Some(particle));
            flame.set_potion_duration_scale(0.25);
            flame.add_effect(&MobEffectInstance::new(mob_effects::INSTANT_DAMAGE, 0, 0));
            let flame_id = flame.entity.id;
            self.add_fresh_entity(LevelEntity::AreaEffectCloud(Box::new(flame)));
            self.sitting_flaming(dragon).flame = Some(flame_id);
        }
    }

    pub fn dragon_sitting_flaming_end(&mut self, dragon: EntityId) {
        if let Some(flame) = self.sitting_flaming(dragon).flame.take()
            && self.entity_ref(flame).is_some()
        {
            self.discard(flame);
        }
    }
}
