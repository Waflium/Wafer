use wafer_core::{BlockPos, Vec3};
use wafer_data::{blocks, item_tags, mob_effects, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use super::{hoglin_ai, hoglin_base};
use crate::server::level::ServerLevel;
use crate::world::attribute::environment_attributes;
use crate::world::effect::MobEffectInstance;
use crate::world::entity::ai::brain::{self, Brain, Provider};
use crate::world::entity::ai::sensing::sensor_type::SensorType;
use crate::world::entity::animal::Animal;
use crate::world::entity::attributes::{Attribute, HOGLIN_ATTRIBUTES};
use crate::world::entity::conversion_params::ConversionParams;
use crate::world::entity::data::{DataValue, ageable_mob_defaults};
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::{Entity, EntityDimensions, EntityId, sound_event};
use crate::world::item::ItemStack;
use crate::world::level::server_level_accessor::ServerLevelAccessor;
use crate::world::{InteractionHand, InteractionResult};

const DATA_IMMUNE_TO_ZOMBIFICATION: usize = 18;
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(1.3964844, 1.4).with_passenger_attachments(&[Vec3::new(0.0, 1.49375f32 as f64, 0.0)]);
pub const BABY_DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.75, 0.85).with_eye_height(0.625).with_passenger_attachments(&[Vec3::new(0.0, 0.875, 0.0)]);
const BABY_ATTACK_DAMAGE: f64 = 0.5;
const ADULT_ATTACK_DAMAGE: f64 = 6.0;
const DEFAULT_IMMUNE_TO_ZOMBIFICATION: bool = false;
const DEFAULT_TIME_IN_OVERWORLD: i32 = 0;
const DEFAULT_CANNOT_BE_HUNTED: bool = false;
pub const CONVERSION_TIME: i32 = 300;

pub static BRAIN_PROVIDER: Provider =
    brain::provider(&[SensorType::NearestLivingEntities, SensorType::NearestPlayers, SensorType::NearestAdult, SensorType::HoglinSpecificSensor], |_| hoglin_ai::get_activities());

#[derive(Default)]
pub struct Hoglin {
    pub animal: Animal,
    pub attack_animation_remaining_ticks: i32,
    pub time_in_overworld: i32,
    cannot_be_hunted: bool,
}

fn hoglin(mob: &Mob) -> &Hoglin {
    match &mob.kind {
        MobKind::Hoglin(hoglin) => hoglin,
        _ => unreachable!("a hoglin"),
    }
}

fn hoglin_mut(mob: &mut Mob) -> &mut Hoglin {
    match &mut mob.kind {
        MobKind::Hoglin(hoglin) => hoglin,
        _ => unreachable!("a hoglin"),
    }
}

pub fn is_food(item_stack: &ItemStack) -> bool {
    item_stack.get_item().is(&item_tags::HOGLIN_FOOD)
}

pub fn is_adult(mob: &Mob) -> bool {
    !mob.is_baby()
}

pub fn is_immune_to_zombification(entity: &Entity) -> bool {
    matches!(entity.data.get(DATA_IMMUNE_TO_ZOMBIFICATION), DataValue::Boolean(true))
}

pub fn set_immune_to_zombification(entity: &mut Entity, is_immune_to_zombification: bool) {
    entity.data.set(DATA_IMMUNE_TO_ZOMBIFICATION, DataValue::Boolean(is_immune_to_zombification));
}

pub fn get_hurt_sound() -> &'static str {
    "minecraft:entity.hoglin.hurt"
}

pub fn get_death_sound() -> &'static str {
    "minecraft:entity.hoglin.death"
}

pub fn get_step_sound() -> &'static str {
    "minecraft:entity.hoglin.step"
}

pub fn finalize_spawn(level: &mut (impl ServerLevelAccessor + ?Sized), mob: &mut Mob) {
    if level.get_random().next_float() < hoglin_base::PROBABILITY_OF_SPAWNING_AS_BABY {
        mob.set_baby(true);
    }
}

impl Hoglin {
    pub fn create() -> Mob {
        let mut data = ageable_mob_defaults();
        data.push(DataValue::Boolean(DEFAULT_IMMUNE_TO_ZOMBIFICATION));
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:hoglin"), DIMENSIONS, data);
        let mut mob = Mob::new(entity, LivingEntity::new(&HOGLIN_ATTRIBUTES), MobKind::Hoglin(Hoglin::default()));
        Animal::init(&mut mob);
        mob.xp_reward = 5;
        mob
    }

    pub fn make_brain(mob: &mut Mob, packed_brain: brain::Packed) -> Brain {
        BRAIN_PROVIDER.make_brain(mob, packed_brain)
    }

    pub fn age_boundary_reached(mob: &mut Mob) {
        let (xp_reward, attack_damage) = if mob.is_baby() { (3, BABY_ATTACK_DAMAGE) } else { (5, ADULT_ATTACK_DAMAGE) };
        mob.xp_reward = xp_reward;
        mob.living.attributes.instance(Attribute::AttackDamage).expect("hoglins have an attack damage").set_base_value(attack_damage);
    }

    pub fn add_additional_save_data(mob: &Mob, out: &mut Vec<(String, Nbt)>) {
        out.push(("IsImmuneToZombification".into(), Nbt::Byte(is_immune_to_zombification(&mob.entity) as i8)));
        out.push(("TimeInOverworld".into(), Nbt::Int(hoglin(mob).time_in_overworld)));
        out.push(("CannotBeHunted".into(), Nbt::Byte(hoglin(mob).cannot_be_hunted as i8)));
    }

