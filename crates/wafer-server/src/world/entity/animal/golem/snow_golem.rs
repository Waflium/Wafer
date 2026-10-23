use wafer_core::random::RandomSource;
use wafer_core::{BlockPos, mth};
use wafer_data::{blocks, damage_types, game_rule, items, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use super::abstract_golem;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::attribute::environment_attributes;
use crate::world::damagesource::DamageSource;
use crate::world::entity::ai::MobAi;
use crate::world::entity::ai::goal::goal::Goal;
use crate::world::entity::ai::goal::look_at_player_goal::LookAtPlayerGoal;
use crate::world::entity::ai::goal::random_look_around_goal::RandomLookAroundGoal;
use crate::world::entity::ai::goal::ranged_attack_goal::RangedAttackGoal;
use crate::world::entity::ai::goal::target::nearest_attackable_target_goal::{NearestAttackableTargetGoal, TargetType};
use crate::world::entity::ai::goal::water_avoiding_random_stroll_goal::WaterAvoidingRandomStrollGoal;
use crate::world::entity::attributes::SNOW_GOLEM_ATTRIBUTES;
use crate::world::entity::data::{DataValue, mob_defaults};
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::projectile::throwableitemprojectile::snowball::Snowball;
use crate::world::entity::{Entity, EntityDimensions, EntityId, sound_event};
use crate::world::item::ItemStack;
use crate::world::level::BlockGetter;
use crate::world::level::block;
use crate::world::level::update_flags::UPDATE_ALL;
use crate::world::{InteractionHand, InteractionResult};
use crate::world::level::storage::tag_value_input;

pub const DATA_PUMPKIN_ID: usize = 16;
const PUMPKIN_FLAG: i8 = 16;
const DEFAULT_PUMPKIN: bool = true;
const SHEAR_SNOW_GOLEM: &str = "minecraft:shearing/snow_golem";
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.7, 1.9).with_eye_height(1.7);

#[derive(Default)]
pub struct SnowGolem;

pub fn snow_golem_defaults() -> Vec<DataValue> {
    let mut data = mob_defaults();
    data.push(DataValue::Byte(PUMPKIN_FLAG));
    data
}

fn pumpkin_flags(entity: &Entity) -> i8 {
    match entity.data.get(DATA_PUMPKIN_ID) {
        DataValue::Byte(flags) => *flags,
        _ => unreachable!("the snow golem pumpkin flags are a byte"),
    }
}

pub fn has_pumpkin(entity: &Entity) -> bool {
    pumpkin_flags(entity) & PUMPKIN_FLAG != 0
}

pub fn set_pumpkin(entity: &mut Entity, pumpkin: bool) {
    let current = pumpkin_flags(entity);
    entity.data.set(DATA_PUMPKIN_ID, DataValue::Byte(if pumpkin { current | PUMPKIN_FLAG } else { current & -17 }));
}

fn is_enemy(_mob: EntityId, target: EntityId, level: &ServerLevel) -> bool {
    level.is_enemy(target)
}

impl SnowGolem {
    pub fn create() -> Mob {
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:snow_golem"), DIMENSIONS, snow_golem_defaults());
        Mob::new(entity, LivingEntity::new(&SNOW_GOLEM_ATTRIBUTES), MobKind::SnowGolem(SnowGolem))
    }

    pub fn register_goals(ai: &mut MobAi) {
        let goal_selector = &mut ai.goal_selector;
        goal_selector.add_goal(1, Goal::RangedAttack(RangedAttackGoal::new(1.25, 20, 10.0)));
        goal_selector.add_goal(2, Goal::WaterAvoidingRandomStroll(WaterAvoidingRandomStrollGoal::with_probability(1.0, 1.0000001e-5)));
        goal_selector.add_goal(3, Goal::LookAtPlayer(LookAtPlayerGoal::new(6.0)));
        goal_selector.add_goal(4, Goal::RandomLookAround(RandomLookAroundGoal::default()));
        let target_selector = &mut ai.target_selector;
        target_selector.add_goal(1, Goal::NearestAttackableTarget(NearestAttackableTargetGoal::with(TargetType::Mob, 10, true, false, Some(is_enemy))));
    }

    pub fn get_ambient_sound_interval() -> i32 {
        abstract_golem::get_ambient_sound_interval()
    }

