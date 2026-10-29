use wafer_data::{registry, registry_id};
use wafer_protocol::nbt::Nbt;

use crate::network::chat::{Component, component_serialization};
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DecodeResult};
use crate::world::entity::attributes::{Attribute, MANNEQUIN_ATTRIBUTES};
use crate::world::entity::avatar::{self, DEFAULT_MAIN_HAND, HUMANOID_ARM_LEFT, HUMANOID_ARM_RIGHT};
use crate::world::entity::data::{DataValue, Pose, ids, living_entity_defaults};
use crate::world::entity::living_entity::{self, LivingEntity};
use crate::world::entity::{Entity, EntityDimensions, EntityId, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::level::storage::tag_value_input;

const DATA_PROFILE: usize = 17;
const DATA_IMMOVABLE: usize = 18;
const DATA_DESCRIPTION: usize = 19;
const MODEL_PARTS: [(&str, i8); 7] = [("cape", 1), ("jacket", 2), ("left_sleeve", 4), ("right_sleeve", 8), ("left_pants_leg", 16), ("right_pants_leg", 32), ("hat", 64)];
const ALL_LAYERS: i8 = 127;
const VALID_POSES: [(Pose, &str); 5] = [(Pose::Standing, "standing"), (Pose::Crouching, "crouching"), (Pose::Swimming, "swimming"), (Pose::FallFlying, "fall_flying"), (Pose::Sleeping, "sleeping")];
const POSE_NAMES: [&str; 18] = [
    "standing",
    "fall_flying",
    "sleeping",
    "swimming",
    "spin_attack",
    "crouching",
    "long_jumping",
    "dying",
    "croaking",
    "using_tongue",
    "sitting",
    "roaring",
    "sniffing",
    "emerging",
    "digging",
    "sliding",
    "shooting",
    "inhaling",
];
const DEFAULT_DESCRIPTION_KEY: &str = "entity.minecraft.mannequin.label";

fn default_description() -> Nbt {
    Component::translatable(DEFAULT_DESCRIPTION_KEY).to_nbt()
}

fn default_profile() -> Nbt {
    Nbt::Compound(Vec::new())
}

pub struct Mannequin {
    pub entity: Entity,
    pub living: LivingEntity,
    pub health: f32,
    pub absorption: f32,
    pub equipment: [Option<ItemStack>; 8],
    description: Nbt,
    hide_description: bool,
}

impl Mannequin {
    pub fn of_type() -> Mannequin {
        let mut data = living_entity_defaults();
        data.push(DataValue::HumanoidArm(DEFAULT_MAIN_HAND));
        data.push(DataValue::Byte(avatar::DEFAULT_MODEL_CUSTOMIZATION));
        data.push(DataValue::ResolvableProfile(default_profile()));
        data.push(DataValue::Boolean(false));
        data.push(DataValue::OptionalComponent(Some(default_description())));
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:mannequin"), avatar::STANDING_DIMENSIONS, data);
        let living = LivingEntity::new(&MANNEQUIN_ATTRIBUTES);
        let mut mannequin =
            Mannequin { entity, living, health: 1.0, absorption: 0.0, equipment: Default::default(), description: default_description(), hide_description: false };
        let max_health = mannequin.living.attributes.value(Attribute::MaxHealth) as f32;
        mannequin.set_health(max_health);
        mannequin.entity.reapply_position();
        mannequin.entity.data.set(ids::PLAYER_MODE_CUSTOMISATION, DataValue::Byte(ALL_LAYERS));
        mannequin
    }

    fn set_health(&mut self, health: f32) {
        let max_health = self.living.attributes.value(Attribute::MaxHealth) as f32;
        self.health = health.clamp(0.0, max_health);
        self.entity.data.set(ids::HEALTH, DataValue::Float(self.health));
    }

    pub fn item_by_slot(&self, slot: wafer_data::EquipmentSlot) -> Option<&ItemStack> {
        self.equipment[slot as usize].as_ref().filter(|stack| !stack.is_empty())
    }

    pub fn get_profile(&self) -> &Nbt {
        match self.entity.data.get(DATA_PROFILE) {
            DataValue::ResolvableProfile(profile) => profile,
            _ => unreachable!("a profile"),
        }
    }

    fn set_profile(&mut self, profile: Nbt) {
        self.entity.data.set(DATA_PROFILE, DataValue::ResolvableProfile(profile));
    }

    pub fn get_immovable(&self) -> bool {
        matches!(self.entity.data.get(DATA_IMMOVABLE), DataValue::Boolean(true))
    }

    fn set_immovable(&mut self, immovable: bool) {
        self.entity.data.set(DATA_IMMOVABLE, DataValue::Boolean(immovable));
    }

    fn get_description(&self) -> Option<&Nbt> {
        match self.entity.data.get(DATA_DESCRIPTION) {
            DataValue::OptionalComponent(description) => description.as_ref(),
            _ => None,
        }
    }

    fn set_description(&mut self, description: Nbt) {
        self.description = description;
        self.update_description();
    }

    fn set_hide_description(&mut self, hide_description: bool) {
        self.hide_description = hide_description;
        self.update_description();
    }

    fn update_description(&mut self) {
        let description = if self.hide_description { None } else { Some(self.description.clone()) };
        self.entity.data.set(DATA_DESCRIPTION, DataValue::OptionalComponent(description));
    }

    fn model_customisation(&self) -> i8 {
        match self.entity.data.get(ids::PLAYER_MODE_CUSTOMISATION) {
            DataValue::Byte(mask) => *mask,
            _ => 0,
        }
    }

    pub fn add_additional_save_data(&self, output: &mut Vec<(String, Nbt)>) {
        living_entity::add_additional_save_data(&self.entity, &self.living, self.health, self.absorption, &self.equipment, output);
        output.push(("profile".into(), self.get_profile().clone()));
        let mask = self.model_customisation();
        let hidden_layers = MODEL_PARTS.iter().filter(|(_, part_mask)| mask & part_mask == 0).map(|(name, _)| Nbt::String((*name).into())).collect();
        output.push(("hidden_layers".into(), Nbt::List(hidden_layers)));
        let main_hand = if avatar::get_main_arm(&self.entity) == HUMANOID_ARM_LEFT { "left" } else { "right" };
        output.push(("main_hand".into(), Nbt::String(main_hand.into())));
        if let Some((_, name)) = VALID_POSES.iter().find(|(pose, _)| *pose == self.entity.pose()) {
            output.push(("pose".into(), Nbt::String((*name).into())));
        }
        output.push(("immovable".into(), Nbt::Byte(self.get_immovable() as i8)));
        match self.get_description() {
            Some(description) => {
                if *description != default_description() {
                    output.push(("description".into(), description.clone()));
                }
            }
            None => output.push(("hide_description".into(), Nbt::Byte(1))),
        }
    }

    pub fn read_additional_save_data(&mut self, input: &Nbt) {
        let y_rot = self.entity.y_rot;
        self.entity.y_head_rot = y_rot;
        self.living.y_body_rot = y_rot;
        if living_entity::read_additional_save_data(&mut self.entity, &mut self.living, &mut self.health, &mut self.absorption, &mut self.equipment, input) {
            self.set_pose(Pose::Sleeping);
        }
        if let Some(profile) = tag_value_input::read(input, "profile", |tag| dc::PROFILE.decode_partial(tag)) {
            self.set_profile(profile);
        }
        let layers = tag_value_input::read(input, "hidden_layers", decode_hidden_layers).unwrap_or(ALL_LAYERS);
        self.entity.data.set(ids::PLAYER_MODE_CUSTOMISATION, DataValue::Byte(layers));
        let main_hand = tag_value_input::read_or_error(input, "main_hand", |tag| codec::string_enum(tag, &[("left", HUMANOID_ARM_LEFT), ("right", HUMANOID_ARM_RIGHT)])).unwrap_or(DEFAULT_MAIN_HAND);
        avatar::set_main_arm(&mut self.entity, main_hand);
        let pose = tag_value_input::read_or_error(input, "pose", decode_pose).unwrap_or(Pose::Standing);
        self.set_pose(pose);
        self.set_immovable(tag_value_input::get_boolean(input, "immovable").unwrap_or(false));
        self.set_hide_description(tag_value_input::get_boolean(input, "hide_description").unwrap_or(false));
        let description = tag_value_input::read(input, "description", component_serialization::decode_partial).map_or_else(default_description, |description| description.to_nbt());
        self.set_description(description);
    }

    pub fn apply_components_from_item_stack(&mut self, stack: &ItemStack) {
        if let Some(profile) = stack.get(dc::PROFILE) {
            self.set_profile(profile.clone());
        }
        self.entity.apply_components_from_item_stack(stack);
    }

    pub fn set_pose(&mut self, pose: Pose) {
        if self.entity.pose() != pose {
            self.entity.set_pose(pose);
            self.refresh_dimensions();
        }
    }

    pub fn refresh_dimensions(&mut self) -> EntityDimensions {
        let old = self.entity.dimensions;
        let scale = self.living.attributes.value(Attribute::Scale) as f32;
        self.entity.dimensions = avatar::get_default_dimensions(self.entity.pose()).scale(scale);
        self.entity.reapply_position();
        old
    }
}

fn decode_hidden_layers(tag: &Nbt) -> DecodeResult<i8> {
    codec::list_partial(tag, |part| codec::string_enum(part, &MODEL_PARTS).into()).map(|masks| masks.iter().fold(ALL_LAYERS, |layers, mask| layers & !mask))
}

fn decode_pose(tag: &Nbt) -> Result<Pose, String> {
    let name = codec::string(tag)?;
    if !POSE_NAMES.contains(&name) {
        return Err(format!("Unknown element name:{name}"));
    }
    VALID_POSES.iter().find(|(_, pose_name)| *pose_name == name).map(|(pose, _)| *pose).ok_or_else(|| format!("Invalid pose: {name}"))
}

impl ServerLevel {
    pub fn is_mannequin(&self, id: EntityId) -> bool {
        matches!(self.find_entity(id), Some(LevelEntity::Mannequin(_)))
    }

    pub fn mannequin(&self, id: EntityId) -> Option<&Mannequin> {
        match self.find_entity(id) {
            Some(LevelEntity::Mannequin(mannequin)) => Some(mannequin),
            _ => None,
        }
    }

    pub fn mannequin_mut(&mut self, id: EntityId) -> Option<&mut Mannequin> {
        match self.find_entity_mut(id) {
            Some(LevelEntity::Mannequin(mannequin)) => Some(mannequin),
            _ => None,
        }
    }

    pub fn tick_mannequin(&mut self, id: EntityId) {
        self.living_entity_tick(id);
    }

    pub fn mannequin_refresh_dimensions(&mut self, id: EntityId) {
        let Some(mannequin) = self.mannequin_mut(id) else { return };
        let old = mannequin.refresh_dimensions();
        let new = mannequin.entity.dimensions;
        let e = &mannequin.entity;
        let small = new.width <= 4.0 && new.height <= 4.0;
        if !e.first_tick && !e.no_physics && small && (new.width > old.width || new.height > old.height) {
            self.fudge_position_after_size_change(id, old);
        }
    }
}
