use wafer_core::random::RandomSource;
use wafer_core::{BlockPos, Vec3, mth};
use wafer_data::{block_tags, item_tags, items, registry, registry_id};

use super::sniffer_ai;
use crate::core::global_pos::GlobalPos;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::damagesource::DamageSource;
use crate::world::entity::ai::brain::{self, Brain, Provider};
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::sensing::sensor_type::SensorType;
use crate::world::entity::ai::util::land_random_pos;
use crate::world::entity::animal::Animal;
use crate::world::entity::attributes::SNIFFER_ATTRIBUTES;
use crate::world::entity::data::{DataValue, ageable_mob_defaults};
use crate::world::entity::item::ItemEntity;
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::{Entity, EntityDimensions, EntityId, LevelEntity, sound_event};
use crate::world::item::ItemStack;
use crate::world::level::BlockGetter;
use crate::world::level::pathfinder::path_type::PathType;
use crate::world::{InteractionHand, InteractionResult};

const DIGGING_DROP_SEED_OFFSET_TICKS: i32 = 120;
pub const SNIFFER_BABY_START_AGE: i32 = -48000;
const DIGGING_BB_HEIGHT_OFFSET: f32 = 0.4;
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(1.9, 1.75).with_eye_height(1.05).with_passenger_attachments(&[Vec3::new(0.0, 2.09375, 0.0)]);
const DIGGING_DIMENSIONS: EntityDimensions = EntityDimensions::scalable(1.9, 1.75 - DIGGING_BB_HEIGHT_OFFSET).with_eye_height(0.81);
pub const DATA_STATE: usize = 18;
const DATA_DROP_SEED_AT_TICK: usize = 19;
const SNIFFER_DIGGING: &str = "minecraft:gameplay/sniffer_digging";

pub static BRAIN_PROVIDER: Provider = brain::provider_with_memories(
    &[memory_module_type::SNIFFER_EXPLORED_POSITIONS.erase()],
    &[SensorType::NearestLivingEntities, SensorType::HurtBy, SensorType::NearestPlayers, SensorType::FoodTemptations],
    |_| sniffer_ai::get_activities(),
);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum State {
    Idling,
    FeelingHappy,
    Scenting,
    Sniffing,
    Searching,
    Digging,
    Rising,
}

impl State {
    pub const VALUES: [State; 7] = [State::Idling, State::FeelingHappy, State::Scenting, State::Sniffing, State::Searching, State::Digging, State::Rising];

    pub fn id(self) -> i32 {
        self as i32
    }

    fn by_id(id: i32) -> State {
        State::VALUES.get(id as usize).copied().unwrap_or(State::Idling)
    }
}

#[derive(Default)]
pub struct Sniffer {
    pub animal: Animal,
}

pub fn get_state(entity: &Entity) -> State {
    match entity.data.get(DATA_STATE) {
        DataValue::SnifferState(id) => State::by_id(*id),
        _ => State::Idling,
    }
}

pub fn is_food(item_stack: &ItemStack) -> bool {
    item_stack.get_item().is(&item_tags::SNIFFER_FOOD)
}

pub fn get_default_dimensions(entity: &Entity, baby: bool) -> EntityDimensions {
    let age_scale = if baby { 0.5 } else { 1.0 };
    if get_state(entity) == State::Digging { DIGGING_DIMENSIONS.scale(age_scale) } else { DIMENSIONS.scale(age_scale) }
}

pub fn get_ambient_sound(entity: &Entity) -> Option<&'static str> {
    if matches!(get_state(entity), State::Digging | State::Searching) { None } else { Some("minecraft:entity.sniffer.idle") }
}

pub fn get_hurt_sound() -> &'static str {
    "minecraft:entity.sniffer.hurt"
}

pub fn get_death_sound() -> &'static str {
    "minecraft:entity.sniffer.death"
}

pub fn get_step_sound() -> &'static str {
    "minecraft:entity.sniffer.step"
}

pub fn get_max_head_y_rot() -> i32 {
    50
}

pub fn on_pathfinding_start(level: &ServerLevel, mob: &Mob) {
    if level.is_on_fire(mob.entity.id) || mob.entity.is_in_water() {
        mob.set_pathfinding_malus(PathType::Water, 0.0);
    }
}

pub fn on_pathfinding_done(mob: &Mob) {
    mob.set_pathfinding_malus(PathType::Water, -1.0);
}

fn get_head_position(entity: &Entity) -> Vec3 {
    entity.position().add_vec(Vec3::direction_from_rotation(entity.x_rot, entity.y_rot).scale(2.25))
}

fn get_head_block(entity: &Entity) -> BlockPos {
    let position = get_head_position(entity);
    BlockPos::containing(position.x, entity.y() + 0.2f32 as f64, position.z)
}

