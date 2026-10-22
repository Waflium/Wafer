use wafer_core::BlockPos;
use wafer_core::random::LegacyRandomSource;

use crate::server::level::ServerLevel;
use crate::server::level::events;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::level::levelgen::feature::end_spike_feature::{self, EndSpikeFeature};
use crate::world::level::server_explosion::ExplosionInteraction;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DragonRespawnStage {
    Start,
    PreparingToSummonPillars,
    SummoningPillars,
    SummoningDragon,
    End,
}

impl DragonRespawnStage {
    const VALUES: [DragonRespawnStage; 5] =
        [DragonRespawnStage::Start, DragonRespawnStage::PreparingToSummonPillars, DragonRespawnStage::SummoningPillars, DragonRespawnStage::SummoningDragon, DragonRespawnStage::End];

    pub fn get_serialized_name(self) -> &'static str {
        match self {
            DragonRespawnStage::Start => "start",
            DragonRespawnStage::PreparingToSummonPillars => "preparing_to_summon_pillars",
            DragonRespawnStage::SummoningPillars => "summoning_pillars",
            DragonRespawnStage::SummoningDragon => "summoning_dragon",
            DragonRespawnStage::End => "end",
        }
    }

    pub fn by_name(name: &str) -> Option<DragonRespawnStage> {
        DragonRespawnStage::VALUES.into_iter().find(|stage| stage.get_serialized_name() == name)
    }
}

const BEAM_POS: BlockPos = BlockPos::new(0, 128, 0);

impl ServerLevel {
    fn set_crystal_beam_targets(&mut self, crystals: &[EntityId], target: Option<BlockPos>) {
        for crystal in crystals {
            if let Some(LevelEntity::EndCrystal(end_crystal)) = self.entities.get_mut(crystal) {
                end_crystal.set_beam_target(target);
            }
        }
    }

    pub fn dragon_respawn_stage_tick(&mut self, stage: DragonRespawnStage, crystals: &[EntityId], time: i32) {
        match stage {
            DragonRespawnStage::Start => {
                self.set_crystal_beam_targets(crystals, Some(BEAM_POS));
                self.dragon_fight_set_respawn_stage(DragonRespawnStage::PreparingToSummonPillars);
            }
            DragonRespawnStage::PreparingToSummonPillars => {
                if time < 100 {
                    if time == 0 || time == 50 || time == 51 || time == 52 || time >= 95 {
                        self.level_event(None, events::ANIMATION_DRAGON_SUMMON_ROAR, BEAM_POS, 0);
                    }
                } else {
                    self.dragon_fight_set_respawn_stage(DragonRespawnStage::SummoningPillars);
                }
            }
            DragonRespawnStage::SummoningPillars => {
                let start_of_beam = time % 40 == 0;
                let end_of_beam = time % 40 == 39;
                if start_of_beam || end_of_beam {
                    let spikes = end_spike_feature::get_spikes_for_level(self.seed);
                    let index = (time / 40) as usize;
                    if index < spikes.len() {
                        let spike = spikes[index];
                        if start_of_beam {
                            self.set_crystal_beam_targets(crystals, Some(BlockPos::new(spike.get_center_x(), spike.get_height() + 1, spike.get_center_z())));
                        } else {
                            let from = BlockPos::new(spike.get_center_x() - 10, spike.get_height() - 10, spike.get_center_z() - 10);
                            let to = BlockPos::new(spike.get_center_x() + 10, spike.get_height() + 10, spike.get_center_z() + 10);
                            for pos in BlockPos::between_closed(from, to) {
                                self.remove_block(pos, false);
                            }
                            let center = wafer_core::Vec3::new((spike.get_center_x() as f32 + 0.5) as f64, spike.get_height() as f64, (spike.get_center_z() as f32 + 0.5) as f64);
                            self.explode(None, None, None, center, 5.0, false, ExplosionInteraction::Block);
                            let feature = EndSpikeFeature::new(vec![spike], true, Some(BEAM_POS));
                            feature.place(self, &mut LegacyRandomSource::create(), BlockPos::new(spike.get_center_x(), 45, spike.get_center_z()));
                        }
                    } else if start_of_beam {
                        self.dragon_fight_set_respawn_stage(DragonRespawnStage::SummoningDragon);
                    }
                }
            }
            DragonRespawnStage::SummoningDragon => {
                if time >= 100 {
                    self.dragon_fight_set_respawn_stage(DragonRespawnStage::End);
                    self.dragon_fight_reset_spike_crystals();
                    for &crystal in crystals {
                        self.set_crystal_beam_targets(&[crystal], None);
                        let Some(center) = self.entity_ref(crystal).map(crate::world::entity::Entity::position) else { continue };
                        self.explode(Some(crystal), None, None, center, 6.0, false, ExplosionInteraction::None);
                        self.discard(crystal);
                    }
                } else if time >= 80 {
                    self.level_event(None, events::ANIMATION_DRAGON_SUMMON_ROAR, BEAM_POS, 0);
                } else if time == 0 {
                    self.set_crystal_beam_targets(crystals, Some(BEAM_POS));
                } else if time < 5 {
                    self.level_event(None, events::ANIMATION_DRAGON_SUMMON_ROAR, BEAM_POS, 0);
                }
            }
            DragonRespawnStage::End => {}
        }
    }
}
