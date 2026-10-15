use wafer_data::packets::play::clientbound;
use wafer_data::{damage_types, entity_type_tags, mob_effects};
use wafer_protocol::nbt::Nbt;

use super::abstract_fish::{AbstractFish, abstract_fish_defaults};
use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::effect::MobEffectInstance;
use crate::world::entity::ai::MobAi;
use crate::world::entity::ai::goal::goal::Goal;
use crate::world::entity::ai::targeting::targeting_conditions::TargetingConditions;
use crate::world::entity::data::DataValue;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::{EntityDimensions, EntityId, sound_event};

pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.7, 0.7).with_eye_height(0.455);
pub const PUFF_STATE: usize = 17;
pub const STATE_SMALL: i32 = 0;
pub const STATE_MID: i32 = 1;
pub const STATE_FULL: i32 = 2;
const DEFAULT_PUFF_STATE: i32 = 0;
const PUFFER_FISH_STING: u8 = 9;

fn scary_mob(target: EntityId, level: &ServerLevel) -> bool {
    if level.is_player(target) && level.player(target).game_mode.game_type.is_creative() {
        false
    } else {
        !entity_type_tags::NOT_SCARY_FOR_PUFFERFISH.contains(level.entity(target).entity_type as usize)
    }
}

fn targeting_conditions() -> TargetingConditions<'static> {
    TargetingConditions::for_non_combat().ignore_invisibility_testing().ignore_line_of_sight().selector(&scary_mob)
}

#[derive(Clone, Copy, Default)]
pub struct Pufferfish {
    pub inflate_counter: i32,
    pub deflate_timer: i32,
}

pub fn pufferfish_defaults() -> Vec<DataValue> {
    let mut values = abstract_fish_defaults();
    values.push(DataValue::Int(DEFAULT_PUFF_STATE));
    values
}

fn get_scale(state: i32) -> f32 {
    match state {
        0 => 0.5,
        1 => 0.7,
        _ => 1.0,
    }
}

fn pufferfish(mob: &Mob) -> &Pufferfish {
    match &mob.kind {
        MobKind::Pufferfish(pufferfish) => pufferfish,
        _ => unreachable!("not a pufferfish"),
    }
}

fn pufferfish_mut(mob: &mut Mob) -> &mut Pufferfish {
    match &mut mob.kind {
        MobKind::Pufferfish(pufferfish) => pufferfish,
        _ => unreachable!("not a pufferfish"),
    }
}

impl Pufferfish {
    pub fn create() -> Mob {
        let mut mob = AbstractFish::create_with("minecraft:pufferfish", DIMENSIONS, pufferfish_defaults(), MobKind::Pufferfish(Pufferfish::default()));
        mob.refresh_dimensions();
        mob
    }

    pub fn register_goals(ai: &mut MobAi) {
        AbstractFish::register_goals(ai);
        ai.goal_selector.add_goal(1, Goal::PufferfishPuff(PufferfishPuffGoal));
    }

    pub fn add_additional_save_data(mob: &Mob, out: &mut Vec<(String, Nbt)>) {
        out.push(("PuffState".into(), Nbt::Int(mob.get_puff_state())));
    }

    pub fn read_additional_save_data(mob: &mut Mob, nbt: &Nbt) {
        let puff_state = nbt.get("PuffState").and_then(Nbt::as_i64).map_or(DEFAULT_PUFF_STATE, |v| v as i32);
        mob.set_puff_state(puff_state.min(2));
    }

    pub fn get_death_sound() -> &'static str {
        "minecraft:entity.puffer_fish.death"
    }

    pub fn get_hurt_sound() -> &'static str {
        "minecraft:entity.puffer_fish.hurt"
    }

    pub fn get_flop_sound() -> &'static str {
        "minecraft:entity.puffer_fish.flop"
    }

    pub fn get_default_dimensions(mob: &Mob) -> EntityDimensions {
        DIMENSIONS.scale(get_scale(mob.get_puff_state()))
    }
}

impl Mob {
    pub fn get_puff_state(&self) -> i32 {
        match self.entity.data.get(PUFF_STATE) {
            DataValue::Int(state) => *state,
            _ => DEFAULT_PUFF_STATE,
        }
    }

    pub fn set_puff_state(&mut self, state: i32) {
        let changed = *self.entity.data.get(PUFF_STATE) != DataValue::Int(state);
        self.entity.data.set(PUFF_STATE, DataValue::Int(state));
        if changed {
            self.refresh_dimensions();
        }
    }
}

impl ServerLevel {
    pub fn set_puff_state(&mut self, id: EntityId, state: i32) {
        let mob = self.mob_mut(id).expect("a pufferfish");
        let changed = *mob.entity.data.get(PUFF_STATE) != DataValue::Int(state);
        mob.entity.data.set(PUFF_STATE, DataValue::Int(state));
        if changed {
            self.refresh_mob_dimensions(id);
        }
    }

