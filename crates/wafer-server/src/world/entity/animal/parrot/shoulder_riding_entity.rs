use wafer_protocol::nbt::Nbt;

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::tamable_animal::TamableAnimal;

const RIDE_COOLDOWN: i32 = 100;

#[derive(Default)]
pub struct ShoulderRidingEntity {
    pub tamable_animal: TamableAnimal,
    pub ride_cooldown_counter: i32,
}

impl ServerLevel {
    pub fn set_entity_on_shoulder(&mut self, id: EntityId, player: EntityId) -> bool {
        let Some(encode_id) = self.entities.get(&id).and_then(|entity| entity.encode_id()) else { return false };
        let mut output = Vec::new();
        self.save_without_id(id, &mut output);
        output.push(("id".into(), Nbt::String(encode_id.into())));
        if self.set_entity_on_shoulder_of(player, Nbt::Compound(output)) {
            self.discard(id);
            return true;
        }
        false
    }

    pub fn can_sit_on_shoulder(&self, id: EntityId) -> bool {
        self.mob(id).is_some_and(|mob| matches!(&mob.kind, crate::world::entity::mob::MobKind::Parrot(parrot) if parrot.shoulder_riding_entity.ride_cooldown_counter > RIDE_COOLDOWN))
    }
}
