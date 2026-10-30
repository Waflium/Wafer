use wafer_core::Vec3;
use wafer_protocol::nbt::Nbt;

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::entity_save::uuid_from_nbt;
use crate::world::level::gameevent::game_event::GameEvent;
use crate::world::level::storage::uuid_ints;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct VibrationInfo {
    pub game_event: GameEvent,
    pub distance: f32,
    pub pos: Vec3,
    pub uuid: Option<u128>,
    pub projectile_owner_uuid: Option<u128>,
    pub entity: Option<EntityId>,
}

impl VibrationInfo {
    pub fn of_entity(level: &ServerLevel, game_event: GameEvent, distance: f32, pos: Vec3, entity: Option<EntityId>) -> VibrationInfo {
        let uuid = entity.and_then(|entity| level.entity_ref(entity)).map(|e| e.uuid);
        let projectile_owner_uuid = entity.filter(|&entity| level.is_projectile(entity)).and_then(|entity| level.projectile_get_owner(entity)).and_then(|owner| level.entity_ref(owner)).map(|e| e.uuid);
        VibrationInfo { game_event, distance, pos, uuid, projectile_owner_uuid, entity }
    }

    pub fn get_entity(&self, level: &ServerLevel) -> Option<EntityId> {
        self.entity.filter(|&entity| level.entity_ref(entity).is_some()).or_else(|| self.uuid.and_then(|uuid| level.entity_manager.visible_entity_storage.get(uuid)))
    }

    pub fn get_projectile_owner(&self, level: &ServerLevel) -> Option<EntityId> {
        self.get_entity(level)
            .filter(|&entity| level.is_projectile(entity))
            .and_then(|entity| level.projectile_get_owner(entity))
            .or_else(|| self.projectile_owner_uuid.and_then(|uuid| level.entity_manager.visible_entity_storage.get(uuid)))
    }

    pub fn to_nbt(&self) -> Nbt {
        let mut fields = vec![
            ("game_event".to_string(), Nbt::String(self.game_event.name().into())),
            ("distance".to_string(), Nbt::Float(self.distance)),
            ("pos".to_string(), Nbt::List(vec![Nbt::Double(self.pos.x), Nbt::Double(self.pos.y), Nbt::Double(self.pos.z)])),
        ];
        if let Some(uuid) = self.uuid {
            fields.push(("source".into(), uuid_ints(uuid)));
        }
        if let Some(uuid) = self.projectile_owner_uuid {
            fields.push(("projectile_owner".into(), uuid_ints(uuid)));
        }
        Nbt::Compound(fields.into())
    }

    pub fn from_nbt(nbt: &Nbt) -> Option<VibrationInfo> {
        let game_event = GameEvent::by_name(&crate::commands::arguments::parse_identifier(nbt.get("game_event")?.as_str()?)?)?;
        let distance = nbt.get("distance")?.as_f64()? as f32;
        if !(0.0..=f32::MAX).contains(&distance) {
            return None;
        }
        let Some(Nbt::List(pos)) = nbt.get("pos") else { return None };
        let [x, y, z] = pos.as_slice() else { return None };
        let pos = Vec3::new(x.as_f64()?, y.as_f64()?, z.as_f64()?);
        let uuid = nbt.get("source").and_then(uuid_from_nbt);
        let projectile_owner_uuid = nbt.get("projectile_owner").and_then(uuid_from_nbt);
        Some(VibrationInfo { game_event, distance, pos, uuid, projectile_owner_uuid, entity: None })
    }
}
