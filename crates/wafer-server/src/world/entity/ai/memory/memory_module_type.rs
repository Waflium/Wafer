#![allow(dead_code)]

use std::marker::PhantomData;

use wafer_core::{BlockPos, Vec3};
use wafer_data::registry;
use wafer_protocol::nbt::Nbt;

use super::expirable_value::ExpirableValue;
use super::memory_value::{EntityList, EntityValue, MemoryValue};
use super::nearest_visible_living_entities::NearestVisibleLivingEntities;
use super::walk_target::WalkTarget;
use crate::core::global_pos::GlobalPos;
use crate::world::damagesource::DamageSource;
use crate::world::entity::ai::behavior::position_tracker::PositionTracker;
use crate::world::entity::ai::behavior::spear_attack::SpearStatus;
use crate::util::{codec, extra_codecs};
use crate::world::entity::entity_save::decode_uuid;
use crate::world::level::pathfinder::path::Path;
use crate::world::level::storage::uuid_ints;

pub enum Erased {}

pub struct MemoryModuleType<U> {
    id: u8,
    _value: PhantomData<fn() -> U>,
}

impl<U> Clone for MemoryModuleType<U> {
    fn clone(&self) -> MemoryModuleType<U> {
        *self
    }
}

impl<U> Copy for MemoryModuleType<U> {}

impl<U> PartialEq for MemoryModuleType<U> {
    fn eq(&self, other: &MemoryModuleType<U>) -> bool {
        self.id == other.id
    }
}

impl<U> Eq for MemoryModuleType<U> {}

impl<U> std::fmt::Debug for MemoryModuleType<U> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl<U> MemoryModuleType<U> {
    const fn new(id: u8) -> MemoryModuleType<U> {
        MemoryModuleType { id, _value: PhantomData }
    }

    pub const fn id(self) -> usize {
        self.id as usize
    }

    pub const fn erase(self) -> MemoryModuleType<Erased> {
        MemoryModuleType::new(self.id)
    }

    pub fn name(self) -> &'static str {
        registry::MEMORY_MODULE_TYPE[self.id as usize]
    }

    pub fn get_codec(self) -> Option<MemoryCodec> {
        CODECS[self.id as usize]
    }

    pub fn can_serialize(self) -> bool {
        self.get_codec().is_some()
    }
}

