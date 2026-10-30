use wafer_data::{item_tags, items, registry, registry_id};

use crate::util::profiling::profiler;
use wafer_protocol::nbt::Nbt;

use super::tadpole_ai;
use crate::server::level::ServerLevel;
use crate::world::entity::ageable_mob::{can_use_golden_dandelion, get_speed_up_seconds_when_feeding};
use crate::world::entity::ai::brain::{self, Brain, Provider};
use crate::world::entity::ai::control::look_control::LookControlKind;
use crate::world::entity::ai::control::move_control::MoveControlKind;
use crate::world::entity::ai::control::smooth_swimming_move_control::SmoothSwimmingMoveControl;
use crate::world::entity::ai::sensing::sensor_type::SensorType;
use crate::world::entity::animal::fish::abstract_fish::abstract_fish_defaults;
use crate::world::entity::animal::fish::water_animal;
use crate::world::entity::attributes::TADPOLE_ATTRIBUTES;
use crate::world::entity::bucketable;
use crate::world::entity::conversion_params::ConversionParams;
use crate::world::entity::data::{DataValue, ids};
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::{Entity, EntityDimensions, EntityId, LevelEntity, sound_event};
use crate::world::item::ItemStack;
use crate::world::level::server_level_accessor::ServerLevelAccessor;
use crate::world::{InteractionHand, InteractionResult};
use crate::world::level::storage::tag_value_input;

const DEFAULT_AGE: i32 = 0;
pub const TICKS_TO_BE_FROG: i32 = 24000;
pub const HITBOX_WIDTH: f32 = 0.4;
pub const HITBOX_HEIGHT: f32 = 0.3;
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(HITBOX_WIDTH, HITBOX_HEIGHT).with_eye_height(0.19500001);
pub const MOVE_CONTROL: SmoothSwimmingMoveControl = SmoothSwimmingMoveControl::new(85, 10, 0.02, 0.1, true);

pub static BRAIN_PROVIDER: Provider = brain::provider(
    &[SensorType::NearestLivingEntities, SensorType::NearestPlayers, SensorType::HurtBy, SensorType::FrogTemptations],
    |_| tadpole_ai::get_activities(),
);

#[derive(Default)]
pub struct Tadpole {
    pub age: i32,
    pub age_lock_particle_timer: i32,
}

fn tadpole(mob: &Mob) -> &Tadpole {
    match &mob.kind {
        MobKind::Tadpole(tadpole) => tadpole,
        _ => unreachable!("a tadpole"),
    }
}

fn tadpole_mut(mob: &mut Mob) -> &mut Tadpole {
    match &mut mob.kind {
        MobKind::Tadpole(tadpole) => tadpole,
        _ => unreachable!("a tadpole"),
    }
}

fn is_food(item_stack: &ItemStack) -> bool {
    item_stack.get_item().is(&item_tags::FROG_FOOD)
}

pub fn set_age_locked(mob: &mut Mob, locked: bool) {
    mob.entity.data.set(ids::AGE_LOCKED, DataValue::Boolean(locked));
}

pub fn get_flop_sound() -> &'static str {
    "minecraft:entity.tadpole.flop"
}

pub fn get_hurt_sound() -> &'static str {
    "minecraft:entity.tadpole.hurt"
}

pub fn get_death_sound() -> &'static str {
    "minecraft:entity.tadpole.death"
}

pub fn get_pickup_sound() -> &'static str {
    "minecraft:item.bucket.fill_tadpole"
}

pub fn get_bucket_item_stack() -> ItemStack {
    ItemStack::new(items::TADPOLE_BUCKET, 1)
}

pub fn should_drop_experience() -> bool {
    false
}

impl Tadpole {
    pub fn create() -> Mob {
        let mut data = abstract_fish_defaults();
        data.push(DataValue::Boolean(false));
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:tadpole"), DIMENSIONS, data);
        let mut mob = Mob::new(entity, LivingEntity::new(&TADPOLE_ATTRIBUTES), MobKind::Tadpole(Tadpole::default()));
        water_animal::init(&mut mob);
        mob.ai.move_control.kind = MoveControlKind::SmoothSwimming(MOVE_CONTROL);
        mob.ai.look_control.kind = LookControlKind::SmoothSwimming(10);
        mob
    }

