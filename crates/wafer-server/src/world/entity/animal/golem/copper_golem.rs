use wafer_core::random::RandomSource;
use wafer_core::{BlockPos, Direction};
use wafer_data::properties::{CHEST_TYPE, COPPER_GOLEM_POSE, ChestType, CopperGolemPose, HORIZONTAL_FACING};
use wafer_data::{EquipmentSlot, block_entity_types, blocks, game_rule, item_tags, items, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use super::copper_golem_ai;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::ai::behavior::behavior_utils;
use crate::world::entity::ai::brain::{self, Brain, Provider};
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::sensing::sensor_type::SensorType;
use crate::world::entity::attributes::COPPER_GOLEM_ATTRIBUTES;
use crate::world::entity::data::{DataValue, ids, mob_defaults};
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::{Entity, EntityDimensions, EntityId, sound_event};
use crate::world::item::ItemStack;
use crate::world::item::component::{self as dc, DataComponentMap};
use crate::world::level::BlockGetter;
use crate::world::level::block::chest_block;
use crate::world::level::block::weathering_copper::WeatherState;
use crate::world::level::pathfinder::path_type::PathType;
use crate::world::level::update_flags::UPDATE_ALL;
use crate::world::{InteractionHand, InteractionResult};

const IGNORE_WEATHERING_TICK: i64 = -2;
const UNSET_WEATHERING_TICK: i64 = -1;
const WEATHERING_TICK_FROM: i32 = 504000;
const WEATHERING_TICK_TO: i32 = 552000;
const TURN_TO_STATUE_CHANCE: f32 = 0.0058;
const SPAWN_COOLDOWN_MIN: i32 = 60;
const SPAWN_COOLDOWN_MAX: i32 = 100;
pub const DATA_WEATHER_STATE: usize = 16;
pub const COPPER_GOLEM_STATE: usize = 17;
pub const EQUIPMENT_SLOT_ANTENNA: EquipmentSlot = EquipmentSlot::Saddle;
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.49, 0.98).with_eye_height(0.8125);

pub static BRAIN_PROVIDER: Provider = brain::provider(&[SensorType::NearestLivingEntities, SensorType::HurtBy], |_| copper_golem_ai::get_activities());

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CopperGolemState {
    Idle,
    GettingItem,
    GettingNoItem,
    DroppingItem,
    DroppingNoItem,
}

pub struct CopperGolem {
    opened_chest_pos: Option<BlockPos>,
    last_lightning_bolt_uuid: Option<u128>,
    next_weathering_tick: i64,
}

impl Default for CopperGolem {
    fn default() -> CopperGolem {
        CopperGolem { opened_chest_pos: None, last_lightning_bolt_uuid: None, next_weathering_tick: UNSET_WEATHERING_TICK }
    }
}

fn copper_golem_mut(mob: &mut Mob) -> &mut CopperGolem {
    match &mut mob.kind {
        MobKind::CopperGolem(copper_golem) => copper_golem,
        _ => unreachable!("a copper golem"),
    }
}

fn copper_golem(mob: &Mob) -> &CopperGolem {
    match &mob.kind {
        MobKind::CopperGolem(copper_golem) => copper_golem,
        _ => unreachable!("a copper golem"),
    }
}

pub fn get_weather_state(entity: &Entity) -> WeatherState {
    match entity.data.get(DATA_WEATHER_STATE) {
        DataValue::WeatheringCopperState(id) => WeatherState::by_id(*id),
        _ => unreachable!("the copper golem weather state is a weathering copper state"),
    }
}

pub fn set_weather_state(entity: &mut Entity, state: WeatherState) {
    entity.data.set(DATA_WEATHER_STATE, DataValue::WeatheringCopperState(state as i32));
}

pub fn get_state(entity: &Entity) -> CopperGolemState {
    match entity.data.get(COPPER_GOLEM_STATE) {
        DataValue::CopperGolemState(1) => CopperGolemState::GettingItem,
        DataValue::CopperGolemState(2) => CopperGolemState::GettingNoItem,
        DataValue::CopperGolemState(3) => CopperGolemState::DroppingItem,
        DataValue::CopperGolemState(4) => CopperGolemState::DroppingNoItem,
        _ => CopperGolemState::Idle,
    }
}