impl Sniffer {
    pub fn create() -> Mob {
        let mut data = ageable_mob_defaults();
        data.push(DataValue::SnifferState(State::Idling.id()));
        data.push(DataValue::Int(0));
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:sniffer"), DIMENSIONS, data);
        let mut mob = Mob::new(entity, LivingEntity::new(&SNIFFER_ATTRIBUTES), MobKind::Sniffer(Sniffer::default()));
        Animal::init(&mut mob);
        mob.ai.navigation.set_can_float(true);
        mob.set_pathfinding_malus(PathType::Water, -1.0);
        mob.set_pathfinding_malus(PathType::OnTopOfPowderSnow, -1.0);
        mob.set_pathfinding_malus(PathType::DamageCautious, -1.0);
        mob
    }

    pub fn make_brain(mob: &mut Mob, packed_brain: brain::Packed) -> Brain {
        BRAIN_PROVIDER.make_brain(mob, packed_brain)
    }
}

impl ServerLevel {
    pub fn is_sniffer(&self, id: EntityId) -> bool {
        self.mob(id).is_some_and(|mob| matches!(mob.kind, MobKind::Sniffer(_)))
    }

    pub fn sniffer_is_searching(&self, id: EntityId) -> bool {
        get_state(self.entity(id)) == State::Searching
    }

    pub fn sniffer_is_tempted(&self, id: EntityId) -> bool {
        self.get_brain(id).has_memory_value(memory_module_type::TEMPTING_PLAYER)
    }

    pub fn sniffer_can_sniff(&self, id: EntityId) -> bool {
        let e = self.entity(id);
        !self.sniffer_is_tempted(id) && !self.is_panicking(id) && !e.is_in_water() && !self.is_in_love(id) && e.on_ground && !self.is_passenger(id) && !self.is_leashed(id)
    }

    fn sniffer_set_state(&mut self, id: EntityId, state: State) {
        if get_state(self.entity(id)) != state {
            self.entity_mut(id).data.set(DATA_STATE, DataValue::SnifferState(state.id()));
            self.refresh_mob_dimensions(id);
        }
    }

    pub fn sniffer_transition_to(&mut self, id: EntityId, state: State) {
        match state {
            State::Idling => self.sniffer_set_state(id, State::Idling),
            State::FeelingHappy => {
                self.entity_play_sound(id, sound_event("minecraft:entity.sniffer.happy"), 1.0, 1.0);
                self.sniffer_set_state(id, State::FeelingHappy);
            }
            State::Scenting => {
                self.sniffer_set_state(id, State::Scenting);
                self.sniffer_on_scenting_start(id);
            }
            State::Sniffing => {
                self.entity_play_sound(id, sound_event("minecraft:entity.sniffer.sniffing"), 1.0, 1.0);
                self.sniffer_set_state(id, State::Sniffing);
            }
            State::Searching => self.sniffer_set_state(id, State::Searching),
            State::Digging => {
                self.sniffer_set_state(id, State::Digging);
                self.sniffer_on_digging_start(id);
            }
            State::Rising => {
                self.entity_play_sound(id, sound_event("minecraft:entity.sniffer.digging_stop"), 1.0, 1.0);
                self.sniffer_set_state(id, State::Rising);
            }
        }
    }

    fn sniffer_on_scenting_start(&mut self, id: EntityId) {
        let pitch = if self.is_baby(id) { 1.3 } else { 1.0 };
        self.entity_play_sound(id, sound_event("minecraft:entity.sniffer.scenting"), 1.0, pitch);
    }

    fn sniffer_on_digging_start(&mut self, id: EntityId) {
        let tick_count = self.entity(id).tick_count;
        self.entity_mut(id).data.set(DATA_DROP_SEED_AT_TICK, DataValue::Int(tick_count + DIGGING_DROP_SEED_OFFSET_TICKS));
        self.broadcast_entity_event(id, 63);
    }

    pub fn sniffer_on_digging_complete(&mut self, id: EntityId, success: bool) {
        if success {
            let on_pos = self.on_pos(id, 1.0e-5);
            self.sniffer_store_explored_position(id, on_pos);
        }
    }

    pub fn sniffer_calculate_dig_position(&mut self, id: EntityId) -> Option<BlockPos> {
        for idx in 0..5 {
            let Some(pos) = land_random_pos::get_pos(self, id, 10 + 2 * idx, 3) else { continue };
            let position = BlockPos::containing(pos.x, pos.y, pos.z);
            if !self.world_border.is_within_bounds_pos(position) {
                continue;
            }
            let below = position.below();
            if self.sniffer_can_dig_at(id, below) {
                return Some(below);
            }
        }
        None
    }

    pub fn sniffer_can_dig(&mut self, id: EntityId) -> bool {
        let e = self.entity(id);
        !self.is_panicking(id)
            && !self.sniffer_is_tempted(id)
            && !self.is_baby(id)
            && !e.is_in_water()
            && e.on_ground
            && !self.is_passenger(id)
            && {
                let head = get_head_block(self.entity(id)).below();
                self.sniffer_can_dig_at(id, head)
            }
    }

    fn sniffer_can_dig_at(&mut self, id: EntityId, position: BlockPos) -> bool {
        if !self.block_state(position).is(&block_tags::SNIFFER_DIGGABLE_BLOCK) {
            return false;
        }
        let explored = GlobalPos::of(self.dimension, position);
        if self.sniffer_explored_positions(id).iter().any(|pos| *pos == explored) {
            return false;
        }
        self.get_navigation(id, |navigation, level| navigation.create_path(level, id, position, 1)).is_some_and(|path| path.can_reach())
    }