    pub fn make_brain(mob: &mut Mob, packed_brain: brain::Packed) -> Brain {
        BRAIN_PROVIDER.make_brain(mob, packed_brain)
    }

    pub fn add_additional_save_data(mob: &Mob, out: &mut Vec<(String, Nbt)>) {
        out.push(("Age".into(), Nbt::Int(tadpole(mob).age)));
        out.push(("AgeLocked".into(), Nbt::Byte(mob.is_age_locked() as i8)));
    }

    pub fn read_additional_save_data(mob: &mut Mob, nbt: &Nbt) {
        tadpole_mut(mob).age = tag_value_input::get_i64(nbt, "Age").map_or(DEFAULT_AGE, |v| v as i32);
        set_age_locked(mob, tag_value_input::get_i64(nbt, "AgeLocked").is_some_and(|v| v != 0));
    }

    pub fn save_to_bucket_tag(mob: &Mob, bucket: &mut ItemStack) {
        bucketable::save_default_data_to_bucket_tag(mob, bucket);
        let mut tag = match bucket.get(crate::world::item::component::BUCKET_ENTITY_DATA) {
            Some(Nbt::Compound(tag)) => tag.clone(),
            _ => Vec::new(),
        };
        let mut put = |key: &str, value: Nbt| {
            tag.retain(|(k, _)| k != key);
            tag.push((key.to_string(), value));
        };
        put("Age", Nbt::Int(tadpole(mob).age));
        put("AgeLocked", Nbt::Byte(mob.is_age_locked() as i8));
        bucket.set(crate::world::item::component::BUCKET_ENTITY_DATA, Some(Nbt::Compound(tag)));
    }

    pub fn load_from_bucket_tag(mob: &mut Mob, tag: &Nbt) {
        bucketable::load_default_data_from_bucket_tag(mob, tag);
        if let Some(age) = tag.get("Age").and_then(|nbt| match nbt {
            Nbt::Int(v) => Some(*v),
            _ => None,
        }) {
            tadpole_mut(mob).age = age;
        }
        set_age_locked(mob, tag_value_input::get_i64(tag, "AgeLocked").is_some_and(|v| v != 0));
    }
}

impl ServerLevel {
    pub fn tadpole_custom_server_ai_step(&mut self, id: EntityId) {
        Brain::tick_in_zone(self, id, "tadpoleBrain");
        if self.mob(id).is_none() {
            return;
        }
        {
            let _zone = profiler::zone("tadpoleActivityUpdate");
            tadpole_ai::update_activity(self.get_brain_mut(id));
        }
    }

    pub fn tadpole_ai_step(&mut self, id: EntityId) {
        self.abstract_fish_ai_step(id);
        let Some(mob) = self.mob(id) else { return };
        if !mob.is_age_locked() {
            let age = tadpole(mob).age;
            self.tadpole_set_age(id, age + 1);
        }
        let Some(mob) = self.mob_mut(id) else { return };
        if mob.entity.is_removed() {
            return;
        }
        let locked = mob.is_age_locked();
        let timer = tadpole(mob).age_lock_particle_timer;
        if timer > 0 {
            if timer % 2 == 0 {
                let y_offset = if locked { 0.2 } else { 0.0 };
                let height = mob.entity.dimensions.height as f64;
                let e = &mut mob.entity;
                let x = e.random_x(1.0);
                let y = e.random_y_scaled(0.2) + height + y_offset;
                let z = e.random_z(1.0);
                let particle = if locked { "minecraft:pause_mob_growth" } else { "minecraft:reset_mob_growth" };
                self.send_particles(particle, |_| {}, x, y, z, 1, (0.0, 0.0, 0.0), 0.0);
            }
            tadpole_mut(self.mob_mut(id).expect("a tadpole")).age_lock_particle_timer -= 1;
        }
    }