pub fn set_state(entity: &mut Entity, state: CopperGolemState) {
    entity.data.set(COPPER_GOLEM_STATE, DataValue::CopperGolemState(state as i32));
}

fn oxidation_level(state: WeatherState) -> (&'static str, &'static str, &'static str, &'static str) {
    match state {
        WeatherState::Unaffected | WeatherState::Exposed => {
            ("minecraft:entity.copper_golem.spin", "minecraft:entity.copper_golem.hurt", "minecraft:entity.copper_golem.death", "minecraft:entity.copper_golem.step")
        }
        WeatherState::Weathered => (
            "minecraft:entity.copper_golem_weathered.spin",
            "minecraft:entity.copper_golem_weathered.hurt",
            "minecraft:entity.copper_golem_weathered.death",
            "minecraft:entity.copper_golem_weathered.step",
        ),
        WeatherState::Oxidized => (
            "minecraft:entity.copper_golem_oxidized.spin",
            "minecraft:entity.copper_golem_oxidized.hurt",
            "minecraft:entity.copper_golem_oxidized.death",
            "minecraft:entity.copper_golem_oxidized.step",
        ),
    }
}

pub fn get_hurt_sound(entity: &Entity) -> &'static str {
    oxidation_level(get_weather_state(entity)).1
}

pub fn get_death_sound(entity: &Entity) -> &'static str {
    oxidation_level(get_weather_state(entity)).2
}

impl CopperGolem {
    pub fn create() -> Mob {
        let mut data = mob_defaults();
        data.extend([DataValue::WeatheringCopperState(WeatherState::Unaffected as i32), DataValue::CopperGolemState(CopperGolemState::Idle as i32)]);
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:copper_golem"), DIMENSIONS, data);
        let mut mob = Mob::new(entity, LivingEntity::new(&COPPER_GOLEM_ATTRIBUTES), MobKind::CopperGolem(CopperGolem::default()));
        let follow_range = mob.living.attributes.value(crate::world::entity::attributes::Attribute::FollowRange);
        mob.ai.navigation.set_required_path_length_detached(follow_range, 48.0);
        mob.ai.navigation.set_can_open_doors(true);
        mob.persistence_required = true;
        set_state(&mut mob.entity, CopperGolemState::Idle);
        mob.set_pathfinding_malus(PathType::FireInNeighbor, 16.0);
        mob.set_pathfinding_malus(PathType::DamagingInNeighbor, 16.0);
        mob.set_pathfinding_malus(PathType::Fire, -1.0);
        let cooldown = mob.entity.random.next_int_range(SPAWN_COOLDOWN_MIN, SPAWN_COOLDOWN_MAX);
        mob.living.brain.set_memory(memory_module_type::TRANSPORT_ITEMS_COOLDOWN_TICKS, cooldown);
        mob
    }

    pub fn make_brain(mob: &mut Mob, packed_brain: brain::Packed) -> Brain {
        BRAIN_PROVIDER.make_brain(mob, packed_brain)
    }

    pub fn add_additional_save_data(mob: &Mob, output: &mut Vec<(String, Nbt)>) {
        output.push(("next_weather_age".into(), Nbt::Long(copper_golem(mob).next_weathering_tick)));
        output.push(("weather_state".into(), Nbt::String(get_weather_state(&mob.entity).name().into())));
    }

    pub fn read_additional_save_data(mob: &mut Mob, input: &Nbt) {
        copper_golem_mut(mob).next_weathering_tick = input.get("next_weather_age").and_then(Nbt::as_i64).unwrap_or(UNSET_WEATHERING_TICK);
        let weather_state = input.get("weather_state").and_then(Nbt::as_str).and_then(WeatherState::by_name).unwrap_or(WeatherState::Unaffected);
        set_weather_state(&mut mob.entity, weather_state);
    }
}

impl ServerLevel {
    pub fn is_copper_golem(&self, id: EntityId) -> bool {
        matches!(self.mob(id).map(|mob| &mob.kind), Some(MobKind::CopperGolem(_)))
    }

    pub fn copper_golem_set_opened_chest_pos(&mut self, id: EntityId, opened_chest_pos: BlockPos) {
        copper_golem_mut(self.mob_mut(id).expect("a copper golem")).opened_chest_pos = Some(opened_chest_pos);
    }

