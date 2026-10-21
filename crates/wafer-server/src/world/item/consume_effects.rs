use wafer_protocol::nbt::Nbt;

use crate::core::holder_set::HolderSet;
use crate::sounds::sound_event::SoundEvent;
use crate::world::effect::MobEffectInstance;

#[derive(Clone, Debug)]
pub enum ConsumeEffect {
    ApplyEffects { effects: Vec<MobEffectInstance>, probability: f32 },
    RemoveEffects { effects: HolderSet },
    ClearAllEffects,
    TeleportRandomly { diameter: f32, directional_particles: bool },
    PlaySound { sound: SoundEvent },
}

impl ConsumeEffect {
    pub fn from_nbt(tag: &Nbt) -> Option<ConsumeEffect> {
        let float = |key: &str, default: f32| tag.get(key).and_then(Nbt::as_f64).map_or(default, |v| v as f32);
        Some(match crate::commands::arguments::parse_identifier(tag.get("type")?.as_str()?)?.as_str() {
            "minecraft:apply_effects" => ConsumeEffect::ApplyEffects {
                effects: tag.get("effects")?.as_list().iter().filter_map(MobEffectInstance::from_nbt).collect(),
                probability: float("probability", 1.0),
            },
            "minecraft:remove_effects" => ConsumeEffect::RemoveEffects { effects: HolderSet::decode("minecraft:mob_effect", tag.get("effects")?).ok()? },
            "minecraft:clear_all_effects" => ConsumeEffect::ClearAllEffects,
            "minecraft:teleport_randomly" => ConsumeEffect::TeleportRandomly {
                diameter: float("diameter", 16.0),
                directional_particles: tag.get("directional_particles").and_then(Nbt::as_i64).is_none_or(|v| v != 0),
            },
            "minecraft:play_sound" => ConsumeEffect::PlaySound { sound: SoundEvent::from_nbt(tag.get("sound")?)? },
            _ => return None,
        })
    }

    pub fn list_from_nbt(tag: Option<&Nbt>) -> Vec<ConsumeEffect> {
        tag.map_or_else(Vec::new, |effects| effects.as_list().iter().filter_map(ConsumeEffect::from_nbt).collect())
    }
}
