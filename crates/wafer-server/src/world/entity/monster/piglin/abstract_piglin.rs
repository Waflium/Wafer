use wafer_data::mob_effects;
use wafer_protocol::nbt::Nbt;

use super::piglin_ai;
use crate::server::level::ServerLevel;
use crate::world::attribute::environment_attributes;
use crate::world::difficulty::Difficulty;
use crate::world::effect::MobEffectInstance;
use crate::world::entity::conversion_params::ConversionParams;
use crate::world::entity::data::DataValue;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::{Entity, EntityId, sound_event};
use crate::world::level::pathfinder::path_type::PathType;

pub const DATA_IMMUNE_TO_ZOMBIFICATION: usize = 16;
pub const CONVERSION_TIME: i32 = 300;
const DEFAULT_IMMUNE_TO_ZOMBIFICATION: bool = false;
const DEFAULT_PICK_UP_LOOT: bool = true;
const DEFAULT_TIME_IN_OVERWORLD: i32 = 0;

#[derive(Default)]
pub struct AbstractPiglin {
    pub time_in_overworld: i32,
}

pub fn abstract_piglin(mob: &Mob) -> &AbstractPiglin {
    match &mob.kind {
        MobKind::Piglin(piglin) => &piglin.abstract_piglin,
        MobKind::PiglinBrute(piglin_brute) => &piglin_brute.abstract_piglin,
        _ => unreachable!("an abstract piglin"),
    }
}

pub fn abstract_piglin_mut(mob: &mut Mob) -> &mut AbstractPiglin {
    match &mut mob.kind {
        MobKind::Piglin(piglin) => &mut piglin.abstract_piglin,
        MobKind::PiglinBrute(piglin_brute) => &mut piglin_brute.abstract_piglin,
        _ => unreachable!("an abstract piglin"),
    }
}

pub fn init(mob: &mut Mob) {
    mob.can_pick_up_loot = true;
    apply_open_doors_ability(mob);
    mob.set_pathfinding_malus(PathType::FireInNeighbor, 16.0);
    mob.set_pathfinding_malus(PathType::Fire, -1.0);
}

fn apply_open_doors_ability(mob: &mut Mob) {
    mob.ai.navigation.set_can_open_doors(true);
}

pub fn set_immune_to_zombification(entity: &mut Entity, is_immune_to_zombification: bool) {
    entity.data.set(DATA_IMMUNE_TO_ZOMBIFICATION, DataValue::Boolean(is_immune_to_zombification));
}

pub fn is_immune_to_zombification(entity: &Entity) -> bool {
    matches!(entity.data.get(DATA_IMMUNE_TO_ZOMBIFICATION), DataValue::Boolean(true))
}

pub fn add_additional_save_data(mob: &Mob, out: &mut Vec<(String, Nbt)>) {
    out.push(("IsImmuneToZombification".into(), Nbt::Byte(is_immune_to_zombification(&mob.entity) as i8)));
    out.push(("TimeInOverworld".into(), Nbt::Int(abstract_piglin(mob).time_in_overworld)));
}

pub fn read_additional_save_data(mob: &mut Mob, nbt: &Nbt) {
    use crate::world::level::block::entity::{byte_or, int_or};
    mob.can_pick_up_loot = byte_or(nbt, "CanPickUpLoot", DEFAULT_PICK_UP_LOOT as i8) != 0;
    set_immune_to_zombification(&mut mob.entity, byte_or(nbt, "IsImmuneToZombification", DEFAULT_IMMUNE_TO_ZOMBIFICATION as i8) != 0);
    abstract_piglin_mut(mob).time_in_overworld = int_or(nbt, "TimeInOverworld", DEFAULT_TIME_IN_OVERWORLD);
}

impl ServerLevel {
    pub fn is_abstract_piglin(&self, id: EntityId) -> bool {
        matches!(self.mob(id).map(|mob| &mob.kind), Some(MobKind::Piglin(_) | MobKind::PiglinBrute(_)))
    }

    pub fn abstract_piglin_can_hunt(&self, id: EntityId) -> bool {
        match self.mob(id).map(|mob| &mob.kind) {
            Some(MobKind::Piglin(_)) => self.piglin_can_hunt(id),
            _ => false,
        }
    }

    pub fn abstract_piglin_custom_server_ai_step(&mut self, id: EntityId) {
        if self.abstract_piglin_is_converting(id) {
            abstract_piglin_mut(self.mob_mut(id).expect("an abstract piglin")).time_in_overworld += 1;
        } else {
            abstract_piglin_mut(self.mob_mut(id).expect("an abstract piglin")).time_in_overworld = 0;
        }
        if abstract_piglin(self.mob(id).expect("an abstract piglin")).time_in_overworld > CONVERSION_TIME {
            if self.difficulty != Difficulty::Peaceful {
                self.abstract_piglin_play_converted_sound(id);
            }
            self.abstract_piglin_finish_conversion(id);
        }
    }

    pub fn abstract_piglin_is_converting(&self, id: EntityId) -> bool {
        let mob = self.mob(id).expect("an abstract piglin");
        !is_immune_to_zombification(&mob.entity) && !mob.is_no_ai() && self.environment_attributes().get_value(environment_attributes::PIGLINS_ZOMBIFY, mob.entity.position())
    }

    fn abstract_piglin_finish_conversion(&mut self, id: EntityId) {
        if let Some(MobKind::Piglin(_)) = self.mob(id).map(|mob| &mob.kind) {
            self.piglin_finish_conversion(id);
        }
        let params = ConversionParams::single(self, id, true, true);
        self.convert_to(id, "minecraft:zombified_piglin", params, |level, zombified| {
            level.add_effect(zombified, MobEffectInstance::new(mob_effects::NAUSEA, 200, 0), None);
        });
    }

    fn abstract_piglin_play_converted_sound(&mut self, id: EntityId) {
        let sound = match self.mob(id).map(|mob| &mob.kind) {
            Some(MobKind::Piglin(_)) => "minecraft:entity.piglin.converted_to_zombified",
            _ => "minecraft:entity.piglin_brute.converted_to_zombified",
        };
        self.make_sound(id, sound_event(sound));
    }

    pub fn abstract_piglin_play_ambient_sound(&mut self, id: EntityId) {
        if piglin_ai::is_idle(self, id) {
            self.mob_play_ambient_sound(id);
        }
    }
}
