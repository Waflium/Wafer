use std::sync::OnceLock;

use wafer_core::{BlockPos, Vec3, mth};
use wafer_data::{block_tags, entity_type_tags, item_tags, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use super::frog_ai;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::ai::brain::{self, Brain, Provider};
use crate::world::entity::ai::control::look_control::LookControlKind;
use crate::world::entity::ai::control::move_control::MoveControlKind;
use crate::world::entity::ai::control::smooth_swimming_move_control::SmoothSwimmingMoveControl;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::navigation::path_navigation::PathNavigation;
use crate::world::entity::ai::sensing::sensor_type::SensorType;
use crate::world::entity::animal::Animal;
use crate::world::entity::attributes::FROG_ATTRIBUTES;
use crate::world::entity::data::{DataValue, ageable_mob_defaults};
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::variant::variant_utils::{self, SpawnPrioritySelectors};
use crate::world::entity::{Entity, EntityDimensions, EntityId, MoverType, sound_event};
use crate::world::item::ItemStack;
use crate::world::level::pathfinder::node_evaluator::NodeEvaluatorKind;
use crate::world::level::pathfinder::path_finder::PathFinder;
use crate::world::level::pathfinder::path_type::PathType;
use crate::world::level::pathfinder::pathfinding_context::PathfindingContext;
use crate::world::level::pathfinder::walk_node_evaluator::WalkNodeEvaluator;
use crate::world::level::server_level_accessor::ServerLevelAccessor;

pub const VARIANT_REGISTRY: &str = "minecraft:frog_variant";
pub const DATA_VARIANT_ID: usize = 18;
pub const DATA_TONGUE_TARGET_ID: usize = 19;
const FROG_FALL_DAMAGE_REDUCTION: i32 = 5;
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.5, 0.5).with_passenger_attachments(&[Vec3::new(0.0, 0.375, -0.25)]);
pub const MOVE_CONTROL: SmoothSwimmingMoveControl = SmoothSwimmingMoveControl::new(85, 10, 0.02, 0.1, true);

pub static BRAIN_PROVIDER: Provider = brain::provider(
    &[SensorType::NearestLivingEntities, SensorType::HurtBy, SensorType::FrogAttackables, SensorType::FrogTemptations, SensorType::IsInWater],
    |_| frog_ai::get_activities(),
);

#[derive(Default)]
pub struct Frog {
    pub animal: Animal,
}

pub struct FrogNodeEvaluator;

fn spawn_selectors() -> &'static SpawnPrioritySelectors {
    static SELECTORS: OnceLock<SpawnPrioritySelectors> = OnceLock::new();
    SELECTORS.get_or_init(|| SpawnPrioritySelectors::load(VARIANT_REGISTRY, crate::core::data_registries::json_entries("minecraft:frog_variant")))
}

pub fn default_variant() -> i32 {
    variant_utils::read_variant(VARIANT_REGISTRY, "minecraft:temperate").unwrap_or(0)
}

pub fn get_variant(entity: &Entity) -> i32 {
    match entity.data.get(DATA_VARIANT_ID) {
        DataValue::FrogVariant(variant) => *variant,
        _ => 0,
    }
}

pub fn set_variant(entity: &mut Entity, variant: i32) {
    entity.data.set(DATA_VARIANT_ID, DataValue::FrogVariant(variant));
}

pub fn erase_tongue_target(entity: &mut Entity) {
    entity.data.set(DATA_TONGUE_TARGET_ID, DataValue::OptionalUnsignedInt(None));
}

pub fn set_tongue_target(entity: &mut Entity, target: EntityId) {
    entity.data.set(DATA_TONGUE_TARGET_ID, DataValue::OptionalUnsignedInt(Some(target as u32)));
}

pub fn can_eat(level: &ServerLevel, entity: EntityId) -> bool {
    if level.mob(entity).and_then(Mob::cube_mob_size).is_some_and(|size| size != 1) {
        false
    } else {
        entity_type_tags::FROG_FOOD.contains(level.entity(entity).entity_type as usize)
    }
}

pub fn is_food(item_stack: &ItemStack) -> bool {
    item_stack.get_item().is(&item_tags::FROG_FOOD)
}

pub fn get_head_rot_speed() -> i32 {
    35
}

pub fn get_max_head_y_rot() -> i32 {
    5
}

pub fn is_pushed_by_fluid() -> bool {
    false
}

pub fn can_be_a_baby() -> bool {
    false
}

pub fn get_ambient_sound() -> &'static str {
    "minecraft:entity.frog.ambient"
}

pub fn get_hurt_sound() -> &'static str {
    "minecraft:entity.frog.hurt"
}

pub fn get_death_sound() -> &'static str {
    "minecraft:entity.frog.death"
}

pub fn get_step_sound() -> &'static str {
    "minecraft:entity.frog.step"
}

impl Frog {
    pub fn create() -> Mob {
        let mut data = ageable_mob_defaults();
        data.extend([DataValue::FrogVariant(default_variant()), DataValue::OptionalUnsignedInt(None)]);
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:frog"), DIMENSIONS, data);
        let mut mob = Mob::new(entity, LivingEntity::new(&FROG_ATTRIBUTES), MobKind::Frog(Frog::default()));
        Animal::init(&mut mob);
        mob.ai.look_control.kind = LookControlKind::Frog;
        mob.set_pathfinding_malus(PathType::Water, 4.0);
        mob.set_pathfinding_malus(PathType::Trapdoor, -1.0);
        mob.ai.move_control.kind = MoveControlKind::SmoothSwimming(MOVE_CONTROL);
        mob
    }