    pub fn copper_golem_clear_opened_chest_pos(&mut self, id: EntityId) {
        copper_golem_mut(self.mob_mut(id).expect("a copper golem")).opened_chest_pos = None;
    }

    pub fn copper_golem_custom_server_ai_step(&mut self, id: EntityId) {
        Brain::tick(self, id);
        if self.mob(id).is_none() {
            return;
        }
        copper_golem_ai::update_activity(self.get_brain_mut(id));
    }

    pub fn copper_golem_tick(&mut self, id: EntityId) {
        self.mob_tick(id);
        if self.mob(id).is_none_or(|mob| mob.entity.is_removed()) {
            return;
        }
        let game_time = self.game_time;
        self.copper_golem_update_weathering(id, game_time);
    }

    pub fn copper_golem_mob_interact(&mut self, id: EntityId, player: EntityId, hand: InteractionHand) -> InteractionResult {
        let mut item_stack = self.player(player).item_in_hand(hand).unwrap_or_else(ItemStack::empty);
        if item_stack.is_empty() {
            let equipped_item = self.item_by_slot(id, EquipmentSlot::Mainhand).unwrap_or_else(ItemStack::empty);
            if !equipped_item.is_empty() {
                let target_pos = self.entity(player).position();
                behavior_utils::throw_item(self, id, equipped_item, target_pos);
                self.set_item_in_hand(id, InteractionHand::MainHand, ItemStack::empty());
                return InteractionResult::SUCCESS;
            }
        }
        if item_stack.is(items::SHEARS) && self.copper_golem_ready_for_shearing(id) {
            self.copper_golem_shear(id, SoundSource::Players, &item_stack);
            self.hurt_and_break(player, hand.as_equipment_slot(), 1);
            return InteractionResult::SUCCESS;
        }
        let next_weathering_tick = copper_golem(self.mob(id).expect("a copper golem")).next_weathering_tick;
        if item_stack.is(items::HONEYCOMB) && next_weathering_tick != IGNORE_WEATHERING_TICK {
            let pos = self.entity(id).block_position();
            self.level_event(Some(id), crate::server::level::events::PARTICLES_WAX_ON, pos, 0);
            self.play_sound(None, pos, registry_id(registry::SOUND_EVENT, "minecraft:item.honeycomb.wax_on"), SoundSource::Blocks, 1.0, 1.0);
            copper_golem_mut(self.mob_mut(id).expect("a copper golem")).next_weathering_tick = IGNORE_WEATHERING_TICK;
            self.use_player_item(player, hand, &mut item_stack);
            return InteractionResult::SUCCESS_SERVER;
        }
        if item_stack.get_item().is(&item_tags::AXES) && next_weathering_tick == IGNORE_WEATHERING_TICK {
            let source = self.sound_source(id);
            self.play_sound_from_entity(None, id, sound_event("minecraft:item.axe.scrape"), source, 1.0, 1.0);
            let pos = self.entity(id).block_position();
            self.level_event(Some(id), crate::server::level::events::PARTICLES_WAX_OFF, pos, 0);
            copper_golem_mut(self.mob_mut(id).expect("a copper golem")).next_weathering_tick = UNSET_WEATHERING_TICK;
            self.hurt_and_break(player, hand.as_equipment_slot(), 1);
            return InteractionResult::SUCCESS_SERVER;
        }
        if item_stack.get_item().is(&item_tags::AXES) {
            let weather_state = get_weather_state(self.entity(id));
            if weather_state != WeatherState::Unaffected {
                let source = self.sound_source(id);
                self.play_sound_from_entity(None, id, sound_event("minecraft:item.axe.scrape"), source, 1.0, 1.0);
                let pos = self.entity(id).block_position();
                self.level_event(Some(id), crate::server::level::events::PARTICLES_SCRAPE, pos, 0);
                copper_golem_mut(self.mob_mut(id).expect("a copper golem")).next_weathering_tick = UNSET_WEATHERING_TICK;
                set_weather_state(self.entity_mut(id), weather_state.previous());
                self.hurt_and_break(player, hand.as_equipment_slot(), 1);
                return InteractionResult::SUCCESS_SERVER;
            }
        }
        self.mob_mob_interact(id, player, hand)
    }