    fn sniffer_drop_seed(&mut self, id: EntityId) {
        let e = self.entity(id);
        let drop_seed_at_tick = match e.data.get(DATA_DROP_SEED_AT_TICK) {
            DataValue::Int(tick) => *tick,
            _ => 0,
        };
        if drop_seed_at_tick != e.tick_count {
            return;
        }
        let head = get_head_block(e);
        self.drop_from_gift_loot_table(id, SNIFFER_DIGGING, |level, item_stack| {
            let mut entity = ItemEntity::new(Vec3::new(head.x as f64, head.y as f64, head.z as f64), item_stack);
            entity.set_default_pickup_delay();
            level.add_fresh_entity(LevelEntity::Item(entity));
        });
        self.entity_play_sound(id, sound_event("minecraft:entity.sniffer.drop_seed"), 1.0, 1.0);
    }

    fn sniffer_store_explored_position(&mut self, id: EntityId, position: BlockPos) {
        let mut updated: Vec<GlobalPos> = self.sniffer_explored_positions(id).iter().take(20).cloned().collect();
        updated.insert(0, GlobalPos::of(self.dimension, position));
        self.get_brain_mut(id).set_memory(memory_module_type::SNIFFER_EXPLORED_POSITIONS, updated);
    }

    fn sniffer_explored_positions(&self, id: EntityId) -> &[GlobalPos] {
        self.get_brain(id).get_memory(memory_module_type::SNIFFER_EXPLORED_POSITIONS).map_or(&[], |positions| positions.as_slice())
    }

    pub fn sniffer_jump_from_ground(&mut self, id: EntityId) {
        self.living_entity_jump_from_ground(id);
        let speed_modifier = self.mob(id).expect("a sniffer").ai.move_control.speed_modifier;
        if speed_modifier > 0.0 {
            let current = self.entity(id).delta_movement.horizontal_distance_sqr();
            if current < 0.01 {
                self.entity_mut(id).move_relative(0.1, Vec3::new(0.0, 0.0, 1.0));
            }
        }
    }

    pub fn sniffer_spawn_child_from_breeding(&mut self, id: EntityId, partner: EntityId) {
        let item_stack = ItemStack::new(items::SNIFFER_EGG, 1);
        let position = self.entity(id).position();
        let mut entity = ItemEntity::new(position, item_stack);
        entity.set_default_pickup_delay();
        self.finalize_spawn_child_from_breeding(id, partner, None);
        let random = &mut self.entity_mut(id).random;
        let pitch = (random.next_float() - random.next_float()) * 0.2 + 0.5;
        self.entity_play_sound(id, sound_event("minecraft:block.sniffer_egg.plop"), 1.0, pitch);
        self.add_fresh_entity(LevelEntity::Item(entity));
    }

    pub fn sniffer_die(&mut self, id: EntityId, source: DamageSource) {
        self.sniffer_transition_to(id, State::Idling);
        self.living_entity_die(id, source);
    }

    pub fn sniffer_tick(&mut self, id: EntityId) {
        if get_state(self.entity(id)) == State::Digging {
            self.sniffer_drop_seed(id);
        }
        self.mob_tick(id);
    }

    pub fn sniffer_mob_interact(&mut self, id: EntityId, player: EntityId, hand: InteractionHand) -> InteractionResult {
        let held_item = self.player(player).item_in_hand(hand).unwrap_or_else(ItemStack::empty);
        let food = is_food(&held_item);
        let interaction_result = self.animal_mob_interact(id, player, hand);
        if interaction_result.consumes_action() && food {
            self.sniffer_play_eating_sound(id);
        }
        interaction_result
    }

    pub fn sniffer_play_eating_sound(&mut self, id: EntityId) {
        let pitch = mth::random_between(&mut self.random, 0.8, 1.2);
        self.play_sound_from_entity(None, id, sound_event("minecraft:entity.sniffer.eat"), SoundSource::Neutral, 1.0, pitch);
    }

    pub fn sniffer_get_breed_offspring(&mut self, _id: EntityId, _partner: EntityId) -> Option<Box<Mob>> {
        Some(Box::new(Sniffer::create()))
    }

    pub fn sniffer_can_mate(&self, id: EntityId, partner: EntityId) -> bool {
        if !self.is_sniffer(partner) {
            return false;
        }
        let states = [State::Idling, State::Scenting, State::FeelingHappy];
        states.contains(&get_state(self.entity(id))) && states.contains(&get_state(self.entity(partner))) && self.animal_can_mate(id, partner)
    }

    pub fn sniffer_custom_server_ai_step(&mut self, id: EntityId) {
        Brain::tick(self, id);
        if self.mob(id).is_none() {
            return;
        }
        sniffer_ai::update_activity(self.get_brain_mut(id));
        self.animal_custom_server_ai_step(id);
    }
}
