use wafer_core::random::RandomSource;
use wafer_data::{damage_type_tags, game_rule};
use wafer_protocol::nbt::Nbt;

use super::abstract_minecart::{AbstractMinecart, MinecartKind};
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::damagesource::DamageSource;
use crate::world::entity::{EntityId, LevelEntity, sound_event};
use crate::world::level::server_explosion::{ExplosionDamageCalculator, ExplosionInteraction};

const DEFAULT_EXPLOSION_POWER_BASE: f32 = 4.0;
const DEFAULT_EXPLOSION_SPEED_FACTOR: f32 = 1.0;
pub const EVENT_PRIME: u8 = 70;

pub struct MinecartTnt {
    ignition_source: Option<DamageSource>,
    fuse: i32,
    explosion_power_base: f32,
    explosion_speed_factor: f32,
}

impl Default for MinecartTnt {
    fn default() -> MinecartTnt {
        MinecartTnt { ignition_source: None, fuse: -1, explosion_power_base: DEFAULT_EXPLOSION_POWER_BASE, explosion_speed_factor: DEFAULT_EXPLOSION_SPEED_FACTOR }
    }
}

fn tnt(minecart: &AbstractMinecart) -> &MinecartTnt {
    match &minecart.kind {
        MinecartKind::Tnt(tnt) => tnt,
        _ => unreachable!("a tnt minecart"),
    }
}

fn tnt_mut(minecart: &mut AbstractMinecart) -> &mut MinecartTnt {
    match &mut minecart.kind {
        MinecartKind::Tnt(tnt) => tnt,
        _ => unreachable!("a tnt minecart"),
    }
}

fn explosion(source: EntityId, causing: Option<EntityId>) -> DamageSource {
    let ty = if causing.is_some() { wafer_data::damage_types::PLAYER_EXPLOSION } else { wafer_data::damage_types::EXPLOSION };
    DamageSource { ty, causing_entity: causing, direct_entity: Some(source), damage_source_position: None }
}

impl MinecartTnt {
    pub fn create() -> AbstractMinecart {
        AbstractMinecart::new("minecraft:tnt_minecart", MinecartKind::Tnt(MinecartTnt::default()))
    }

    pub fn get_fuse(&self) -> i32 {
        self.fuse
    }

    pub fn is_primed(&self) -> bool {
        self.fuse > -1
    }

    pub fn read_additional_save_data(minecart: &mut AbstractMinecart, input: &Nbt) {
        let tnt = tnt_mut(minecart);
        tnt.fuse = input.get("fuse").and_then(Nbt::as_i64).map_or(-1, |v| v as i32);
        tnt.explosion_power_base = input.get("explosion_power").and_then(Nbt::as_f64).map_or(DEFAULT_EXPLOSION_POWER_BASE, |v| v as f32).clamp(0.0, 128.0);
        tnt.explosion_speed_factor = input.get("explosion_speed_factor").and_then(Nbt::as_f64).map_or(DEFAULT_EXPLOSION_SPEED_FACTOR, |v| v as f32).clamp(0.0, 128.0);
    }

    pub fn add_additional_save_data(minecart: &AbstractMinecart, output: &mut Vec<(String, Nbt)>) {
        let tnt = tnt(minecart);
        output.push(("fuse".into(), Nbt::Int(tnt.fuse)));
        if tnt.explosion_power_base != DEFAULT_EXPLOSION_POWER_BASE {
            output.push(("explosion_power".into(), Nbt::Float(tnt.explosion_power_base)));
        }
        if tnt.explosion_speed_factor != DEFAULT_EXPLOSION_SPEED_FACTOR {
            output.push(("explosion_speed_factor".into(), Nbt::Float(tnt.explosion_speed_factor)));
        }
    }
}

impl ServerLevel {
    fn minecart_tnt(&self, id: EntityId) -> &MinecartTnt {
        tnt(self.minecart(id))
    }

    pub fn minecart_tnt_tick(&mut self, id: EntityId) {
        self.abstract_minecart_tick(id);
        if self.entity_ref(id).is_none() {
            return;
        }
        let tnt = tnt_mut(self.minecart_mut(id));
        if tnt.fuse > 0 {
            tnt.fuse -= 1;
        } else if tnt.fuse == 0 {
            let ignition_source = tnt.ignition_source.clone();
            let speed_sqr = self.entity(id).delta_movement.horizontal_distance_sqr();
            self.minecart_tnt_explode(id, ignition_source, speed_sqr);
        }
        if self.entity_ref(id).is_some_and(|e| e.horizontal_collision) {
            let speed_sqr = self.entity(id).delta_movement.horizontal_distance_sqr();
            if speed_sqr >= 0.01f32 as f64 {
                let ignition_source = self.minecart_tnt(id).ignition_source.clone();
                self.minecart_tnt_explode(id, ignition_source, speed_sqr);
            }
        }
    }