    pub fn pufferfish_tick(&mut self, id: EntityId) {
        if self.is_alive(id) && self.is_effective_ai(id) {
            let mob = self.mob(id).expect("a pufferfish");
            let puff_state = mob.get_puff_state();
            let Pufferfish { inflate_counter, deflate_timer } = *pufferfish(mob);
            if inflate_counter > 0 {
                if puff_state == STATE_SMALL {
                    self.make_sound(id, sound_event("minecraft:entity.puffer_fish.blow_up"));
                    self.set_puff_state(id, STATE_MID);
                } else if inflate_counter > 40 && puff_state == STATE_MID {
                    self.make_sound(id, sound_event("minecraft:entity.puffer_fish.blow_up"));
                    self.set_puff_state(id, STATE_FULL);
                }
                pufferfish_mut(self.mob_mut(id).expect("a pufferfish")).inflate_counter += 1;
            } else if puff_state != STATE_SMALL {
                if deflate_timer > 60 && puff_state == STATE_FULL {
                    self.make_sound(id, sound_event("minecraft:entity.puffer_fish.blow_out"));
                    self.set_puff_state(id, STATE_MID);
                } else if deflate_timer > 100 && puff_state == STATE_MID {
                    self.make_sound(id, sound_event("minecraft:entity.puffer_fish.blow_out"));
                    self.set_puff_state(id, STATE_SMALL);
                }
                pufferfish_mut(self.mob_mut(id).expect("a pufferfish")).deflate_timer += 1;
            }
        }
        self.mob_tick(id);
    }

    pub fn pufferfish_ai_step(&mut self, id: EntityId) {
        self.abstract_fish_ai_step(id);
        if self.mob(id).is_some_and(|mob| mob.get_puff_state() > 0) && self.is_alive(id) {
            let area = self.entity(id).bounding_box().inflate_all(0.3);
            let conditions = targeting_conditions();
            let mobs: Vec<EntityId> = self.get_entities(None, &area).into_iter().filter(|&target| self.is_mob(target)).collect();
            let mobs: Vec<EntityId> = mobs.into_iter().filter(|&target| conditions.test(self, Some(id), target)).collect();
            for mob in mobs {
                if self.is_alive(mob) {
                    self.touch(id, mob);
                }
            }
        }
    }

    fn touch(&mut self, id: EntityId, mob: EntityId) {
        if self.do_teams_allow_damage(id, mob) {
            let puff_state = self.mob(id).expect("a pufferfish").get_puff_state();
            if self.hurt_server(mob, mob_attack(id), (1 + puff_state) as f32) {
                self.add_effect(mob, MobEffectInstance::new(mob_effects::POISON, 60 * puff_state, 0), Some(id));
                self.entity_play_sound(id, sound_event("minecraft:entity.puffer_fish.sting"), 1.0, 1.0);
            }
        }
    }

    pub fn pufferfish_player_touch(&mut self, id: EntityId, player: EntityId) {
        if self.do_teams_allow_damage(id, player) {
            let puff_state = self.mob(id).expect("a pufferfish").get_puff_state();
            if puff_state > 0 && self.hurt_server(player, mob_attack(id), (1 + puff_state) as f32) {
                if !self.entity(id).is_silent() {
                    self.player(player).connection.send(clientbound::GAME_EVENT, |w| {
                        w.u8(PUFFER_FISH_STING);
                        w.f32(0.0);
                    });
                }
                self.add_effect(player, MobEffectInstance::new(mob_effects::POISON, 60 * puff_state, 0), Some(id));
            }
        }
    }
}

fn mob_attack(id: EntityId) -> DamageSource {
    DamageSource { causing_entity: Some(id), direct_entity: Some(id), ..DamageSource::new(damage_types::MOB_ATTACK) }
}

pub struct PufferfishPuffGoal;

impl PufferfishPuffGoal {
    pub fn can_use(&self, level: &mut ServerLevel, fish: EntityId) -> bool {
        let area = level.entity(fish).bounding_box().inflate_all(2.0);
        let conditions = targeting_conditions();
        let entities: Vec<EntityId> = level.get_entities(None, &area).into_iter().filter(|&target| level.is_living(target)).collect();
        entities.into_iter().any(|target| conditions.test(level, Some(fish), target))
    }

    pub fn start(&self, level: &mut ServerLevel, fish: EntityId) {
        let pufferfish = pufferfish_mut(level.mob_mut(fish).expect("a pufferfish"));
        pufferfish.inflate_counter = 1;
        pufferfish.deflate_timer = 0;
    }

    pub fn stop(&self, level: &mut ServerLevel, fish: EntityId) {
        pufferfish_mut(level.mob_mut(fish).expect("a pufferfish")).inflate_counter = 0;
    }
}