    pub fn tadpole_mob_interact(&mut self, id: EntityId, player: EntityId, hand: InteractionHand) -> InteractionResult {
        let mut item_stack = self.player(player).item_in_hand(hand).unwrap_or_else(ItemStack::empty);
        let mob = self.mob(id).expect("a tadpole");
        if is_food(&item_stack) && !mob.is_age_locked() {
            self.tadpole_feed(id, player, hand, &mut item_stack);
            return InteractionResult::SUCCESS;
        }
        if can_use_golden_dandelion(&item_stack, true, tadpole(mob).age_lock_particle_timer, mob) {
            self.set_age_locked(id, player, hand, item_stack, |level| level.tadpole_set_age_locked_data(id));
            return InteractionResult::SUCCESS;
        }
        match bucketable::bucket_mob_pickup(self, player, hand, id) {
            Some(result) => result,
            None => self.abstract_fish_mob_interact(id, player, hand),
        }
    }

    fn tadpole_set_age_locked_data(&mut self, id: EntityId) {
        let mob = self.mob_mut(id).expect("a tadpole");
        let locked = !mob.is_age_locked();
        set_age_locked(mob, locked);
        self.tadpole_set_age(id, 0);
        if let Some(mob) = self.mob_mut(id) {
            tadpole_mut(mob).age_lock_particle_timer = 40;
        }
    }

    fn tadpole_feed(&mut self, id: EntityId, player: EntityId, hand: InteractionHand, item_stack: &mut ItemStack) {
        item_stack.consume(1, self, Some(player));
        self.player_mut(player).update_item_in_hand(hand, item_stack);
        let ticks_left_until_adult = self.get_ticks_left_until_adult(id);
        self.tadpole_age_up_by(id, get_speed_up_seconds_when_feeding(ticks_left_until_adult));
        if let Some(mob) = self.mob_mut(id) {
            let e = &mut mob.entity;
            e.random_x(1.0);
            e.random_y();
            e.random_z(1.0);
        }
    }

    fn tadpole_age_up_by(&mut self, id: EntityId, ticks_to_age_up: i32) {
        let age = tadpole(self.mob(id).expect("a tadpole")).age;
        self.tadpole_set_age(id, age + ticks_to_age_up * 20);
    }

    pub fn tadpole_set_age(&mut self, id: EntityId, new_age: i32) {
        let mob = self.mob_mut(id).expect("a tadpole");
        tadpole_mut(mob).age = new_age;
        if new_age >= TICKS_TO_BE_FROG {
            self.tadpole_age_up(id);
        }
    }

    fn tadpole_age_up(&mut self, id: EntityId) {
        let params = ConversionParams::single(self, id, false, false);
        let pose = self.entity(id).pose();
        let previous = self.mob(id).expect("a tadpole").get_dimensions(pose);
        self.convert_to(id, "minecraft:frog", params, |level, frog| {
            let pos = level.entity(frog).block_position();
            let difficulty = level.get_current_difficulty_at(pos);
            level.finalize_spawn(frog, &difficulty, EntitySpawnReason::Conversion, None);
            level.mob_mut(frog).expect("a frog").persistence_required = true;
            level.fudge_position_after_size_change(frog, previous);
            level.entity_play_sound(id, sound_event("minecraft:entity.tadpole.grow_up"), 0.15, 1.0);
        });
    }

    pub fn tadpole_read_level_dependent_save_data(&mut self, id: EntityId) {
        let age = tadpole(self.mob(id).expect("a tadpole")).age;
        self.tadpole_set_age(id, age);
    }

    pub fn tadpole_set_age_detached(&mut self, entity: LevelEntity) -> LevelEntity {
        let id = entity.entity().id;
        self.entities.insert(id, entity);
        self.tadpole_read_level_dependent_save_data(id);
        self.entities.shift_remove(&id).expect("the tadpole")
    }

    fn get_ticks_left_until_adult(&self, id: EntityId) -> i32 {
        0.max(TICKS_TO_BE_FROG - tadpole(self.mob(id).expect("a tadpole")).age)
    }
}