    pub fn make_brain(mob: &mut Mob, packed_brain: brain::Packed) -> Brain {
        BRAIN_PROVIDER.make_brain(mob, packed_brain)
    }

    pub fn add_additional_save_data(mob: &Mob, out: &mut Vec<(String, Nbt)>) {
        out.push(("variant".into(), Nbt::String(variant_utils::variant_name(VARIANT_REGISTRY, get_variant(&mob.entity)).into())));
    }

    pub fn read_additional_save_data(mob: &mut Mob, nbt: &Nbt) {
        if let Some(variant) = nbt.get("variant").and_then(Nbt::as_str).and_then(|name| variant_utils::read_variant(VARIANT_REGISTRY, name)) {
            set_variant(&mut mob.entity, variant);
        }
    }
}

pub fn finalize_spawn(level: &mut (impl ServerLevelAccessor + ?Sized), mob: &mut Mob) {
    if let Some(variant) = variant_utils::select_variant_to_spawn(level, mob.entity.block_position(), spawn_selectors()) {
        set_variant(&mut mob.entity, variant);
    }
    frog_ai::init_memories(&mut mob.living.brain, level.get_random());
}

impl ServerLevel {
    pub fn get_tongue_target(&self, id: EntityId) -> Option<EntityId> {
        match self.entity(id).data.get(DATA_TONGUE_TARGET_ID) {
            DataValue::OptionalUnsignedInt(Some(target)) => Some(*target as EntityId).filter(|&target| self.entity_ref(target).is_some()),
            _ => None,
        }
    }

    pub fn frog_custom_server_ai_step(&mut self, id: EntityId) {
        Brain::tick(self, id);
        if self.mob(id).is_none() {
            return;
        }
        frog_ai::update_activity(self.get_brain_mut(id));
        self.animal_custom_server_ai_step(id);
    }

    pub fn frog_play_eating_sound(&mut self, id: EntityId) {
        self.play_sound_from_entity(None, id, sound_event("minecraft:entity.frog.eat"), SoundSource::Neutral, 2.0, 1.0);
    }

    pub fn frog_get_breed_offspring(&mut self, _id: EntityId, _partner: EntityId) -> Option<Box<Mob>> {
        let mut frog = Box::new(Frog::create());
        frog_ai::init_memories(&mut frog.living.brain, &mut self.random);
        Some(frog)
    }

    pub fn frog_spawn_child_from_breeding(&mut self, id: EntityId, partner: EntityId) {
        self.finalize_spawn_child_from_breeding(id, partner, None);
        self.get_brain_mut(id).set_memory(memory_module_type::IS_PREGNANT, ());
    }

    pub fn frog_calculate_fall_damage(&self, id: EntityId, fall_distance: f64, damage_modifier: f32) -> i32 {
        self.living_entity_calculate_fall_damage(id, fall_distance, damage_modifier) - FROG_FALL_DAMAGE_REDUCTION
    }

    pub fn frog_travel_in_water(&mut self, id: EntityId, input: Vec3, _base_gravity: f64, _is_falling: bool, _old_y: f64) {
        let speed = self.get_speed(id);
        self.entity_mut(id).move_relative(speed, input);
        let delta = self.entity(id).delta_movement;
        self.move_entity(id, MoverType::SelfMove, delta);
        let e = self.entity_mut(id);
        e.set_delta_movement(e.delta_movement.scale(0.9));
    }

    pub fn frog_get_target(&self, id: EntityId) -> Option<EntityId> {
        self.get_target_from_brain(id)
    }

    pub fn frog_look_control_reset_x_rot_on_tick(&self, id: EntityId) -> bool {
        self.get_tongue_target(id).is_none()
    }
}

impl WalkNodeEvaluator {
    pub fn frog_node_evaluator_get_start(&mut self, context: &PathfindingContext) -> Option<u32> {
        let e = &self.mob(context).entity;
        if !e.is_in_water() {
            return self.amphibious_node_evaluator_get_start(context);
        }
        let bb = e.bounding_box();
        let pos = BlockPos::new(mth::floor(bb.min_x), mth::floor(bb.min_y), mth::floor(bb.min_z));
        Some(self.get_start_node(context, pos))
    }

    pub fn frog_node_evaluator_get_path_type(&self, context: &PathfindingContext, x: i32, y: i32, z: i32) -> PathType {
        let below_state = context.get_block_state(BlockPos::new(x, y - 1, z));
        if below_state.is(&block_tags::FROG_PREFER_JUMP_TO) { PathType::Open } else { self.amphibious_node_evaluator_get_path_type(context, x, y, z) }
    }
}

impl PathNavigation {
    pub fn frog_path_navigation_can_cut_corner(&self, path_type: PathType) -> bool {
        path_type != PathType::WaterBorder && self.path_navigation_can_cut_corner(path_type)
    }

    pub fn frog_path_navigation_create_path_finder(&mut self, max_visited_nodes: i32) -> PathFinder {
        PathFinder::new(NodeEvaluatorKind::Walk(WalkNodeEvaluator::frog(true)), max_visited_nodes)
    }
}