    fn copper_golem_update_weathering(&mut self, id: EntityId, game_time: i64) {
        let next_weathering_tick = copper_golem(self.mob(id).expect("a copper golem")).next_weathering_tick;
        if next_weathering_tick == IGNORE_WEATHERING_TICK {
            return;
        }
        if next_weathering_tick == UNSET_WEATHERING_TICK {
            let next = game_time + self.random.next_int_between_inclusive(WEATHERING_TICK_FROM, WEATHERING_TICK_TO) as i64;
            copper_golem_mut(self.mob_mut(id).expect("a copper golem")).next_weathering_tick = next;
        } else {
            let weather_state = get_weather_state(self.entity(id));
            let is_fully_oxidized = weather_state == WeatherState::Oxidized;
            if game_time >= next_weathering_tick && !is_fully_oxidized {
                let new_state = weather_state.next();
                let is_new_state_fully_oxidized = new_state == WeatherState::Oxidized;
                set_weather_state(self.entity_mut(id), new_state);
                let next = if is_new_state_fully_oxidized { 0 } else { next_weathering_tick + self.random.next_int_between_inclusive(WEATHERING_TICK_FROM, WEATHERING_TICK_TO) as i64 };
                copper_golem_mut(self.mob_mut(id).expect("a copper golem")).next_weathering_tick = next;
            }
            if is_fully_oxidized && self.copper_golem_can_turn_to_statue(id) {
                self.copper_golem_turn_to_statue(id);
            }
        }
    }

    fn copper_golem_can_turn_to_statue(&mut self, id: EntityId) -> bool {
        let pos = self.entity(id).block_position();
        self.block_state(pos).is_air() && self.random.next_float() <= TURN_TO_STATUE_CHANCE
    }

    fn copper_golem_turn_to_statue(&mut self, id: EntityId) {
        let pos = self.entity(id).block_position();
        let pose = CopperGolemPose::VALUES[self.entity_mut(id).random.next_int_range(0, CopperGolemPose::VALUES.len() as i32) as usize];
        let facing = Direction::from_y_rot(self.entity(id).y_rot as f64);
        let state = blocks::OXIDIZED_COPPER_GOLEM_STATUE.default_state().set(COPPER_GOLEM_POSE, pose).set(HORIZONTAL_FACING, facing);
        self.set_block(pos, state, UPDATE_ALL);
        if self.get_block_entity(pos).is_some_and(|entity| entity.ty() == block_entity_types::COPPER_GOLEM_STATUE) {
            self.copper_golem_statue_create_statue(pos, id);
            self.drop_preserved_equipment(id);
            self.discard(id);
            self.entity_play_sound(id, sound_event("minecraft:entity.copper_golem_become_statue"), 1.0, 1.0);
            if self.is_leashed(id) {
                if self.game_rules.get_bool(game_rule::ENTITY_DROPS) {
                    self.drop_leash(id);
                } else {
                    self.remove_leash(id);
                }
            }
        }
    }

    fn copper_golem_statue_create_statue(&mut self, pos: BlockPos, copper_golem: EntityId) {
        let custom_name = match self.entity(copper_golem).data.get(ids::CUSTOM_NAME) {
            DataValue::OptionalComponent(name) => name.clone(),
            _ => None,
        };
        let Some(entity) = self.get_block_entity(pos) else { return };
        let mut entries: Vec<_> = entity.components().iter().filter(|&(ty, _)| ty != dc::CUSTOM_NAME).map(|(ty, value)| (ty, value.clone())).collect();
        if let Some(custom_name) = custom_name {
            entries.push((dc::CUSTOM_NAME, custom_name));
        }
        entity.set_components(DataComponentMap::new(entries));
        self.block_entity_set_changed(pos);
    }