    pub fn minecart_tnt_hurt_server(&mut self, id: EntityId, source: DamageSource, damage: f32) -> bool {
        if let Some(projectile) = source.direct_entity.filter(|&direct| self.as_projectile(direct).is_some_and(|p| p.abstract_arrow().is_some()))
            && self.is_on_fire(projectile)
        {
            let damage_source = explosion(id, source.causing_entity);
            let length_sqr = self.entity(projectile).delta_movement.length_sqr();
            self.minecart_tnt_explode(id, Some(damage_source), length_sqr);
        }
        if self.entity_ref(id).is_none() {
            return true;
        }
        self.vehicle_entity_hurt_server(id, source, damage)
    }

    pub fn minecart_tnt_destroy(&mut self, id: EntityId, source: &DamageSource) {
        let speed_sqr = self.entity(id).delta_movement.horizontal_distance_sqr();
        if !self.damage_source_ignites_tnt(source) && !(speed_sqr >= 0.01f32 as f64) {
            let drop_item = self.vehicle_drop_item(id);
            self.destroy(id, drop_item);
        } else if self.minecart_tnt(id).fuse < 0 {
            self.minecart_tnt_prime_fuse(id, Some(source));
            let random = &mut self.entity_mut(id).random;
            let fuse = random.next_int_bounded(20) + random.next_int_bounded(20);
            tnt_mut(self.minecart_mut(id)).fuse = fuse;
        }
    }

    fn minecart_tnt_explode(&mut self, id: EntityId, damage_source: Option<DamageSource>, speed_sqr: f64) {
        if self.game_rules.get_bool(game_rule::TNT_EXPLODES) {
            let speed = speed_sqr.sqrt().min(5.0);
            let tnt = self.minecart_tnt(id);
            let (explosion_power_base, explosion_speed_factor, primed) = (tnt.explosion_power_base, tnt.explosion_speed_factor, tnt.is_primed());
            let next_double = self.entity_mut(id).random.next_double();
            let radius = (explosion_power_base as f64 + explosion_speed_factor as f64 * next_double * 1.5 * speed) as f32;
            let center = self.entity(id).position();
            self.explode_with_calculator(Some(id), None, damage_source, ExplosionDamageCalculator::TntMinecart { primed }, center, radius, false, ExplosionInteraction::Tnt);
            self.discard(id);
        } else if self.minecart_tnt(id).is_primed() {
            self.discard(id);
        }
    }

    pub fn minecart_tnt_cause_fall_damage(&mut self, id: EntityId, fall_distance: f64, damage_modifier: f32, damage_source: DamageSource) -> bool {
        if fall_distance >= 3.0 {
            let power = fall_distance / 10.0;
            let ignition_source = self.minecart_tnt(id).ignition_source.clone();
            self.minecart_tnt_explode(id, ignition_source, power * power);
        }
        if self.entity_ref(id).is_none() {
            return false;
        }
        self.entity_cause_fall_damage(id, fall_distance, damage_modifier, damage_source)
    }

    pub fn minecart_tnt_activate_minecart(&mut self, id: EntityId, state: bool) {
        if state && self.minecart_tnt(id).fuse < 0 {
            self.minecart_tnt_prime_fuse(id, None);
        }
    }

    pub fn minecart_tnt_prime_fuse(&mut self, id: EntityId, source: Option<&DamageSource>) {
        if !self.game_rules.get_bool(game_rule::TNT_EXPLODES) {
            return;
        }
        let tnt = tnt_mut(self.minecart_mut(id));
        tnt.fuse = 80;
        if let Some(source) = source
            && tnt.ignition_source.is_none()
        {
            tnt.ignition_source = Some(explosion(id, source.causing_entity));
        }
        self.broadcast_entity_event(id, EVENT_PRIME);
        if !self.entity(id).is_silent() {
            let pos = self.entity(id).position();
            self.play_sound_at(None, pos.x, pos.y, pos.z, sound_event("minecraft:entity.tnt.primed"), SoundSource::Blocks, 1.0, 1.0);
        }
    }

    pub fn minecart_tnt_should_source_destroy(&self, source: &DamageSource) -> bool {
        self.damage_source_ignites_tnt(source)
    }

    fn damage_source_ignites_tnt(&self, source: &DamageSource) -> bool {
        match source.direct_entity.filter(|&direct| matches!(self.entities.get(&direct), Some(LevelEntity::Projectile(_)))) {
            Some(projectile) => self.is_on_fire(projectile),
            None => source.is(&damage_type_tags::IS_FIRE) || source.is(&damage_type_tags::IS_EXPLOSION),
        }
    }
}