    pub fn get_ambient_sound() -> &'static str {
        "minecraft:entity.snow_golem.ambient"
    }

    pub fn get_hurt_sound() -> &'static str {
        "minecraft:entity.snow_golem.hurt"
    }

    pub fn get_death_sound() -> &'static str {
        "minecraft:entity.snow_golem.death"
    }

    pub fn add_additional_save_data(mob: &Mob, out: &mut Vec<(String, Nbt)>) {
        out.push(("Pumpkin".into(), Nbt::Byte(has_pumpkin(&mob.entity) as i8)));
    }

    pub fn read_additional_save_data(mob: &mut Mob, input: &Nbt) {
        let pumpkin = tag_value_input::get_i64(input, "Pumpkin").map_or(DEFAULT_PUMPKIN, |v| v as i8 != 0);
        set_pumpkin(&mut mob.entity, pumpkin);
    }
}

impl ServerLevel {
    pub fn snow_golem_ai_step(&mut self, id: EntityId) {
        self.mob_ai_step(id);
        if self.mob(id).is_none() {
            return;
        }
        let position = self.entity(id).position();
        if self.environment_attributes().get_value(environment_attributes::SNOW_GOLEM_MELTS, position) {
            self.hurt_server(id, DamageSource::new(damage_types::ON_FIRE), 1.0);
        }
        if !self.game_rules.get_bool(game_rule::MOB_GRIEFING) {
            return;
        }
        let snow = blocks::SNOW.default_state();
        for i in 0..4 {
            let e = self.entity(id);
            let xx = mth::floor(e.x() + ((i % 2 * 2 - 1) as f32 * 0.25) as f64);
            let yy = mth::floor(e.y());
            let zz = mth::floor(e.z() + ((i / 2 % 2 * 2 - 1) as f32 * 0.25) as f64);
            let snow_pos = BlockPos::new(xx, yy, zz);
            if self.block_state(snow_pos).is_air() && block::of(snow).can_survive(snow, self, snow_pos) {
                self.set_block(snow_pos, snow, UPDATE_ALL);
                self.game_event_at_with(crate::world::level::gameevent::game_event::BLOCK_PLACE, snow_pos, crate::world::level::gameevent::game_event::Context::of_entity_and_state(Some(id), Some(snow)));
            }
        }
    }

    pub fn snow_golem_perform_ranged_attack(&mut self, id: EntityId, target: EntityId, _power: f32) {
        let (e, t) = (self.entity(id), self.entity(target));
        let xd = t.x() - e.x();
        let yd = t.eye_y() - 1.1f32 as f64;
        let zd = t.z() - e.z();
        let yo = (xd * xd + zd * zd).sqrt() * 0.2f32 as f64;
        let item_stack = ItemStack::new(items::SNOWBALL, 1);
        let snowball = Snowball::from_owner(self, id, &item_stack);
        let uncertainty = self.ranged_attack_uncertainty(id);
        self.spawn_projectile(snowball, &item_stack, |projectile| {
            let y = projectile.entity.y();
            projectile.shoot(xd, yd + yo - y, zd, 1.6, uncertainty)
        });
        let pitch = 0.4 / (self.entity_mut(id).random.next_float() * 0.4 + 0.8);
        self.entity_play_sound(id, sound_event("minecraft:entity.snow_golem.shoot"), 1.0, pitch);
    }

    pub fn snow_golem_ranged_attack_uncertainty(&self) -> f32 {
        12.0
    }

    pub fn snow_golem_mob_interact(&mut self, id: EntityId, player: EntityId, hand: InteractionHand) -> InteractionResult {
        let item_stack = self.player(player).item_in_hand(hand).unwrap_or_else(ItemStack::empty);
        if item_stack.is(items::SHEARS) && self.snow_golem_ready_for_shearing(id) {
            self.snow_golem_shear(id, SoundSource::Players, &item_stack);
            self.entity_game_event_by(id, crate::world::level::gameevent::game_event::SHEAR, Some(player));
            self.hurt_and_break(player, hand.as_equipment_slot(), 1);
            InteractionResult::SUCCESS
        } else {
            InteractionResult::Pass
        }
    }

    pub fn snow_golem_shear(&mut self, id: EntityId, sound_source: SoundSource, tool: &ItemStack) {
        self.play_sound_from_entity(None, id, sound_event("minecraft:entity.snow_golem.shear"), sound_source, 1.0, 1.0);
        set_pumpkin(self.entity_mut(id), false);
        self.drop_from_shearing_loot_table(id, SHEAR_SNOW_GOLEM, tool, |l, drop| {
            let offset = l.entity(id).dimensions.eye_height;
            l.spawn_at_location(id, drop, offset);
        });
    }

    pub fn snow_golem_ready_for_shearing(&self, id: EntityId) -> bool {
        has_pumpkin(self.entity(id))
    }
}
