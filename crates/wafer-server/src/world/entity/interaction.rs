use wafer_data::{registry, registry_id};
use wafer_protocol::nbt::Nbt;

use super::data::{DataValue, entity_defaults};
use super::entity_reference::EntityReference;
use super::{Entity, EntityDimensions, EntityId, LevelEntity};
use crate::server::level::ServerLevel;
use crate::world::InteractionResult;
use crate::util::codec::{self, DecodeResult, Record};
use crate::world::entity::entity_save::decode_uuid;
use crate::world::level::block::entity::{byte_or, float_or};
use crate::world::level::storage::{tag_value_input, uuid_ints};

const DATA_WIDTH_ID: usize = 8;
const DATA_HEIGHT_ID: usize = 9;
const DATA_RESPONSE_ID: usize = 10;
const DEFAULT_WIDTH: f32 = 1.0;
const DEFAULT_HEIGHT: f32 = 1.0;
const DEFAULT_RESPONSE: bool = false;

#[derive(Clone, Copy)]
struct PlayerAction {
    player: u128,
    timestamp: i64,
}

impl PlayerAction {
    fn to_nbt(self) -> Nbt {
        Nbt::Compound(vec![("player".into(), uuid_ints(self.player)), ("timestamp".into(), Nbt::Long(self.timestamp))].into())
    }

    fn decode(tag: &Nbt) -> DecodeResult<PlayerAction> {
        if let Err(error) = codec::map(tag) {
            return DecodeResult::error(error);
        }
        let mut record = Record::default();
        let player = record.required(tag, "player", decode_uuid);
        let timestamp = record.required(tag, "timestamp", |timestamp| DecodeResult::from(codec::long(timestamp)));
        record.build(|| Some(PlayerAction { player: player?, timestamp: timestamp? }))
    }
}

pub struct Interaction {
    pub entity: Entity,
    attack: Option<PlayerAction>,
    interaction: Option<PlayerAction>,
}

fn float(entity: &Entity, id: usize, default: f32) -> f32 {
    match entity.data.get(id) {
        DataValue::Float(v) => *v,
        _ => default,
    }
}

impl Interaction {
    pub fn create() -> Interaction {
        let mut data = entity_defaults();
        data.push(DataValue::Float(DEFAULT_WIDTH));
        data.push(DataValue::Float(DEFAULT_HEIGHT));
        data.push(DataValue::Boolean(DEFAULT_RESPONSE));
        let mut entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:interaction"), EntityDimensions::scalable(DEFAULT_WIDTH, DEFAULT_HEIGHT), data);
        entity.no_physics = true;
        Interaction { entity, attack: None, interaction: None }
    }

    fn get_width(&self) -> f32 {
        float(&self.entity, DATA_WIDTH_ID, DEFAULT_WIDTH)
    }

    fn get_height(&self) -> f32 {
        float(&self.entity, DATA_HEIGHT_ID, DEFAULT_HEIGHT)
    }

    fn get_response(&self) -> bool {
        matches!(self.entity.data.get(DATA_RESPONSE_ID), DataValue::Boolean(true))
    }

    fn set_width(&mut self, width: f32) {
        if *self.entity.data.get(DATA_WIDTH_ID) != DataValue::Float(width) {
            self.entity.data.set(DATA_WIDTH_ID, DataValue::Float(width));
            self.refresh_dimensions();
        }
    }

    fn set_height(&mut self, height: f32) {
        if *self.entity.data.get(DATA_HEIGHT_ID) != DataValue::Float(height) {
            self.entity.data.set(DATA_HEIGHT_ID, DataValue::Float(height));
            self.refresh_dimensions();
        }
    }

    fn refresh_dimensions(&mut self) {
        self.entity.dimensions = self.get_dimensions();
        self.entity.reapply_position();
    }

    fn get_dimensions(&self) -> EntityDimensions {
        EntityDimensions::scalable(self.get_width(), self.get_height())
    }

    pub fn read_additional_save_data(&mut self, input: &Nbt) {
        self.set_width(float_or(input, "width", DEFAULT_WIDTH));
        self.set_height(float_or(input, "height", DEFAULT_HEIGHT));
        self.attack = tag_value_input::read(input, "attack", PlayerAction::decode);
        self.interaction = tag_value_input::read(input, "interaction", PlayerAction::decode);
        let response = byte_or(input, "response", DEFAULT_RESPONSE as i8) != 0;
        self.entity.data.set(DATA_RESPONSE_ID, DataValue::Boolean(response));
        let bounding_box = self.get_dimensions().make_bounding_box(self.entity.position());
        self.entity.set_bounding_box(bounding_box);
    }

