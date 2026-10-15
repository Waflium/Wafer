use wafer_core::{BlockPos, Vec3};

use super::nearest_visible_living_entities::NearestVisibleLivingEntities;
use super::walk_target::WalkTarget;
use crate::core::global_pos::GlobalPos;
use crate::world::damagesource::DamageSource;
use crate::world::entity::EntityId;
use crate::world::entity::ai::behavior::position_tracker::PositionTracker;
use crate::world::entity::ai::behavior::spear_attack::SpearStatus;
use crate::world::level::pathfinder::path::Path;

#[derive(Clone, Debug)]
pub enum MemoryValue {
    Unit,
    Bool(bool),
    Int(i32),
    Long(i64),
    Uuid(u128),
    Uuids(Vec<u128>),
    BlockPos(BlockPos),
    GlobalPos(GlobalPos),
    GlobalPositions(Vec<GlobalPos>),
    Vec3(Vec3),
    Entity(EntityId),
    Entities(Vec<EntityId>),
    NearestVisibleLivingEntities(NearestVisibleLivingEntities),
    WalkTarget(WalkTarget),
    PositionTracker(PositionTracker),
    Path(Path),
    DamageSource(DamageSource),
    SpearStatus(SpearStatus),
}

impl MemoryValue {
    pub fn is_empty_collection(&self) -> bool {
        match self {
            MemoryValue::Uuids(v) => v.is_empty(),
            MemoryValue::GlobalPositions(v) => v.is_empty(),
            MemoryValue::Entities(v) => v.is_empty(),
            _ => false,
        }
    }
}

pub enum EntityValue {}

pub enum EntityList {}

pub trait MemoryValueType: 'static {
    type Value: Clone;

    fn wrap(value: Self::Value) -> MemoryValue;

    fn unwrap(value: &MemoryValue) -> &Self::Value;
}

macro_rules! memory_value_type {
    ($marker:ty, $value:ty, $variant:ident) => {
        impl MemoryValueType for $marker {
            type Value = $value;

            fn wrap(value: $value) -> MemoryValue {
                MemoryValue::$variant(value)
            }

            fn unwrap(value: &MemoryValue) -> &$value {
                match value {
                    MemoryValue::$variant(value) => value,
                    _ => unreachable!("a {} memory", stringify!($variant)),
                }
            }
        }
    };
}

impl MemoryValueType for () {
    type Value = ();

    fn wrap(_value: ()) -> MemoryValue {
        MemoryValue::Unit
    }

    fn unwrap(value: &MemoryValue) -> &() {
        match value {
            MemoryValue::Unit => &(),
            _ => unreachable!("a Unit memory"),
        }
    }
}

memory_value_type!(bool, bool, Bool);
memory_value_type!(i32, i32, Int);
memory_value_type!(i64, i64, Long);
memory_value_type!(u128, u128, Uuid);
memory_value_type!(Vec<u128>, Vec<u128>, Uuids);
memory_value_type!(BlockPos, BlockPos, BlockPos);
memory_value_type!(GlobalPos, GlobalPos, GlobalPos);
memory_value_type!(Vec<GlobalPos>, Vec<GlobalPos>, GlobalPositions);
memory_value_type!(Vec3, Vec3, Vec3);
memory_value_type!(EntityValue, EntityId, Entity);
memory_value_type!(EntityList, Vec<EntityId>, Entities);
memory_value_type!(NearestVisibleLivingEntities, NearestVisibleLivingEntities, NearestVisibleLivingEntities);
memory_value_type!(WalkTarget, WalkTarget, WalkTarget);
memory_value_type!(PositionTracker, PositionTracker, PositionTracker);
memory_value_type!(Path, Path, Path);
memory_value_type!(DamageSource, DamageSource, DamageSource);
memory_value_type!(SpearStatus, SpearStatus, SpearStatus);
