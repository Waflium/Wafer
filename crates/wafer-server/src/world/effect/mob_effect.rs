use std::borrow::Cow;

use wafer_core::random::RandomSource;
use wafer_core::{BlockPos, Direction, Vec3, mth};
use wafer_data::{MobEffect, blocks, damage_types, entity_type_tags, game_rule, mob_effects};

use crate::server::level::ServerLevel;
use crate::util::argb::Vec3f;
use crate::world::damagesource::DamageSource;
use crate::world::entity::attributes::{Attribute, AttributeMap, AttributeModifier, Operation};
use crate::world::entity::sound_event;
use crate::world::entity::{EntityId, RemovalReason};
use crate::world::item::crossbow_item::cos_from_sin;
use crate::world::level::BlockGetter;
use crate::world::level::update_flags::UPDATE_ALL;

const ANIMATION_SPAWN_COBWEB: i32 = 3018;
const INFESTED_CHANCE_TO_SPAWN: f32 = 0.1;

fn rotate_y(v: Vec3f, angle: f32) -> Vec3f {
    let sin = (angle as f64).sin() as f32;
    let cos = cos_from_sin(sin, angle);
    [v[0] * cos + v[2] * sin, v[1], -v[0] * sin + v[2] * cos]
}

pub fn add_attribute_modifiers(effect: MobEffect, attributes: &mut AttributeMap, amplifier: i32) {
    for template in effect.info().modifiers {
        if let Some(instance) = attributes.instance(Attribute::by_id(template.attribute)) {
            instance.remove_modifier(template.id);
            instance.add_permanent_modifier(AttributeModifier {
                id: Cow::Borrowed(template.id),
                amount: template.amount * (amplifier + 1) as f64,
                operation: Operation::by_id(template.operation),
            });
        }
    }
}

pub fn remove_attribute_modifiers(effect: MobEffect, attributes: &mut AttributeMap) {
    for template in effect.info().modifiers {
        if let Some(instance) = attributes.instance(Attribute::by_id(template.attribute)) {
            instance.remove_modifier(template.id);
        }
    }
}

pub fn should_apply_effect_tick_this_tick(effect: MobEffect, tick_count: i32, amplifier: i32) -> bool {
    let every = |interval: i32| {
        let interval = interval.wrapping_shr(amplifier as u32);
        if interval > 0 { tick_count % interval == 0 } else { true }
    };
    match effect {
        mob_effects::REGENERATION => every(50),
        mob_effects::POISON => every(25),
        mob_effects::WITHER => every(40),
        mob_effects::HUNGER | mob_effects::ABSORPTION | mob_effects::BAD_OMEN => true,
        mob_effects::RAID_OMEN => tick_count == 1,
        _ if effect.info().instantaneous => tick_count >= 1,
        _ => false,
    }
}

impl ServerLevel {
    pub fn apply_effect_tick(&mut self, id: EntityId, effect: MobEffect, amplifier: i32) -> bool {
        match effect {
            mob_effects::REGENERATION => {
                if self.health(id) < self.max_health(id) {
                    self.heal(id, 1.0);
                }
                true
            }
            mob_effects::POISON => {
                if self.health(id) > 1.0 {
                    self.hurt_server(id, DamageSource::new(damage_types::MAGIC), 1.0);
                }
                true
            }
            mob_effects::WITHER => {
                self.hurt_server(id, DamageSource::new(damage_types::WITHER), 1.0);
                true
            }
            mob_effects::HUNGER => {
                if self.is_player(id) {
                    self.cause_food_exhaustion(id, 0.005 * (amplifier + 1) as f32);
                }
                true
            }
            mob_effects::ABSORPTION => self.absorption_amount(id) > 0.0,
            mob_effects::BAD_OMEN => {
                if self.is_player(id) && !self.is_spectator(id) && self.difficulty != crate::world::difficulty::Difficulty::Peaceful {
                    let pos = self.entity(id).block_position();
                    if self.is_village(pos) {
                        let raid = self.get_raid_at(pos);
                        if raid.is_none_or(|raid| raid.get_raid_omen_level() < raid.get_max_raid_omen_level()) {
                            self.add_effect(id, crate::world::effect::MobEffectInstance::new(mob_effects::RAID_OMEN, 600, amplifier), None);
                            self.player_mut(id).raid_omen_position = Some(pos);
                            return false;
                        }
                    }
                }
                true
            }
            mob_effects::RAID_OMEN => {
                if self.is_player(id)
                    && !self.is_spectator(id)
                    && let Some(raid_omen_position) = self.player(id).raid_omen_position
                {
                    self.create_or_extend_raid(id, raid_omen_position);
                    self.player_mut(id).raid_omen_position = None;
                    return false;
                }
                true
            }
            mob_effects::SATURATION => {
                if let Some(player) = self.find_player_mut(id) {
                    player.food_data.eat_with_modifier(amplifier + 1, 1.0);
                }
                true
            }
            mob_effects::INSTANT_HEALTH | mob_effects::INSTANT_DAMAGE => {
                let is_harm = effect == mob_effects::INSTANT_DAMAGE;
                if is_harm == self.is_inverted_heal_and_harm(id) {
                    self.heal(id, 4i32.wrapping_shl(amplifier as u32).max(0) as f32);
                } else {
                    self.hurt_server(id, DamageSource::new(damage_types::MAGIC), 6i32.wrapping_shl(amplifier as u32) as f32);
                }
                true
            }
            _ => true,
        }
    }

