use wafer_core::mth;
use wafer_core::random::RandomSource;
use wafer_protocol::nbt::Nbt;

use super::codec::as_f32;
use crate::core::holder_set::HolderSet;
use crate::server::level::ServerLevel;
use crate::sounds::sound_event::SoundEvent;
use crate::stats::stats;
use crate::world::InteractionHand;
use crate::world::damagesource::DamageSource;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;

pub struct BlocksAttacks {
    pub block_delay_seconds: f32,
    pub disable_cooldown_scale: f32,
    pub damage_reductions: Vec<DamageReduction>,
    pub item_damage: ItemDamageFunction,
    pub bypassed_by: Option<HolderSet>,
    pub block_sound: Option<SoundEvent>,
    pub disable_sound: Option<SoundEvent>,
}

pub struct DamageReduction {
    pub horizontal_blocking_angle: f32,
    pub type_: Option<HolderSet>,
    pub base: f32,
    pub factor: f32,
}

pub struct ItemDamageFunction {
    pub threshold: f32,
    pub base: f32,
    pub factor: f32,
}

fn damage_types(v: Option<&Nbt>) -> Option<HolderSet> {
    v.and_then(|types| HolderSet::decode("minecraft:damage_type", types).ok())
}

fn sound(v: Option<&Nbt>) -> Option<SoundEvent> {
    v.and_then(SoundEvent::from_nbt)
}

impl BlocksAttacks {
    pub fn from_nbt(v: &Nbt) -> BlocksAttacks {
        let damage_reductions = match v.get("damage_reductions") {
            Some(reductions) => reductions.as_list().iter().map(DamageReduction::from_nbt).collect(),
            None => vec![DamageReduction { horizontal_blocking_angle: 90.0, type_: None, base: 0.0, factor: 1.0 }],
        };
        BlocksAttacks {
            block_delay_seconds: v.get("block_delay_seconds").map_or(0.0, as_f32),
            disable_cooldown_scale: v.get("disable_cooldown_scale").map_or(1.0, as_f32),
            damage_reductions,
            item_damage: v.get("item_damage").map_or(ItemDamageFunction::DEFAULT, ItemDamageFunction::from_nbt),
            bypassed_by: damage_types(v.get("bypassed_by")),
            block_sound: sound(v.get("block_sound")),
            disable_sound: sound(v.get("disabled_sound")),
        }
    }

    pub fn on_blocked(&self, level: &mut ServerLevel, user: EntityId) {
        if let Some(sound) = self.block_sound.clone() {
            let e = level.entity(user);
            let (x, y, z) = (e.x(), e.y(), e.z());
            let source = level.sound_source(user);
            let pitch = 0.8 + level.random.next_float() * 0.4;
            level.play_sound_at(None, x, y, z, sound, source, 1.0, pitch);
        }
    }

    pub fn disable(&self, level: &mut ServerLevel, user: EntityId, base_seconds: f32, blocking_with: &ItemStack) {
        let cooldown_ticks = self.disable_blocking_for_ticks(base_seconds);
        if cooldown_ticks > 0 {
            if let Some(player) = level.find_player_mut(user) {
                player.cooldowns.add_cooldown(blocking_with, cooldown_ticks, &player.connection);
            }
            level.stop_using_item(user);
            if let Some(sound) = self.disable_sound.clone() {
                let e = level.entity(user);
                let (x, y, z) = (e.x(), e.y(), e.z());
                let source = level.sound_source(user);
                let pitch = 0.8 + level.random.next_float() * 0.4;
                level.play_sound_at(None, x, y, z, sound, source, 0.8, pitch);
            }
        }
    }

    pub fn hurt_blocking_item(&self, level: &mut ServerLevel, item: &ItemStack, user: EntityId, hand: InteractionHand, damage: f32) {
        if level.is_player(user) {
            level.award_stat(user, stats::ITEM_USED.get(item.get_item().0), 1);
            let item_damage = self.item_damage.apply(damage);
            if item_damage > 0 {
                level.hurt_and_break(user, hand.as_equipment_slot(), item_damage);
            }
        }
    }

    fn disable_blocking_for_ticks(&self, base_seconds: f32) -> i32 {
        let seconds = base_seconds * self.disable_cooldown_scale;
        if seconds > 0.0 { crate::util::java_round_f(seconds * 20.0) } else { 0 }
    }

    pub fn block_delay_ticks(&self) -> i32 {
        crate::util::java_round_f(self.block_delay_seconds * 20.0)
    }

    pub fn resolve_blocked_damage(&self, source: &DamageSource, dealt_damage: f32, angle: f64) -> f32 {
        let mut blocked_damage = 0.0;
        for reduction in &self.damage_reductions {
            blocked_damage += reduction.resolve(source, dealt_damage, angle);
        }
        mth::clamp_f(blocked_damage, 0.0, dealt_damage)
    }
}

impl DamageReduction {
    fn from_nbt(v: &Nbt) -> DamageReduction {
        DamageReduction {
            horizontal_blocking_angle: v.get("horizontal_blocking_angle").map_or(90.0, as_f32),
            type_: damage_types(v.get("type")),
            base: v.get("base").map_or(0.0, as_f32),
            factor: v.get("factor").map_or(0.0, as_f32),
        }
    }

    pub fn resolve(&self, source: &DamageSource, dealt_damage: f32, angle: f64) -> f32 {
        if angle > (mth::DEG_TO_RAD * self.horizontal_blocking_angle) as f64 {
            return 0.0;
        }
        if self.type_.as_ref().is_some_and(|type_| !type_.contains(source.ty.0 as i32)) {
            return 0.0;
        }
        mth::clamp_f(self.base + self.factor * dealt_damage, 0.0, dealt_damage)
    }
}

impl ItemDamageFunction {
    pub const DEFAULT: ItemDamageFunction = ItemDamageFunction { threshold: 1.0, base: 0.0, factor: 1.0 };

    fn from_nbt(v: &Nbt) -> ItemDamageFunction {
        ItemDamageFunction {
            threshold: v.get("threshold").map_or(0.0, as_f32),
            base: v.get("base").map_or(0.0, as_f32),
            factor: v.get("factor").map_or(0.0, as_f32),
        }
    }

    pub fn apply(&self, dealt_damage: f32) -> i32 {
        if dealt_damage < self.threshold { 0 } else { mth::floor((self.base + self.factor * dealt_damage) as f64) }
    }
}
