use wafer_core::BlockPos;
use wafer_protocol::Writer;

use crate::world::item::{ItemStack, write_slot};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pose {
    Standing,
    FallFlying,
    Sleeping,
    Swimming,
    SpinAttack,
    Crouching,
    LongJumping,
    Dying,
    Croaking,
    UsingTongue,
    Sitting,
    Roaring,
    Sniffing,
    Emerging,
    Digging,
    Sliding,
    Shooting,
    Inhaling,
}

#[derive(Clone, Debug)]
pub struct ParticleOptions {
    pub particle_type: i32,
    pub color: Option<i32>,
}

impl ParticleOptions {
    fn write(&self, w: &mut Writer) {
        w.varint(self.particle_type);
        if let Some(color) = self.color {
            w.i32(color);
        }
    }
}

impl PartialEq for ParticleOptions {
    fn eq(&self, other: &ParticleOptions) -> bool {
        self.color.is_none() && other.color.is_none() && self.particle_type == other.particle_type
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum DataValue {
    Byte(i8),
    Int(i32),
    Float(f32),
    OptionalComponent(Option<wafer_protocol::nbt::Nbt>),
    String(String),
    Component(wafer_protocol::nbt::Nbt),
    ItemStack(Option<ItemStack>),
    Boolean(bool),
    BlockPos(BlockPos),
    OptionalBlockPos(Option<BlockPos>),
    Particle(crate::core::particles::particle_options::ParticleOptions),
    Particles(Vec<ParticleOptions>),
    OptionalUnsignedInt(Option<u32>),
    Pose(Pose),
    HumanoidArm(i32),
    BlockState(wafer_data::BlockState),
    CowVariant(i32),
    CowSoundVariant(i32),
    PigVariant(i32),
    PigSoundVariant(i32),
    OptionalBlockState(Option<wafer_data::BlockState>),
    ChickenVariant(i32),
    ChickenSoundVariant(i32),
    Long(i64),
    OptionalLivingEntityReference(Option<u128>),
    WolfVariant(i32),
    WolfSoundVariant(i32),
    CatVariant(i32),
    CatSoundVariant(i32),
    ArmadilloState(i32),
    FrogVariant(i32),
    VillagerData(super::npc::villager::villager_data::VillagerData),
    Vector3(wafer_core::joml::Vector3f),
    Quaternion(wafer_core::joml::Quaternionf),
    Rotations([f32; 3]),
    DyeColor(i32),
    Direction(wafer_core::Direction),
    PaintingVariant(i32),
    ResolvableProfile(wafer_protocol::nbt::Nbt),
    SnifferState(i32),
    ZombieNautilusVariant(i32),
}

impl DataValue {
    pub fn serializer_id(&self) -> i32 {
        match self {
            DataValue::Byte(_) => 0,
            DataValue::Int(_) => 1,
            DataValue::Float(_) => 3,
            DataValue::OptionalComponent(_) => 6,
            DataValue::String(_) => 4,
            DataValue::Component(_) => 5,
            DataValue::ItemStack(_) => 7,
            DataValue::Boolean(_) => 8,
            DataValue::BlockPos(_) => 10,
            DataValue::OptionalBlockPos(_) => 11,
            DataValue::Particle(_) => 16,
            DataValue::Particles(_) => 17,
            DataValue::OptionalUnsignedInt(_) => 19,
            DataValue::Pose(_) => 20,
            DataValue::HumanoidArm(_) => 42,
            DataValue::BlockState(_) => 14,
            DataValue::CowVariant(_) => 23,
            DataValue::CowSoundVariant(_) => 24,
            DataValue::PigVariant(_) => 28,
            DataValue::PigSoundVariant(_) => 29,
            DataValue::OptionalBlockState(_) => 15,
            DataValue::ChickenVariant(_) => 30,
            DataValue::ChickenSoundVariant(_) => 31,
            DataValue::Long(_) => 2,
            DataValue::OptionalLivingEntityReference(_) => 13,
            DataValue::WolfVariant(_) => 25,
            DataValue::WolfSoundVariant(_) => 26,
            DataValue::CatVariant(_) => 21,
            DataValue::CatSoundVariant(_) => 22,
            DataValue::ArmadilloState(_) => 36,
            DataValue::FrogVariant(_) => 27,
            DataValue::VillagerData(_) => 18,
            DataValue::Vector3(_) => 39,
            DataValue::Quaternion(_) => 40,
            DataValue::Rotations(_) => 9,
            DataValue::DyeColor(_) => 43,
            DataValue::Direction(_) => 12,
            DataValue::PaintingVariant(_) => 34,
            DataValue::ResolvableProfile(_) => 41,
            DataValue::SnifferState(_) => 35,
            DataValue::ZombieNautilusVariant(_) => 32,
        }
    }

    fn write(&self, w: &mut Writer) {
        match self {
            DataValue::Byte(v) => w.u8(*v as u8),
            DataValue::Int(v) => w.varint(*v),
            DataValue::Float(v) => w.f32(*v),
            DataValue::String(v) => w.string(v),
            DataValue::Component(v) => v.write(w),
            DataValue::OptionalComponent(v) => {
                w.bool(v.is_some());
                if let Some(c) = v {
                    c.write(w);
                }
            }
            DataValue::ItemStack(v) => write_slot(w, v.as_ref()),
            DataValue::Boolean(v) => w.bool(*v),
            DataValue::BlockPos(p) => w.position(p.x, p.y, p.z),
            DataValue::OptionalBlockPos(v) => {
                w.bool(v.is_some());
                if let Some(p) = v {
                    w.position(p.x, p.y, p.z);
                }
            }
            DataValue::Particle(particle) => particle.write(w),
            DataValue::Particles(list) => {
                w.varint(list.len() as i32);
                list.iter().for_each(|p| p.write(w));
            }
            DataValue::OptionalUnsignedInt(v) => w.varint(v.map_or(0, |v| v as i32 + 1)),
            DataValue::Pose(p) => w.varint(*p as i32),
            DataValue::HumanoidArm(a) => w.varint(*a),
            DataValue::BlockState(s) => w.varint(s.id() as i32),
            DataValue::CowVariant(id)
            | DataValue::CowSoundVariant(id)
            | DataValue::PigVariant(id)
            | DataValue::PigSoundVariant(id)
            | DataValue::ChickenVariant(id)
            | DataValue::ChickenSoundVariant(id) => w.varint(*id),
            DataValue::OptionalBlockState(s) => w.varint(s.map_or(0, |s| s.id() as i32)),
            DataValue::Long(v) => w.varlong(*v),
            DataValue::OptionalLivingEntityReference(v) => {
                w.bool(v.is_some());
                if let Some(uuid) = v {
                    w.uuid(*uuid);
                }
            }
            DataValue::WolfVariant(id) | DataValue::WolfSoundVariant(id) => w.varint(*id),
            DataValue::CatVariant(id) | DataValue::CatSoundVariant(id) => w.varint(*id),
            DataValue::ArmadilloState(id) => w.varint(*id),
            DataValue::FrogVariant(id) => w.varint(*id),
            DataValue::VillagerData(data) => data.write(w),
            DataValue::Vector3(v) => {
                w.f32(v.x);
                w.f32(v.y);
                w.f32(v.z);
            }
            DataValue::Direction(direction) => w.varint(direction.ordinal() as i32),
            DataValue::PaintingVariant(id) => w.varint(*id + 1),
            DataValue::DyeColor(id) => w.varint(*id),
            DataValue::SnifferState(id) => w.varint(*id),
            DataValue::ZombieNautilusVariant(id) => w.varint(*id),
            DataValue::ResolvableProfile(profile) => {
                let _ = crate::world::item::component::PROFILE.write(profile, w);
            }
            DataValue::Rotations(r) => {
                w.f32(r[0]);
                w.f32(r[1]);
                w.f32(r[2]);
            }
            DataValue::Quaternion(q) => {
                w.f32(q.x);
                w.f32(q.y);
                w.f32(q.z);
                w.f32(q.w);
            }
        }
    }
}

struct DataItem {
    value: DataValue,
    initial: DataValue,
    dirty: bool,
    set_to_default: bool,
}

pub struct SynchedEntityData {
    items: Vec<DataItem>,
    is_dirty: bool,
}

impl SynchedEntityData {
    pub fn new(defaults: Vec<DataValue>) -> SynchedEntityData {
        SynchedEntityData {
            items: defaults.into_iter().map(|v| DataItem { initial: v.clone(), value: v, dirty: false, set_to_default: true }).collect(),
            is_dirty: false,
        }
    }

    pub fn serializer_ids(&self) -> impl Iterator<Item = i32> + '_ {
        self.items.iter().map(|item| item.value.serializer_id())
    }

    pub fn get(&self, id: usize) -> &DataValue {
        &self.items[id].value
    }

    pub fn get_mut(&mut self, id: usize) -> &mut DataValue {
        &mut self.items[id].value
    }

    pub fn set_forced(&mut self, id: usize, value: DataValue) {
        let item = &mut self.items[id];
        item.set_to_default = value == item.initial;
        item.value = value;
        item.dirty = true;
        self.is_dirty = true;
    }

    pub fn set(&mut self, id: usize, value: DataValue) {
        let item = &mut self.items[id];
        let compared_by_identity = matches!(value, DataValue::ItemStack(Some(_)));
        if compared_by_identity || item.value != value {
            item.set_to_default = !compared_by_identity && value == item.initial;
            item.value = value;
            item.dirty = true;
            self.is_dirty = true;
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.is_dirty
    }

    pub fn pack_dirty(&mut self) -> Option<Vec<(usize, DataValue)>> {
        if !self.is_dirty {
            return None;
        }
        self.is_dirty = false;
        let mut out = Vec::new();
        for (id, item) in self.items.iter_mut().enumerate() {
            if item.dirty {
                item.dirty = false;
                out.push((id, item.value.clone()));
            }
        }
        Some(out)
    }

    pub fn non_default_values(&self) -> Option<Vec<(usize, DataValue)>> {
        let values: Vec<_> = self.items.iter().enumerate().filter(|(_, i)| !i.set_to_default).map(|(id, i)| (id, i.value.clone())).collect();
        (!values.is_empty()).then_some(values)
    }
}

pub fn write_values(w: &mut Writer, entity_id: i32, values: &[(usize, DataValue)]) {
    w.varint(entity_id);
    for (id, value) in values {
        w.u8(*id as u8);
        w.varint(value.serializer_id());
        value.write(w);
    }
    w.u8(0xFF);
}

pub fn entity_defaults() -> Vec<DataValue> {
    vec![
        DataValue::Byte(0),
        DataValue::Int(300),
        DataValue::OptionalComponent(None),
        DataValue::Boolean(false),
        DataValue::Boolean(false),
        DataValue::Boolean(false),
        DataValue::Pose(Pose::Standing),
        DataValue::Int(0),
    ]
}

pub fn living_entity_defaults() -> Vec<DataValue> {
    let mut values = entity_defaults();
    values.extend([
        DataValue::Byte(0),
        DataValue::Float(1.0),
        DataValue::Particles(Vec::new()),
        DataValue::Boolean(false),
        DataValue::Int(0),
        DataValue::Int(0),
        DataValue::OptionalBlockPos(None),
    ]);
    values
}

pub fn mob_defaults() -> Vec<DataValue> {
    let mut values = living_entity_defaults();
    values.push(DataValue::Byte(0));
    values
}

pub fn ageable_mob_defaults() -> Vec<DataValue> {
    let mut values = mob_defaults();
    values.extend([DataValue::Boolean(false), DataValue::Boolean(false)]);
    values
}

pub fn player_defaults() -> Vec<DataValue> {
    let mut values = living_entity_defaults();
    values.extend([
        DataValue::HumanoidArm(1),
        DataValue::Byte(0),
        DataValue::Float(0.0),
        DataValue::Int(0),
        DataValue::OptionalUnsignedInt(None),
        DataValue::OptionalUnsignedInt(None),
    ]);
    values
}

pub mod ids {
    pub const SHARED_FLAGS: usize = 0;
    pub const AIR_SUPPLY: usize = 1;
    pub const CUSTOM_NAME: usize = 2;
    pub const CUSTOM_NAME_VISIBLE: usize = 3;
    pub const SILENT: usize = 4;
    pub const NO_GRAVITY: usize = 5;
    pub const POSE: usize = 6;
    pub const TICKS_FROZEN: usize = 7;
    pub const ITEM: usize = 8;
    pub const LIVING_ENTITY_FLAGS: usize = 8;
    pub const HEALTH: usize = 9;
    pub const EFFECT_PARTICLES: usize = 10;
    pub const EFFECT_AMBIENCE: usize = 11;
    pub const ARROW_COUNT: usize = 12;
    pub const STINGER_COUNT: usize = 13;
    pub const SLEEPING_POS: usize = 14;
    pub const MOB_FLAGS: usize = 15;
    pub const AGEABLE_BABY: usize = 16;
    pub const AGE_LOCKED: usize = 17;
    pub const PLAYER_MAIN_HAND: usize = 15;
    pub const PLAYER_MODE_CUSTOMISATION: usize = 16;
    pub const PLAYER_ABSORPTION: usize = 17;
    pub const PLAYER_SCORE: usize = 18;
    pub const PLAYER_SHOULDER_PARROT_LEFT: usize = 19;
    pub const PLAYER_SHOULDER_PARROT_RIGHT: usize = 20;
}