impl MemoryModuleType<Erased> {
    pub fn by_name(name: &str) -> Option<MemoryModuleType<Erased>> {
        registry::MEMORY_MODULE_TYPE.iter().position(|&n| n == name).map(|id| MemoryModuleType::new(id as u8))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MemoryCodec {
    GlobalPos,
    Bool,
    Long,
    Int,
    Unit,
    Uuid,
    GlobalPosList,
    GlobalPosSet,
    BlockPos,
}

impl MemoryCodec {
    fn encode_value(self, value: &MemoryValue) -> Option<Nbt> {
        Some(match (self, value) {
            (MemoryCodec::GlobalPos, MemoryValue::GlobalPos(pos)) => pos.to_nbt(),
            (MemoryCodec::Bool, MemoryValue::Bool(v)) => Nbt::Byte(*v as i8),
            (MemoryCodec::Long, MemoryValue::Long(v)) => Nbt::Long(*v),
            (MemoryCodec::Int, MemoryValue::Int(v)) => Nbt::Int(*v),
            (MemoryCodec::Unit, MemoryValue::Unit) => Nbt::Compound(Vec::new().into()),
            (MemoryCodec::Uuid, MemoryValue::Uuid(uuid)) => uuid_ints(*uuid),
            (MemoryCodec::GlobalPosList | MemoryCodec::GlobalPosSet, MemoryValue::GlobalPositions(list)) => Nbt::List(list.iter().map(GlobalPos::to_nbt).collect()),
            (MemoryCodec::BlockPos, MemoryValue::BlockPos(pos)) => Nbt::IntArray(vec![pos.x, pos.y, pos.z]),
            _ => return None,
        })
    }

    fn decode_value(self, nbt: &Nbt) -> Result<MemoryValue, String> {
        Ok(match self {
            MemoryCodec::GlobalPos => MemoryValue::GlobalPos(GlobalPos::decode(nbt)?),
            MemoryCodec::Bool => MemoryValue::Bool(codec::bool_value(nbt)?),
            MemoryCodec::Long => MemoryValue::Long(codec::long(nbt)?),
            MemoryCodec::Int => MemoryValue::Int(codec::int(nbt)?),
            MemoryCodec::Unit => MemoryValue::Unit,
            MemoryCodec::Uuid => MemoryValue::Uuid(decode_uuid(nbt).into_result()?),
            MemoryCodec::GlobalPosList => MemoryValue::GlobalPositions(decode_global_positions(nbt)?),
            MemoryCodec::GlobalPosSet => {
                let mut set: Vec<GlobalPos> = Vec::new();
                for pos in decode_global_positions(nbt)? {
                    if !set.contains(&pos) {
                        set.push(pos);
                    }
                }
                MemoryValue::GlobalPositions(set)
            }
            MemoryCodec::BlockPos => MemoryValue::BlockPos(extra_codecs::block_pos(nbt).into_result()?),
        })
    }

    pub fn encode(self, value: &ExpirableValue) -> Option<Nbt> {
        let mut fields = vec![("value".to_string(), self.encode_value(&value.value)?)];
        if let Some(time_to_live) = value.time_to_live {
            fields.push(("ttl".to_string(), Nbt::Long(time_to_live)));
        }
        Some(Nbt::Compound(fields.into()))
    }

    pub fn decode(self, nbt: &Nbt) -> Result<ExpirableValue, String> {
        codec::map(nbt)?;
        let value = codec::decode_field(nbt, "value", |value| self.decode_value(value))?;
        let time_to_live = nbt.get("ttl").and_then(|ttl| codec::long(ttl).ok());
        Ok(ExpirableValue { value, time_to_live })
    }
}

fn decode_global_positions(nbt: &Nbt) -> Result<Vec<GlobalPos>, String> {
    codec::list_partial(nbt, |pos| GlobalPos::decode(pos).into()).into_result()
}

pub const DUMMY: MemoryModuleType<()> = MemoryModuleType::new(0);
pub const HOME: MemoryModuleType<GlobalPos> = MemoryModuleType::new(1);
pub const JOB_SITE: MemoryModuleType<GlobalPos> = MemoryModuleType::new(2);
pub const POTENTIAL_JOB_SITE: MemoryModuleType<GlobalPos> = MemoryModuleType::new(3);
pub const MEETING_POINT: MemoryModuleType<GlobalPos> = MemoryModuleType::new(4);
pub const SECONDARY_JOB_SITE: MemoryModuleType<Vec<GlobalPos>> = MemoryModuleType::new(5);
pub const NEAREST_LIVING_ENTITIES: MemoryModuleType<EntityList> = MemoryModuleType::new(6);
pub const NEAREST_VISIBLE_LIVING_ENTITIES: MemoryModuleType<NearestVisibleLivingEntities> = MemoryModuleType::new(7);
pub const VISIBLE_VILLAGER_BABIES: MemoryModuleType<EntityList> = MemoryModuleType::new(8);
pub const NEAREST_PLAYERS: MemoryModuleType<EntityList> = MemoryModuleType::new(9);
pub const NEAREST_VISIBLE_PLAYER: MemoryModuleType<EntityValue> = MemoryModuleType::new(10);
pub const NEAREST_VISIBLE_ATTACKABLE_PLAYER: MemoryModuleType<EntityValue> = MemoryModuleType::new(11);
pub const NEAREST_VISIBLE_ATTACKABLE_PLAYERS: MemoryModuleType<EntityList> = MemoryModuleType::new(12);
pub const WALK_TARGET: MemoryModuleType<WalkTarget> = MemoryModuleType::new(13);
pub const LOOK_TARGET: MemoryModuleType<PositionTracker> = MemoryModuleType::new(14);
pub const ATTACK_TARGET: MemoryModuleType<EntityValue> = MemoryModuleType::new(15);
pub const ATTACK_COOLING_DOWN: MemoryModuleType<bool> = MemoryModuleType::new(16);
pub const INTERACTION_TARGET: MemoryModuleType<EntityValue> = MemoryModuleType::new(17);
pub const BREED_TARGET: MemoryModuleType<EntityValue> = MemoryModuleType::new(18);
pub const RIDE_TARGET: MemoryModuleType<EntityValue> = MemoryModuleType::new(19);
pub const PATH: MemoryModuleType<Path> = MemoryModuleType::new(20);
pub const DOORS_TO_CLOSE: MemoryModuleType<Vec<GlobalPos>> = MemoryModuleType::new(21);
pub const NEAREST_BED: MemoryModuleType<BlockPos> = MemoryModuleType::new(22);
pub const HURT_BY: MemoryModuleType<DamageSource> = MemoryModuleType::new(23);
pub const HURT_BY_ENTITY: MemoryModuleType<EntityValue> = MemoryModuleType::new(24);
pub const AVOID_TARGET: MemoryModuleType<EntityValue> = MemoryModuleType::new(25);
pub const NEAREST_HOSTILE: MemoryModuleType<EntityValue> = MemoryModuleType::new(26);
pub const NEAREST_ATTACKABLE: MemoryModuleType<EntityValue> = MemoryModuleType::new(27);
pub const HIDING_PLACE: MemoryModuleType<GlobalPos> = MemoryModuleType::new(28);
pub const HEARD_BELL_TIME: MemoryModuleType<i64> = MemoryModuleType::new(29);
pub const CANT_REACH_WALK_TARGET_SINCE: MemoryModuleType<i64> = MemoryModuleType::new(30);
pub const GOLEM_DETECTED_RECENTLY: MemoryModuleType<bool> = MemoryModuleType::new(31);
pub const DANGER_DETECTED_RECENTLY: MemoryModuleType<bool> = MemoryModuleType::new(32);
pub const LAST_SLEPT: MemoryModuleType<i64> = MemoryModuleType::new(33);
pub const LAST_WOKEN: MemoryModuleType<i64> = MemoryModuleType::new(34);
pub const LAST_WORKED_AT_POI: MemoryModuleType<i64> = MemoryModuleType::new(35);
pub const NEAREST_VISIBLE_ADULT: MemoryModuleType<EntityValue> = MemoryModuleType::new(36);
pub const NEAREST_VISIBLE_WANTED_ITEM: MemoryModuleType<EntityValue> = MemoryModuleType::new(37);
pub const NEAREST_VISIBLE_NEMESIS: MemoryModuleType<EntityValue> = MemoryModuleType::new(38);
pub const PLAY_DEAD_TICKS: MemoryModuleType<i32> = MemoryModuleType::new(39);
pub const TEMPTING_PLAYER: MemoryModuleType<EntityValue> = MemoryModuleType::new(40);
pub const TEMPTATION_COOLDOWN_TICKS: MemoryModuleType<i32> = MemoryModuleType::new(41);
pub const GAZE_COOLDOWN_TICKS: MemoryModuleType<i32> = MemoryModuleType::new(42);
pub const LONG_JUMP_COOLDOWN_TICKS: MemoryModuleType<i32> = MemoryModuleType::new(43);
pub const LONG_JUMP_MID_JUMP: MemoryModuleType<bool> = MemoryModuleType::new(44);
pub const HAS_HUNTING_COOLDOWN: MemoryModuleType<bool> = MemoryModuleType::new(45);
pub const RAM_COOLDOWN_TICKS: MemoryModuleType<i32> = MemoryModuleType::new(46);
pub const RAM_TARGET: MemoryModuleType<Vec3> = MemoryModuleType::new(47);
pub const IS_IN_WATER: MemoryModuleType<()> = MemoryModuleType::new(48);
pub const IS_PREGNANT: MemoryModuleType<()> = MemoryModuleType::new(49);
pub const IS_PANICKING: MemoryModuleType<bool> = MemoryModuleType::new(50);
pub const UNREACHABLE_TONGUE_TARGETS: MemoryModuleType<Vec<u128>> = MemoryModuleType::new(51);
pub const VISITED_BLOCK_POSITIONS: MemoryModuleType<Vec<GlobalPos>> = MemoryModuleType::new(52);
pub const UNREACHABLE_TRANSPORT_BLOCK_POSITIONS: MemoryModuleType<Vec<GlobalPos>> = MemoryModuleType::new(53);
pub const TRANSPORT_ITEMS_COOLDOWN_TICKS: MemoryModuleType<i32> = MemoryModuleType::new(54);
pub const CHARGE_COOLDOWN_TICKS: MemoryModuleType<i32> = MemoryModuleType::new(55);
pub const ATTACK_TARGET_COOLDOWN: MemoryModuleType<i32> = MemoryModuleType::new(56);
pub const SPEAR_FLEEING_TIME: MemoryModuleType<i32> = MemoryModuleType::new(57);
pub const SPEAR_FLEEING_POSITION: MemoryModuleType<Vec3> = MemoryModuleType::new(58);
pub const SPEAR_CHARGE_POSITION: MemoryModuleType<Vec3> = MemoryModuleType::new(59);
pub const SPEAR_ENGAGE_TIME: MemoryModuleType<i32> = MemoryModuleType::new(60);
pub const SPEAR_STATUS: MemoryModuleType<SpearStatus> = MemoryModuleType::new(61);
pub const ANGRY_AT: MemoryModuleType<u128> = MemoryModuleType::new(62);
pub const UNIVERSAL_ANGER: MemoryModuleType<bool> = MemoryModuleType::new(63);
pub const ADMIRING_ITEM: MemoryModuleType<bool> = MemoryModuleType::new(64);
pub const TIME_TRYING_TO_REACH_ADMIRE_ITEM: MemoryModuleType<i32> = MemoryModuleType::new(65);
pub const DISABLE_WALK_TO_ADMIRE_ITEM: MemoryModuleType<bool> = MemoryModuleType::new(66);
pub const ADMIRING_DISABLED: MemoryModuleType<bool> = MemoryModuleType::new(67);
pub const HUNTED_RECENTLY: MemoryModuleType<bool> = MemoryModuleType::new(68);
pub const CELEBRATE_LOCATION: MemoryModuleType<BlockPos> = MemoryModuleType::new(69);
pub const DANCING: MemoryModuleType<bool> = MemoryModuleType::new(70);
pub const NEAREST_VISIBLE_HUNTABLE_HOGLIN: MemoryModuleType<EntityValue> = MemoryModuleType::new(71);
pub const NEAREST_VISIBLE_BABY_HOGLIN: MemoryModuleType<EntityValue> = MemoryModuleType::new(72);
pub const NEAREST_TARGETABLE_PLAYER_NOT_WEARING_GOLD: MemoryModuleType<EntityValue> = MemoryModuleType::new(73);
pub const NEARBY_ADULT_PIGLINS: MemoryModuleType<EntityList> = MemoryModuleType::new(74);
pub const NEAREST_VISIBLE_ADULT_PIGLINS: MemoryModuleType<EntityList> = MemoryModuleType::new(75);
pub const NEAREST_VISIBLE_ADULT_HOGLINS: MemoryModuleType<EntityList> = MemoryModuleType::new(76);
pub const NEAREST_VISIBLE_ADULT_PIGLIN: MemoryModuleType<EntityValue> = MemoryModuleType::new(77);
pub const NEAREST_VISIBLE_ZOMBIFIED: MemoryModuleType<EntityValue> = MemoryModuleType::new(78);
pub const VISIBLE_ADULT_PIGLIN_COUNT: MemoryModuleType<i32> = MemoryModuleType::new(79);
pub const VISIBLE_ADULT_HOGLIN_COUNT: MemoryModuleType<i32> = MemoryModuleType::new(80);
pub const NEAREST_PLAYER_HOLDING_WANTED_ITEM: MemoryModuleType<EntityValue> = MemoryModuleType::new(81);
pub const ATE_RECENTLY: MemoryModuleType<bool> = MemoryModuleType::new(82);
pub const NEAREST_REPELLENT: MemoryModuleType<BlockPos> = MemoryModuleType::new(83);
pub const PACIFIED: MemoryModuleType<bool> = MemoryModuleType::new(84);
pub const ROAR_TARGET: MemoryModuleType<EntityValue> = MemoryModuleType::new(85);
pub const DISTURBANCE_LOCATION: MemoryModuleType<BlockPos> = MemoryModuleType::new(86);
pub const RECENT_PROJECTILE: MemoryModuleType<()> = MemoryModuleType::new(87);
pub const IS_SNIFFING: MemoryModuleType<()> = MemoryModuleType::new(88);
pub const IS_EMERGING: MemoryModuleType<()> = MemoryModuleType::new(89);
pub const ROAR_SOUND_DELAY: MemoryModuleType<()> = MemoryModuleType::new(90);
pub const DIG_COOLDOWN: MemoryModuleType<()> = MemoryModuleType::new(91);
pub const ROAR_SOUND_COOLDOWN: MemoryModuleType<()> = MemoryModuleType::new(92);
pub const SNIFF_COOLDOWN: MemoryModuleType<()> = MemoryModuleType::new(93);
pub const TOUCH_COOLDOWN: MemoryModuleType<()> = MemoryModuleType::new(94);
pub const VIBRATION_COOLDOWN: MemoryModuleType<()> = MemoryModuleType::new(95);
pub const SONIC_BOOM_COOLDOWN: MemoryModuleType<()> = MemoryModuleType::new(96);
pub const SONIC_BOOM_SOUND_COOLDOWN: MemoryModuleType<()> = MemoryModuleType::new(97);
pub const SONIC_BOOM_SOUND_DELAY: MemoryModuleType<()> = MemoryModuleType::new(98);
pub const LIKED_PLAYER: MemoryModuleType<u128> = MemoryModuleType::new(99);
pub const LIKED_NOTEBLOCK_POSITION: MemoryModuleType<GlobalPos> = MemoryModuleType::new(100);
pub const LIKED_NOTEBLOCK_COOLDOWN_TICKS: MemoryModuleType<i32> = MemoryModuleType::new(101);
pub const ITEM_PICKUP_COOLDOWN_TICKS: MemoryModuleType<i32> = MemoryModuleType::new(102);
pub const SNIFFER_EXPLORED_POSITIONS: MemoryModuleType<Vec<GlobalPos>> = MemoryModuleType::new(103);
pub const SNIFFER_SNIFFING_TARGET: MemoryModuleType<BlockPos> = MemoryModuleType::new(104);
pub const SNIFFER_DIGGING: MemoryModuleType<bool> = MemoryModuleType::new(105);
pub const SNIFFER_HAPPY: MemoryModuleType<bool> = MemoryModuleType::new(106);
pub const BREEZE_JUMP_COOLDOWN: MemoryModuleType<()> = MemoryModuleType::new(107);
pub const BREEZE_SHOOT: MemoryModuleType<()> = MemoryModuleType::new(108);
pub const BREEZE_SHOOT_CHARGING: MemoryModuleType<()> = MemoryModuleType::new(109);
pub const BREEZE_SHOOT_RECOVERING: MemoryModuleType<()> = MemoryModuleType::new(110);
pub const BREEZE_SHOOT_COOLDOWN: MemoryModuleType<()> = MemoryModuleType::new(111);
pub const BREEZE_JUMP_INHALING: MemoryModuleType<()> = MemoryModuleType::new(112);
pub const BREEZE_JUMP_TARGET: MemoryModuleType<BlockPos> = MemoryModuleType::new(113);
pub const BREEZE_LEAVING_WATER: MemoryModuleType<()> = MemoryModuleType::new(114);

const CODECS: [Option<MemoryCodec>; 115] = [
    None,
    Some(MemoryCodec::GlobalPos),
    Some(MemoryCodec::GlobalPos),
    Some(MemoryCodec::GlobalPos),
    Some(MemoryCodec::GlobalPos),
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    Some(MemoryCodec::Bool),
    Some(MemoryCodec::Bool),
    Some(MemoryCodec::Long),
    Some(MemoryCodec::Long),
    Some(MemoryCodec::Long),
    None,
    None,
    None,
    Some(MemoryCodec::Int),
    None,
    Some(MemoryCodec::Int),
    Some(MemoryCodec::Int),
    Some(MemoryCodec::Int),
    None,
    Some(MemoryCodec::Bool),
    Some(MemoryCodec::Int),
    None,
    Some(MemoryCodec::Unit),
    Some(MemoryCodec::Unit),
    Some(MemoryCodec::Bool),
    None,
    Some(MemoryCodec::GlobalPosSet),
    Some(MemoryCodec::GlobalPosSet),
    None,
    Some(MemoryCodec::Int),
    Some(MemoryCodec::Int),
    None,
    None,
    None,
    None,
    None,
    Some(MemoryCodec::Uuid),
    Some(MemoryCodec::Bool),
    Some(MemoryCodec::Bool),
    None,
    None,
    Some(MemoryCodec::Bool),
    Some(MemoryCodec::Bool),
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    Some(MemoryCodec::Unit),
    Some(MemoryCodec::Unit),
    Some(MemoryCodec::Unit),
    Some(MemoryCodec::Unit),
    Some(MemoryCodec::Unit),
    Some(MemoryCodec::Unit),
    Some(MemoryCodec::Unit),
    Some(MemoryCodec::Unit),
    Some(MemoryCodec::Unit),
    Some(MemoryCodec::Unit),
    Some(MemoryCodec::Unit),
    Some(MemoryCodec::Unit),
    Some(MemoryCodec::Uuid),
    Some(MemoryCodec::GlobalPos),
    Some(MemoryCodec::Int),
    Some(MemoryCodec::Int),
    Some(MemoryCodec::GlobalPosList),
    None,
    None,
    None,
    Some(MemoryCodec::Unit),
    Some(MemoryCodec::Unit),
    Some(MemoryCodec::Unit),
    Some(MemoryCodec::Unit),
    Some(MemoryCodec::Unit),
    Some(MemoryCodec::Unit),
    Some(MemoryCodec::BlockPos),
    Some(MemoryCodec::Unit),
];