    pub fn add_additional_save_data(&self, output: &mut Vec<(String, Nbt)>) {
        output.push(("width".into(), Nbt::Float(self.get_width())));
        output.push(("height".into(), Nbt::Float(self.get_height())));
        if let Some(attack) = self.attack {
            output.push(("attack".into(), attack.to_nbt()));
        }
        if let Some(interaction) = self.interaction {
            output.push(("interaction".into(), interaction.to_nbt()));
        }
        output.push(("response".into(), Nbt::Byte(self.get_response() as i8)));
    }
}

impl ServerLevel {
    fn interaction_mut(&mut self, id: EntityId) -> &mut Interaction {
        match self.find_entity_mut(id) {
            Some(LevelEntity::Interaction(interaction)) => interaction,
            _ => unreachable!("an interaction"),
        }
    }

    fn interaction(&self, id: EntityId) -> &Interaction {
        match self.find_entity(id) {
            Some(LevelEntity::Interaction(interaction)) => interaction,
            _ => unreachable!("an interaction"),
        }
    }

    pub fn interaction_skip_attack_interaction(&mut self, id: EntityId, source: EntityId) -> bool {
        if !self.is_player(source) {
            return false;
        }
        let action = PlayerAction { player: self.entity(source).uuid, timestamp: self.game_time };
        self.interaction_mut(id).attack = Some(action);
        let damage_source = crate::world::damagesource::DamageSource::new(wafer_data::damage_types::GENERIC);
        crate::advancements::triggers::criteria_triggers::PLAYER_HURT_ENTITY.trigger(self, source, id, &damage_source, 1.0, 1.0, false);
        !self.interaction(id).get_response()
    }

    pub fn interaction_interact(&mut self, id: EntityId, player: EntityId) -> InteractionResult {
        let action = PlayerAction { player: self.entity(player).uuid, timestamp: self.game_time };
        self.interaction_mut(id).interaction = Some(action);
        InteractionResult::CONSUME
    }

    pub fn interaction_get_last_attacker(&self, id: EntityId) -> Option<EntityId> {
        self.interaction(id).attack.and_then(|attack| self.get_player_by_uuid(attack.player))
    }

    pub fn interaction_get_target(&self, id: EntityId) -> Option<EntityId> {
        self.interaction(id).interaction.and_then(|interaction| self.get_player_by_uuid(interaction.player))
    }

    pub fn get_player_by_uuid(&self, uuid: u128) -> Option<EntityId> {
        self.players.values().find(|p| p.entity.uuid == uuid).map(|p| p.entity.id)
    }

    pub fn get_last_attacker(&self, id: EntityId) -> Option<EntityId> {
        match self.find_entity(id) {
            Some(LevelEntity::Interaction(_)) => self.interaction_get_last_attacker(id),
            _ if self.is_living(id) => self.get_last_hurt_by_mob(id),
            _ => None,
        }
    }

    pub fn targeting_get_target(&self, id: EntityId) -> Option<EntityId> {
        match self.find_entity(id) {
            Some(LevelEntity::Interaction(_)) => self.interaction_get_target(id),
            Some(LevelEntity::Mob(_)) => self.get_target(id),
            _ => None,
        }
    }

    pub fn traceable_get_owner(&self, id: EntityId) -> Option<EntityId> {
        match self.find_entity(id) {
            Some(LevelEntity::Projectile(_)) => self.projectile_get_owner(id),
            Some(LevelEntity::Item(item)) => item.thrower.and_then(|thrower| EntityReference::of(thrower).get_entity(self)),
            Some(LevelEntity::PrimedTnt(tnt)) => EntityReference::get_living_entity(tnt.owner.map(EntityReference::of), self),
            Some(LevelEntity::AreaEffectCloud(_)) => self.area_effect_cloud_get_owner(id),
            Some(LevelEntity::EvokerFangs(_)) => self.evoker_fangs_owner(id),
            Some(LevelEntity::Mob(_)) if self.is_vex(id) => self.get_owner(id),
            _ => None,
        }
    }
}