    pub fn copper_golem_statue_remove_statue(&mut self, pos: BlockPos, state: wafer_data::BlockState) -> Mob {
        let mut copper_golem = CopperGolem::create();
        let custom_name = self.get_block_entity(pos).and_then(|entity| entity.components().get(dc::CUSTOM_NAME).cloned());
        copper_golem.entity.set_custom_name(custom_name);
        let center = wafer_core::Vec3::at_center_of(pos);
        copper_golem.entity.snap_to(center.x, pos.y as f64, center.z, state.get(HORIZONTAL_FACING).to_y_rot(), 0.0);
        copper_golem.entity.y_head_rot = copper_golem.entity.y_rot;
        copper_golem.living.y_body_rot = copper_golem.entity.y_rot;
        if !copper_golem.entity.is_silent() {
            let (x, y, z) = (copper_golem.entity.x(), copper_golem.entity.y(), copper_golem.entity.z());
            self.play_sound_at(None, x, y, z, sound_event("minecraft:entity.copper_golem.spawn"), SoundSource::Neutral, 1.0, 1.0);
        }
        copper_golem
    }

    pub fn copper_golem_spawn(&mut self, id: EntityId, weather_state: WeatherState) {
        set_weather_state(self.entity_mut(id), weather_state);
        self.copper_golem_play_spawn_sound(id);
    }

    pub fn copper_golem_play_spawn_sound(&mut self, id: EntityId) {
        self.entity_play_sound(id, sound_event("minecraft:entity.copper_golem.spawn"), 1.0, 1.0);
    }

    pub fn copper_golem_play_step_sound(&mut self, id: EntityId) {
        let step_sound = oxidation_level(get_weather_state(self.entity(id))).3;
        self.entity_play_sound(id, sound_event(step_sound), 1.0, 1.0);
    }

    pub fn copper_golem_has_container_open(&mut self, id: EntityId, block_pos: BlockPos) -> bool {
        let Some(opened_chest_pos) = copper_golem(self.mob(id).expect("a copper golem")).opened_chest_pos else { return false };
        let block_state = self.block_state(opened_chest_pos);
        opened_chest_pos == block_pos
            || block_state.block().is_instance_of("ChestBlock")
                && block_state.get(CHEST_TYPE) != ChestType::Single
                && opened_chest_pos.relative(chest_block::get_connected_direction(block_state)) == block_pos
    }

    pub fn copper_golem_get_container_interaction_range(&self) -> f64 {
        3.0
    }

    pub fn copper_golem_shear(&mut self, id: EntityId, sound_source: SoundSource, _tool: &ItemStack) {
        self.play_sound_from_entity(None, id, sound_event("minecraft:entity.copper_golem.shear"), sound_source, 1.0, 1.0);
        let item_stack = self.item_by_slot(id, EQUIPMENT_SLOT_ANTENNA).unwrap_or_else(ItemStack::empty);
        self.living_set_item_slot(id, EQUIPMENT_SLOT_ANTENNA, ItemStack::empty());
        self.spawn_at_location(id, item_stack, 1.5);
    }

    pub fn copper_golem_ready_for_shearing(&self, id: EntityId) -> bool {
        self.item_by_slot_ref(id, EQUIPMENT_SLOT_ANTENNA).is_some_and(|item_stack| item_stack.get_item().is(&item_tags::SHEARABLE_FROM_COPPER_GOLEM))
    }

    pub fn copper_golem_drop_equipment(&mut self, id: EntityId) {
        self.drop_preserved_equipment(id);
    }

    pub fn copper_golem_actually_hurt(&mut self, id: EntityId, source: &crate::world::damagesource::DamageSource, damage: f32) {
        self.living_entity_actually_hurt(id, source, damage);
        set_state(self.entity_mut(id), CopperGolemState::Idle);
    }

    pub fn copper_golem_thunder_hit(&mut self, id: EntityId, lightning_bolt: EntityId) {
        self.entity_thunder_hit(id, lightning_bolt);
        if self.mob(id).is_none() {
            return;
        }
        let lightning_bolt_uuid = self.entity(lightning_bolt).uuid;
        let copper_golem = copper_golem_mut(self.mob_mut(id).expect("a copper golem"));
        if copper_golem.last_lightning_bolt_uuid != Some(lightning_bolt_uuid) {
            copper_golem.last_lightning_bolt_uuid = Some(lightning_bolt_uuid);
            let weather_state = get_weather_state(self.entity(id));
            if weather_state != WeatherState::Unaffected {
                copper_golem_mut(self.mob_mut(id).expect("a copper golem")).next_weathering_tick = UNSET_WEATHERING_TICK;
                set_weather_state(self.entity_mut(id), weather_state.previous());
            }
        }
    }
}