    pub fn apply_instantaneous_effect(&mut self, effect: MobEffect, source: Option<EntityId>, owner: Option<EntityId>, mob: EntityId, amplification: i32, scale: f64) {
        match effect {
            mob_effects::INSTANT_HEALTH | mob_effects::INSTANT_DAMAGE => {
                let is_harm = effect == mob_effects::INSTANT_DAMAGE;
                if is_harm == self.is_inverted_heal_and_harm(mob) {
                    let amount = (scale * 4i32.wrapping_shl(amplification as u32) as f64 + 0.5) as i32;
                    self.heal(mob, amount as f32);
                } else {
                    let amount = (scale * 6i32.wrapping_shl(amplification as u32) as f64 + 0.5) as i32;
                    let damage_source = match source {
                        None => DamageSource::new(damage_types::MAGIC),
                        Some(source) => DamageSource::source(damage_types::INDIRECT_MAGIC, Some(source), owner),
                    };
                    self.hurt_server(mob, damage_source, amount as f32);
                }
            }
            _ => {
                self.apply_effect_tick(mob, effect, amplification);
            }
        }
    }

    pub fn is_inverted_heal_and_harm(&self, id: EntityId) -> bool {
        entity_type_tags::INVERTED_HEALING_AND_HARM.contains(self.entity(id).entity_type as usize)
    }

    pub fn on_effect_started(&mut self, id: EntityId, effect: MobEffect, amplifier: i32) {
        if effect == mob_effects::ABSORPTION {
            let amount = self.absorption_amount(id).max((4 * (1 + amplifier)) as f32);
            self.set_absorption_amount(id, amount);
        }
    }

    pub fn on_mob_effect_added(&mut self, id: EntityId, effect: MobEffect) {
        if let Some(sound) = effect.info().sound_on_added {
            let e = self.entity(id);
            let (x, y, z) = (e.x(), e.y(), e.z());
            self.play_sound_at(None, x, y, z, sound_event(sound), crate::server::level::SoundSource::Players, 1.0, 1.0);
        }
    }

    pub fn on_mob_hurt(&mut self, id: EntityId, effect: MobEffect, _amplifier: i32, _source: &DamageSource, _damage: f32) {
        if effect == mob_effects::INFESTED {
            self.infested_on_mob_hurt(id);
        }
    }

    fn infested_on_mob_hurt(&mut self, mob: EntityId) {
        if self.entity_mut(mob).random.next_float() <= INFESTED_CHANCE_TO_SPAWN {
            let count = mth::random_between_inclusive(&mut self.entity_mut(mob).random, 1, 2);
            for _ in 0..count {
                let e = self.entity(mob);
                let (x, y, z) = (e.x(), e.y() + e.dimensions.height as f64 / 2.0, e.z());
                self.spawn_silverfish(mob, x, y, z);
            }
        }
    }

    fn spawn_silverfish(&mut self, mob: EntityId, x: f64, y: f64, z: f64) {
        let Some(mut silverfish) = ServerLevel::create_entity("minecraft:silverfish") else { return };
        let random_angle = mth::random_between(&mut self.entity_mut(mob).random, -std::f32::consts::FRAC_PI_2, std::f32::consts::FRAC_PI_2);
        let look_angle = self.entity(mob).look_angle();
        let view_direction = rotate_y([look_angle.x as f32 * 0.3, look_angle.y as f32 * 0.3 * 1.5, look_angle.z as f32 * 0.3], random_angle);
        let y_rot = self.random.next_float() * 360.0;
        let silverfish_entity = silverfish.entity_mut();
        silverfish_entity.snap_to(x, y, z, y_rot, 0.0);
        silverfish_entity.set_delta_movement(Vec3::new(view_direction[0] as f64, view_direction[1] as f64, view_direction[2] as f64));
        let id = silverfish_entity.id;
        self.add_fresh_entity(silverfish);
        self.entity_play_sound(id, sound_event("minecraft:entity.silverfish.hurt"), 1.0, 1.0);
    }

    pub fn on_mob_removed(&mut self, id: EntityId, effect: MobEffect, reason: RemovalReason) {
        if effect == mob_effects::WEAVING && reason == RemovalReason::Killed && (self.is_player(id) || self.game_rules.get_bool(game_rule::MOB_GRIEFING)) {
            let pos = self.entity(id).block_position();
            self.spawn_cobwebs_randomly_around(id, pos);
        }
    }

    fn spawn_cobwebs_randomly_around(&mut self, id: EntityId, pos: BlockPos) {
        let random = &mut self.entity_mut(id).random;
        let cobweb_count = random.next_int_between_inclusive(2, 3) as usize;
        let mut positions: Vec<BlockPos> = Vec::new();
        for _ in 0..15 {
            let random = &mut self.entity_mut(id).random;
            let x = pos.x - 1 + random.next_int_bounded(3);
            let y = pos.y - 1 + random.next_int_bounded(3);
            let z = pos.z - 1 + random.next_int_bounded(3);
            let candidate = BlockPos::new(x, y, z);
            let below = candidate.below();
            if !positions.contains(&candidate) && self.block_state(candidate).can_be_replaced() && {
                let below_state = self.block_state(below);
                self.is_face_sturdy_full_at(below, below_state, Direction::Up)
            } {
                positions.push(candidate);
                if positions.len() >= cobweb_count {
                    break;
                }
            }
        }
        positions.sort_by_key(|p| {
            let h = (p.y.wrapping_add(p.z.wrapping_mul(31))).wrapping_mul(31).wrapping_add(p.x);
            (h ^ ((h as u32) >> 16) as i32) & 15
        });
        for pos in positions {
            self.set_block(pos, blocks::COBWEB.default_state(), UPDATE_ALL);
            self.level_event(None, ANIMATION_SPAWN_COBWEB, pos, 0);
        }
    }
}
