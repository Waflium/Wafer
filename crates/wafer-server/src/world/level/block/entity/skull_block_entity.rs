use wafer_protocol::nbt::Nbt;

use crate::world::item::component::{self as dc, DataComponentType};
use crate::world::level::storage::tag_value_input;

const TAG_PROFILE: &str = "profile";
const TAG_NOTE_BLOCK_SOUND: &str = "note_block_sound";
const TAG_CUSTOM_NAME: &str = "custom_name";

#[derive(Clone, Default)]
pub struct SkullBlockEntity {
    owner: Option<Nbt>,
    note_block_sound: Option<Nbt>,
    custom_name: Option<Nbt>,
}

impl SkullBlockEntity {
    pub fn save_additional(&self, output: &mut Vec<(String, Nbt)>) {
        if let Some(owner) = &self.owner {
            output.push((TAG_PROFILE.into(), owner.clone()));
        }
        if let Some(note_block_sound) = &self.note_block_sound {
            output.push((TAG_NOTE_BLOCK_SOUND.into(), note_block_sound.clone()));
        }
        if let Some(custom_name) = &self.custom_name {
            output.push((TAG_CUSTOM_NAME.into(), custom_name.clone()));
        }
    }

    pub fn load_additional(&mut self, input: &Nbt) {
        self.owner = tag_value_input::read_or_error(input, TAG_PROFILE, |profile| dc::PROFILE.decode(profile));
        self.note_block_sound = tag_value_input::read_or_error(input, TAG_NOTE_BLOCK_SOUND, |sound| dc::NOTE_BLOCK_SOUND.decode(sound));
        self.custom_name = tag_value_input::read_or_error(input, TAG_CUSTOM_NAME, |name| dc::CUSTOM_NAME.decode(name));
    }

    pub fn get_owner_profile(&self) -> Option<&Nbt> {
        self.owner.as_ref()
    }

    pub fn get_note_block_sound(&self) -> Option<&str> {
        self.note_block_sound.as_ref().and_then(Nbt::as_str)
    }

    pub fn apply_implicit_components<'a>(&mut self, get: &mut impl FnMut(DataComponentType) -> Option<&'a Nbt>) {
        self.owner = get(dc::PROFILE).cloned();
        self.note_block_sound = get(dc::NOTE_BLOCK_SOUND).cloned();
        self.custom_name = get(dc::CUSTOM_NAME).cloned();
    }

    pub fn collect_implicit_components(&self, components: &mut Vec<(DataComponentType, Option<Nbt>)>) {
        let mut set = |ty: DataComponentType, value: Option<Nbt>| {
            components.retain(|(t, _)| *t != ty);
            components.push((ty, value));
        };
        set(dc::PROFILE, self.owner.clone());
        set(dc::NOTE_BLOCK_SOUND, self.note_block_sound.clone());
        set(dc::CUSTOM_NAME, self.custom_name.clone());
    }
}
