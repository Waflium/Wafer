use wafer_core::Vec3;
use wafer_data::{damage_types, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::entity::data::entity_defaults;
use crate::world::entity::entity_reference::EntityReference;
use crate::world::entity::{Entity, EntityDimensions, EntityId, LevelEntity};

pub const ATTACK_DURATION: i32 = 20;
pub const LIFE_OFFSET: i32 = 2;
const DEFAULT_WARMUP_DELAY: i32 = 0;
const EVENT_ATTACK: u8 = 4;

pub struct EvokerFangs {
    pub entity: Entity,
    warmup_delay_ticks: i32,
    sent_spike_event: bool,
    life_ticks: i32,
    owner: Option<EntityReference>,
}

impl EvokerFangs {
    pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.5, 0.8);

    pub fn of_type() -> EvokerFangs {
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:evoker_fangs"), Self::DIMENSIONS, entity_defaults());
        EvokerFangs { entity, warmup_delay_ticks: DEFAULT_WARMUP_DELAY, sent_spike_event: false, life_ticks: ATTACK_DURATION + LIFE_OFFSET, owner: None }
    }

    pub fn new(x: f64, y: f64, z: f64, rotation_radians: f32, warmup_delay_ticks: i32, owner: u128) -> EvokerFangs {
        let mut fangs = EvokerFangs::of_type();
        fangs.warmup_delay_ticks = warmup_delay_ticks;
        fangs.set_owner(Some(owner));
        fangs.entity.set_y_rot(rotation_radians * (180.0 / std::f64::consts::PI) as f32);
        fangs.entity.set_pos(Vec3::new(x, y, z));
        fangs
    }

    pub fn set_owner(&mut self, owner: Option<u128>) {
        self.owner = owner.map(EntityReference::of);
    }

    pub fn read_additional_save_data(&mut self, input: &Nbt) {
        self.warmup_delay_ticks = input.get("Warmup").and_then(Nbt::as_i64).map_or(DEFAULT_WARMUP_DELAY, |v| v as i32);
        self.owner = EntityReference::read(input, "Owner");
    }

    pub fn add_additional_save_data(&self, output: &mut Vec<(String, Nbt)>) {
        output.push(("Warmup".into(), Nbt::Int(self.warmup_delay_ticks)));
        EntityReference::store(self.owner, output, "Owner");
    }
}

impl ServerLevel {
    fn evoker_fangs(&mut self, id: EntityId) -> &mut EvokerFangs {
        match self.entities.get_mut(&id) {
            Some(LevelEntity::EvokerFangs(fangs)) => fangs,
            _ => unreachable!("not evoker fangs"),
        }
    }

    pub fn evoker_fangs_owner(&self, id: EntityId) -> Option<EntityId> {
        match self.entities.get(&id) {
            Some(LevelEntity::EvokerFangs(fangs)) => EntityReference::get_living_entity(fangs.owner, self),
            _ => None,
        }
    }

    fn evoker_fangs_get_owner(&mut self, id: EntityId) -> Option<EntityId> {
        let owner = self.evoker_fangs(id).owner;
        EntityReference::get_living_entity(owner, self)
    }

    pub fn tick_evoker_fangs(&mut self, id: EntityId) {
        self.entity_base_tick(id);
        let fangs = self.evoker_fangs(id);
        fangs.warmup_delay_ticks -= 1;
        if fangs.warmup_delay_ticks < 0 {
            if fangs.warmup_delay_ticks == -8 {
                let bb = self.entity(id).bounding_box().inflate(0.2, 0.0, 0.2);
                let entities: Vec<EntityId> = self.get_entities(None, &bb).into_iter().filter(|&entity| self.is_living(entity)).collect();
                for entity in entities {
                    self.evoker_fangs_deal_damage_to(id, entity);
                }
            }
            if !self.evoker_fangs(id).sent_spike_event {
                self.broadcast_entity_event(id, EVENT_ATTACK);
                self.evoker_fangs(id).sent_spike_event = true;
            }
            let fangs = self.evoker_fangs(id);
            fangs.life_ticks -= 1;
            if fangs.life_ticks < 0 {
                self.discard(id);
            }
        }
    }

    fn evoker_fangs_deal_damage_to(&mut self, id: EntityId, entity: EntityId) {
        let current_owner = self.evoker_fangs_get_owner(id);
        if self.is_alive(entity) && !self.entity(entity).invulnerable && Some(entity) != current_owner {
            match current_owner {
                None => {
                    self.hurt(entity, DamageSource::new(damage_types::MAGIC), 6.0);
                }
                Some(current_owner) => {
                    if self.is_allied_to(current_owner, entity) {
                        return;
                    }
                    let damage_source = DamageSource::source(damage_types::INDIRECT_MAGIC, Some(id), Some(current_owner));
                    if self.hurt_server(entity, damage_source.clone(), 6.0) {
                        self.do_post_attack_effects(entity, &damage_source);
                    }
                }
            }
        }
    }
}