    pub fn read_additional_save_data(mob: &mut Mob, nbt: &Nbt) {
        use crate::world::level::block::entity::{byte_or, int_or};
        set_immune_to_zombification(&mut mob.entity, byte_or(nbt, "IsImmuneToZombification", DEFAULT_IMMUNE_TO_ZOMBIFICATION as i8) != 0);
        hoglin_mut(mob).time_in_overworld = int_or(nbt, "TimeInOverworld", DEFAULT_TIME_IN_OVERWORLD);
        hoglin_mut(mob).cannot_be_hunted = byte_or(nbt, "CannotBeHunted", DEFAULT_CANNOT_BE_HUNTED as i8) != 0;
    }

    pub fn get_walk_target_value(mob: &Mob, pos: BlockPos, level: &mut (impl ServerLevelAccessor + ?Sized)) -> f32 {
        if hoglin_ai::is_pos_near_nearest_repellent(mob, pos) {
            -1.0
        } else if level.block_state(pos.below()).is_block(blocks::CRIMSON_NYLIUM) {
            10.0
        } else {
            0.0
        }
    }
}

impl ServerLevel {
    pub fn hoglin_is_converting(&self, id: EntityId) -> bool {
        let mob = self.mob(id).expect("a hoglin");
        !is_immune_to_zombification(&mob.entity) && !mob.is_no_ai() && self.environment_attributes().get_value(environment_attributes::PIGLINS_ZOMBIFY, mob.entity.position())
    }

    pub fn hoglin_do_hurt_target(&mut self, id: EntityId, target: EntityId) -> bool {
        if !self.is_living(target) {
            return false;
        }
        hoglin_mut(self.mob_mut(id).expect("a hoglin")).attack_animation_remaining_ticks = 10;
        self.broadcast_entity_event(id, 4);
        self.make_sound(id, sound_event("minecraft:entity.hoglin.attack"));
        hoglin_ai::on_hit_target(self, id, target);
        hoglin_base::hurt_and_throw_target(self, id, target)
    }

    pub fn hoglin_blocked_by_item(&mut self, id: EntityId, defender: EntityId) {
        if is_adult(self.mob(id).expect("a hoglin")) {
            hoglin_base::throw_target(self, id, defender);
        }
    }

    pub fn hoglin_hurt_server(&mut self, id: EntityId, source: crate::world::damagesource::DamageSource, damage: f32) -> bool {
        let source_entity = source.causing_entity;
        let was_hurt = self.living_entity_hurt_server(id, source, damage);
        if was_hurt && let Some(source_entity) = source_entity.filter(|&entity| self.is_living(entity)) {
            hoglin_ai::was_hurt_by(self, id, source_entity);
        }
        was_hurt
    }

    pub fn hoglin_custom_server_ai_step(&mut self, id: EntityId) {
        Brain::tick_in_zone(self, id, "hoglinBrain");
        if self.mob(id).is_none() {
            return;
        }
        hoglin_ai::update_activity(self, id);
        if self.hoglin_is_converting(id) {
            let hoglin = hoglin_mut(self.mob_mut(id).expect("a hoglin"));
            hoglin.time_in_overworld += 1;
            if hoglin.time_in_overworld > CONVERSION_TIME {
                self.make_sound(id, sound_event("minecraft:entity.hoglin.converted_to_zombified"));
                self.hoglin_finish_conversion(id);
            }
        } else {
            hoglin_mut(self.mob_mut(id).expect("a hoglin")).time_in_overworld = 0;
        }
    }

    fn hoglin_finish_conversion(&mut self, id: EntityId) {
        let params = ConversionParams::single(self, id, true, false);
        self.convert_to(id, "minecraft:zoglin", params, |level, zoglin| {
            level.add_effect(zoglin, MobEffectInstance::new(mob_effects::NAUSEA, 200, 0), None);
        });
    }

    pub fn hoglin_ai_step(&mut self, id: EntityId) {
        let hoglin = hoglin_mut(self.mob_mut(id).expect("a hoglin"));
        if hoglin.attack_animation_remaining_ticks > 0 {
            hoglin.attack_animation_remaining_ticks -= 1;
        }
        self.animal_ai_step(id);
    }

    pub fn hoglin_mob_interact(&mut self, id: EntityId, player: EntityId, hand: InteractionHand) -> InteractionResult {
        let interaction_succeeded = self.animal_mob_interact(id, player, hand);
        if interaction_succeeded.consumes_action() {
            self.mob_mut(id).expect("a hoglin").persistence_required = true;
        }
        interaction_succeeded
    }

    pub fn hoglin_get_breed_offspring(&mut self, _id: EntityId, _partner: EntityId) -> Option<Box<Mob>> {
        let mut offspring = Box::new(Hoglin::create());
        offspring.persistence_required = true;
        Some(offspring)
    }

    pub fn hoglin_can_fall_in_love(&self, id: EntityId) -> bool {
        !hoglin_ai::is_pacified(self, id) && self.animal_can_fall_in_love(id)
    }

    pub fn hoglin_get_walk_target_value(&self, id: EntityId, pos: BlockPos) -> f32 {
        if hoglin_ai::is_pos_near_nearest_repellent(self.mob(id).expect("a hoglin"), pos) {
            -1.0
        } else if crate::world::level::path_navigation_region::PathNavigationRegion::new(self).get_block_state(pos.below()).is_block(blocks::CRIMSON_NYLIUM) {
            10.0
        } else {
            0.0
        }
    }

    pub fn hoglin_can_be_hunted(&self, id: EntityId) -> bool {
        let mob = self.mob(id).expect("a hoglin");
        is_adult(mob) && !hoglin(mob).cannot_be_hunted
    }

    pub fn hoglin_get_ambient_sound(&self, id: EntityId) -> Option<&'static str> {
        hoglin_ai::get_sound_for_current_activity(self, id)
    }
}
