use wafer_protocol::nbt::Nbt;

use crate::server::level::ServerLevel;
use crate::world::entity::ai::goal::goal::reduced_tick_delay;
use crate::world::entity::data::DataValue;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::raid::raider::Raider;
use crate::world::entity::{EntityId, sound_event};
use crate::world::level::storage::tag_value_input;

pub const DATA_SPELL_CASTING_ID: usize = 17;
const DEFAULT_SPELLCASTING_TICKS: i32 = 0;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum IllagerSpell {
    #[default]
    None,
    SummonVex,
    Fangs,
    Wololo,
    Disappear,
    Blindness,
}

impl IllagerSpell {
    pub fn id(self) -> i8 {
        self as i8
    }
}

#[derive(Default)]
pub struct SpellcasterIllager {
    pub raider: Raider,
    pub spell_casting_tick_count: i32,
    current_spell: IllagerSpell,
}

pub fn spellcaster_illager(mob: &Mob) -> &SpellcasterIllager {
    match &mob.kind {
        MobKind::Evoker(evoker) => &evoker.spellcaster_illager,
        MobKind::Illusioner(illusioner) => &illusioner.spellcaster_illager,
        _ => unreachable!("a spellcaster illager"),
    }
}

pub fn spellcaster_illager_mut(mob: &mut Mob) -> &mut SpellcasterIllager {
    match &mut mob.kind {
        MobKind::Evoker(evoker) => &mut evoker.spellcaster_illager,
        MobKind::Illusioner(illusioner) => &mut illusioner.spellcaster_illager,
        _ => unreachable!("a spellcaster illager"),
    }
}

impl SpellcasterIllager {
    pub fn read_additional_save_data(&mut self, input: &Nbt) {
        self.raider.read_additional_save_data(input);
        self.spell_casting_tick_count = tag_value_input::get_i64(input, "SpellTicks").map_or(DEFAULT_SPELLCASTING_TICKS, |v| v as i32);
    }

    pub fn add_additional_save_data(&self, output: &mut Vec<(String, Nbt)>) {
        self.raider.add_additional_save_data(output);
        output.push(("SpellTicks".into(), Nbt::Int(self.spell_casting_tick_count)));
    }

    pub fn is_casting_spell(&self) -> bool {
        self.spell_casting_tick_count > 0
    }

    pub fn get_current_spell(&self) -> IllagerSpell {
        self.current_spell
    }

    pub fn get_spell_casting_time(&self) -> i32 {
        self.spell_casting_tick_count
    }
}

pub fn set_is_casting_spell(mob: &mut Mob, spell: IllagerSpell) {
    spellcaster_illager_mut(mob).current_spell = spell;
    mob.entity.data.set(DATA_SPELL_CASTING_ID, DataValue::Byte(spell.id()));
}

impl ServerLevel {
    pub fn spellcaster_illager_custom_server_ai_step(&mut self, id: EntityId) {
        let spellcaster = spellcaster_illager_mut(self.mob_mut(id).expect("a spellcaster illager"));
        if spellcaster.spell_casting_tick_count > 0 {
            spellcaster.spell_casting_tick_count -= 1;
        }
    }

    pub fn get_casting_sound_event(&self, id: EntityId) -> &'static str {
        match self.mob(id).map(|mob| &mob.kind) {
            Some(MobKind::Evoker(_)) => super::evoker::Evoker::get_casting_sound_event(),
            Some(MobKind::Illusioner(_)) => super::illusioner::Illusioner::get_casting_sound_event(),
            _ => unreachable!("not a spellcaster illager"),
        }
    }
}

pub struct SpellcasterCastingSpellGoal;

impl SpellcasterCastingSpellGoal {
    pub fn can_use(&self, level: &ServerLevel, mob: EntityId) -> bool {
        spellcaster_illager(level.mob(mob).expect("a spellcaster illager")).get_spell_casting_time() > 0
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        level.mob_mut(mob).expect("a spellcaster illager").ai.navigation.stop();
    }

    pub fn stop(&mut self, level: &mut ServerLevel, mob: EntityId) {
        set_is_casting_spell(level.mob_mut(mob).expect("a spellcaster illager"), IllagerSpell::None);
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        if let Some(target) = level.get_target(mob) {
            let m = level.mob(mob).expect("a spellcaster illager");
            let (y_max, x_max) = (m.max_head_y_rot() as f32, m.get_max_head_x_rot() as f32);
            level.set_look_at_entity(mob, target, y_max, x_max);
        }
    }
}

#[derive(Default)]
pub struct SpellcasterUseSpellGoal {
    pub attack_warmup_delay: i32,
    pub next_attack_tick_count: i32,
}

impl SpellcasterUseSpellGoal {
    pub fn can_use(&self, level: &ServerLevel, mob: EntityId) -> bool {
        if !level.get_target(mob).is_some_and(|target| level.is_alive(target)) {
            return false;
        }
        let m = level.mob(mob).expect("a spellcaster illager");
        if spellcaster_illager(m).is_casting_spell() { false } else { m.entity.tick_count >= self.next_attack_tick_count }
    }

    pub fn can_continue_to_use(&self, level: &ServerLevel, mob: EntityId) -> bool {
        level.get_target(mob).is_some_and(|target| level.is_alive(target)) && self.attack_warmup_delay > 0
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId, cast_warmup_time: i32, casting_time: i32, casting_interval: i32, spell_prepare_sound: Option<&'static str>, spell: IllagerSpell) {
        self.attack_warmup_delay = reduced_tick_delay(cast_warmup_time);
        let m = level.mob_mut(mob).expect("a spellcaster illager");
        spellcaster_illager_mut(m).spell_casting_tick_count = casting_time;
        self.next_attack_tick_count = m.entity.tick_count + casting_interval;
        if let Some(spell_prepare_sound) = spell_prepare_sound {
            level.entity_play_sound(mob, sound_event(spell_prepare_sound), 1.0, 1.0);
        }
        set_is_casting_spell(level.mob_mut(mob).expect("a spellcaster illager"), spell);
    }

    pub fn tick_warmup(&mut self) -> bool {
        self.attack_warmup_delay -= 1;
        self.attack_warmup_delay == 0
    }

    pub fn play_casting_sound(level: &mut ServerLevel, mob: EntityId) {
        let sound = level.get_casting_sound_event(mob);
        level.entity_play_sound(mob, sound_event(sound), 1.0, 1.0);
    }
}
