use wafer_core::Direction;
use wafer_core::random::RandomSource;
use wafer_data::{registry, registry_id};
use wafer_protocol::nbt::Nbt;

use super::data::{DataValue, entity_defaults};
use super::entity_reference::EntityReference;
use super::item::ItemEntity;
use super::{Entity, EntityDimensions, EntityId, LevelEntity, RemovalReason};
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::item::{ItemStack, projectile_item};
use crate::world::level::storage::tag_value_input;

const SPAWN_ITEM_DELAY_MIN: i32 = 60;
const SPAWN_ITEM_DELAY_MAX: i32 = 120;
const TAG_SPAWN_ITEM_AFTER_TICKS: &str = "spawn_item_after_ticks";
const TAG_ITEM: &str = "item";
pub const DATA_ITEM: usize = 8;
pub const TICKS_BEFORE_ABOUT_TO_SPAWN_SOUND: i64 = 36;
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.25, 0.25);

pub struct OminousItemSpawner {
    pub entity: Entity,
    spawn_item_after_ticks: i64,
}

impl OminousItemSpawner {
    pub fn of_type() -> OminousItemSpawner {
        let mut data = entity_defaults();
        data.push(DataValue::ItemStack(None));
        let mut entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:ominous_item_spawner"), DIMENSIONS, data);
        entity.no_physics = true;
        OminousItemSpawner { entity, spawn_item_after_ticks: 0 }
    }

    pub fn create(level: &mut ServerLevel, item: ItemStack) -> OminousItemSpawner {
        let mut item_spawner = OminousItemSpawner::of_type();
        item_spawner.spawn_item_after_ticks = level.random.next_int_between_inclusive(SPAWN_ITEM_DELAY_MIN, SPAWN_ITEM_DELAY_MAX) as i64;
        item_spawner.set_item(item);
        item_spawner
    }

    pub fn get_item(&self) -> ItemStack {
        match self.entity.data.get(DATA_ITEM) {
            DataValue::ItemStack(Some(item)) => item.clone(),
            _ => ItemStack::empty(),
        }
    }

    fn set_item(&mut self, item_stack: ItemStack) {
        self.entity.data.set(DATA_ITEM, DataValue::ItemStack(Some(item_stack).filter(|item| !item.is_empty())));
    }

    pub fn read_additional_save_data(&mut self, input: &Nbt) {
        let item = tag_value_input::read(input, TAG_ITEM, ItemStack::from_nbt_partial).unwrap_or_else(ItemStack::empty);
        self.set_item(item);
        self.spawn_item_after_ticks = tag_value_input::get_i64(input, TAG_SPAWN_ITEM_AFTER_TICKS).unwrap_or(0);
    }

    pub fn add_additional_save_data(&self, output: &mut Vec<(String, Nbt)>) {
        let item = self.get_item();
        if !item.is_empty() {
            output.push((TAG_ITEM.into(), item.to_nbt()));
        }
        output.push((TAG_SPAWN_ITEM_AFTER_TICKS.into(), Nbt::Long(self.spawn_item_after_ticks)));
    }

    pub fn can_add_passenger() -> bool {
        false
    }

    pub fn could_accept_passenger() -> bool {
        false
    }

    pub fn is_ignoring_block_triggers() -> bool {
        true
    }

    pub fn hurt_server() -> bool {
        false
    }
}

fn ominous_item_spawner(level: &ServerLevel, id: EntityId) -> &OminousItemSpawner {
    match level.find_entity(id) {
        Some(LevelEntity::OminousItemSpawner(spawner)) => spawner,
        _ => unreachable!("an ominous item spawner"),
    }
}

fn ominous_item_spawner_mut(level: &mut ServerLevel, id: EntityId) -> &mut OminousItemSpawner {
    match level.find_entity_mut(id) {
        Some(LevelEntity::OminousItemSpawner(spawner)) => spawner,
        _ => unreachable!("an ominous item spawner"),
    }
}

impl ServerLevel {
    pub fn tick_ominous_item_spawner(&mut self, id: EntityId) {
        self.entity_tick(id);
        if self.entity_ref(id).is_none_or(|e| e.is_removed()) {
            return;
        }
        self.ominous_item_spawner_tick_server(id);
    }

    fn ominous_item_spawner_tick_server(&mut self, id: EntityId) {
        let spawn_item_after_ticks = ominous_item_spawner(self, id).spawn_item_after_ticks;
        let tick_count = self.entity(id).tick_count as i64;
        if tick_count == spawn_item_after_ticks - TICKS_BEFORE_ABOUT_TO_SPAWN_SOUND {
            let block_position = self.entity(id).block_position();
            self.play_sound(None, block_position, registry_id(registry::SOUND_EVENT, "minecraft:block.trial_spawner.about_to_spawn_item"), SoundSource::Neutral, 1.0, 1.0);
        }
        if tick_count >= spawn_item_after_ticks {
            self.ominous_item_spawner_spawn_item(id);
            self.remove(id, RemovalReason::Killed);
        }
    }

    fn ominous_item_spawner_spawn_item(&mut self, id: EntityId) {
        let item = ominous_item_spawner(self, id).get_item();
        if item.is_empty() {
            return;
        }
        let spawned_entity = if projectile_item::is_projectile_item(item.get_item()) {
            self.ominous_item_spawner_spawn_projectile(id, &item)
        } else {
            let e = self.entity(id);
            let spawned_entity = ItemEntity::new(e.position(), item);
            let spawned_id = spawned_entity.entity.id;
            self.add_fresh_entity(LevelEntity::Item(spawned_entity));
            spawned_id
        };
        let block_position = self.entity(id).block_position();
        self.level_event(None, crate::server::level::events::PARTICLES_TRIAL_SPAWNER_SPAWN_ITEM, block_position, 1);
        let position = self.entity(id).position();
        self.game_event_from_entity(Some(spawned_entity), crate::world::level::gameevent::game_event::ENTITY_PLACE, position);
        ominous_item_spawner_mut(self, id).set_item(ItemStack::empty());
    }

    fn ominous_item_spawner_spawn_projectile(&mut self, id: EntityId, item: &ItemStack) -> EntityId {
        let dispense_config = projectile_item::create_dispense_config(item.get_item());
        if let Some(event) = dispense_config.override_dispense_event {
            let block_position = self.entity(id).block_position();
            self.level_event(None, event, block_position, 0);
        }
        let direction = Direction::Down;
        let position = self.entity(id).position();
        let projectile = projectile_item::as_projectile(self, position, item, direction);
        let projectile = self.spawn_projectile_using_shoot(
            projectile,
            item,
            direction.step_x() as f64,
            direction.step_y() as f64,
            direction.step_z() as f64,
            dispense_config.power,
            dispense_config.uncertainty,
        );
        let owner = Some(EntityReference::of(self.entity(id).uuid));
        self.projectile_set_owner_reference(projectile, owner);
        projectile
    }
}
