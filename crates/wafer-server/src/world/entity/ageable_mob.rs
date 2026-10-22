use wafer_core::random::RandomSource;
use wafer_data::{entity_type_tags, items};

use super::EntityId;
use super::data::{DataValue, ids};
use super::mob::{Mob, MobKind};
use super::sound_event;
use super::spawn_group_data::SpawnGroupData;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::level::server_level_accessor::ServerLevelAccessor;
use crate::world::item::ItemStack;
use crate::world::{InteractionHand, InteractionResult};

pub const BABY_START_AGE: i32 = -24000;

#[derive(Default)]
pub struct AgeableMob {
    pub age: i32,
    pub forced_age: i32,
    pub forced_age_timer: i32,
    pub age_lock_particle_timer: i32,
}

pub fn can_use_golden_dandelion(item_in_hand: &ItemStack, is_baby: bool, cooldown: i32, mob: &Mob) -> bool {
    item_in_hand.get_item() == items::GOLDEN_DANDELION && is_baby && cooldown == 0 && !entity_type_tags::CANNOT_BE_AGE_LOCKED.contains(mob.entity.entity_type as usize)
}

pub fn get_speed_up_seconds_when_feeding(ticks_until_adult: i32) -> i32 {
    ((ticks_until_adult / 20) as f32 * 0.1) as i32
}

pub fn finalize_spawn(level: &mut (impl ServerLevelAccessor + ?Sized), mob: &mut Mob, group_data: Option<SpawnGroupData>) -> Option<SpawnGroupData> {
    let mut group_data = group_data.unwrap_or(SpawnGroupData::AgeableMob(AgeableMobGroupData::new(true)));
    if group_data.as_ageable_mob_group_data_mut().is_none() {
        panic!("class {group_data:?} cannot be cast to class AgeableMobGroupData");
    }
    let ageable_mob_group_data = group_data.as_ageable_mob_group_data_mut().expect("ageable mob group data");
    if ageable_mob_group_data.is_should_spawn_baby()
        && ageable_mob_group_data.get_group_size() > 0
        && level.get_random().next_float() <= ageable_mob_group_data.get_baby_spawn_chance()
        && mob.set_age(BABY_START_AGE)
    {
        mob.refresh_dimensions();
    }
    ageable_mob_group_data.increase_group_size_by_one();
    Some(group_data)
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct AgeableMobGroupData {
    group_size: i32,
    should_spawn_baby: bool,
    baby_spawn_chance: f32,
}

impl AgeableMobGroupData {
    pub fn with_chance(should_spawn_baby: bool, baby_spawn_chance: f32) -> AgeableMobGroupData {
        AgeableMobGroupData { group_size: 0, should_spawn_baby, baby_spawn_chance }
    }

    pub fn new(should_spawn_baby: bool) -> AgeableMobGroupData {
        AgeableMobGroupData::with_chance(should_spawn_baby, 0.05)
    }

    pub fn get_group_size(&self) -> i32 {
        self.group_size
    }

    pub fn increase_group_size_by_one(&mut self) {
        self.group_size += 1;
    }

    pub fn is_should_spawn_baby(&self) -> bool {
        self.should_spawn_baby
    }

    pub fn get_baby_spawn_chance(&self) -> f32 {
        self.baby_spawn_chance
    }
}

impl ServerLevel {
    pub fn get_breed_offspring(&mut self, id: EntityId, partner: EntityId) -> Option<Box<Mob>> {
        match self.mob(id).expect("an ageable mob").kind {
            MobKind::Chicken(_) => self.chicken_get_breed_offspring(id, partner),
            MobKind::Cow(_) => self.cow_get_breed_offspring(id, partner),
            MobKind::MushroomCow(_) => self.mushroom_cow_get_breed_offspring(id, partner),
            MobKind::Pig(_) => self.pig_get_breed_offspring(id, partner),
            MobKind::Sheep(_) => self.sheep_get_breed_offspring(id, partner),
            MobKind::Wolf(_) => self.wolf_get_breed_offspring(id, partner),
            MobKind::Goat(_) => self.goat_get_breed_offspring(id, partner),
            MobKind::Hoglin(_) => self.hoglin_get_breed_offspring(id, partner),
            MobKind::Panda(_) => self.panda_get_breed_offspring(id, partner),
            MobKind::Zoglin(_) => None,
            MobKind::PolarBear(_) => self.polar_bear_get_breed_offspring(id, partner),
            MobKind::Cat(_) => self.cat_get_breed_offspring(id, partner),
            MobKind::Ocelot(_) => Some(Box::new(super::animal::feline::ocelot::Ocelot::create())),
            MobKind::Armadillo(_) => self.armadillo_get_breed_offspring(id, partner),
            MobKind::Zombie(_)
            | MobKind::ZombieVillager(_)
            | MobKind::Husk(_)
            | MobKind::Drowned(_)
            | MobKind::Creeper(_)
            | MobKind::Spider(_)
            | MobKind::CaveSpider(_)
            | MobKind::Skeleton(_)
            | MobKind::Stray(_)
            | MobKind::Parched(_)
            | MobKind::WitherSkeleton(_)
            | MobKind::Bogged(_)
            | MobKind::Silverfish(_)
            | MobKind::Endermite(_)
            | MobKind::Giant(_) => None,
            MobKind::Enderman(_) => None,
            MobKind::Blaze(_) | MobKind::Ghast(_) | MobKind::ZombifiedPiglin(_) | MobKind::Phantom(_) => None,
            MobKind::Witch(_) | MobKind::Vindicator(_) | MobKind::Pillager(_) | MobKind::Evoker(_) | MobKind::Illusioner(_) | MobKind::Vex(_) | MobKind::Ravager(_) | MobKind::Guardian(_) | MobKind::ElderGuardian(_) => None,
            MobKind::Slime(_) | MobKind::MagmaCube(_) => None,
            MobKind::Bat(_) => None,
            MobKind::Squid(_) => self.squid_get_breed_offspring(id, partner),
            MobKind::GlowSquid(_) => self.glow_squid_get_breed_offspring(id, partner),
            MobKind::IronGolem(_) | MobKind::SnowGolem(_) => None,
            MobKind::Cod(_) | MobKind::Salmon(_) | MobKind::Pufferfish(_) | MobKind::TropicalFish(_) => None,
            MobKind::Horse(_) => self.horse_get_breed_offspring(id, partner),
            MobKind::Donkey(_) => self.donkey_get_breed_offspring(id, partner),
            MobKind::Mule(_) => self.mule_get_breed_offspring(id, partner),
            MobKind::SkeletonHorse(_) => self.skeleton_horse_get_breed_offspring(id, partner),
            MobKind::ZombieHorse(_) => self.zombie_horse_get_breed_offspring(id, partner),
            MobKind::Llama(_) | MobKind::TraderLlama(_) => self.llama_get_breed_offspring(id, partner),
            MobKind::WanderingTrader(_) => None,
            MobKind::Axolotl(_) => self.axolotl_get_breed_offspring(id, partner),
            MobKind::Frog(_) => self.frog_get_breed_offspring(id, partner),
            MobKind::Tadpole(_) => None,
            MobKind::Villager(_) => self.villager_get_breed_offspring(id, partner),
            MobKind::Fox(_) => self.fox_get_breed_offspring(id, partner),
            MobKind::Rabbit(_) => self.rabbit_get_breed_offspring(id, partner),
            MobKind::Turtle(_) => self.turtle_get_breed_offspring(id, partner),
        }
    }

    pub fn ageable_mob_mob_interact(&mut self, id: EntityId, player: EntityId, hand: InteractionHand) -> InteractionResult {
        let item_in_hand = self.player(player).item_in_hand(hand).unwrap_or_else(ItemStack::empty);
        let mob = self.mob(id).expect("an ageable mob");
        if can_use_golden_dandelion(&item_in_hand, mob.is_baby(), mob.ageable().expect("an ageable mob").age_lock_particle_timer, mob) {
            self.set_age_locked(id, player, hand, item_in_hand, |level| level.set_age_locked_data(id));
            return InteractionResult::SUCCESS;
        }
        self.mob_mob_interact(id, player, hand)
    }

    fn set_age_locked_data(&mut self, id: EntityId) {
        let locked = !self.mob(id).expect("an ageable mob").is_age_locked();
        self.mob_mut(id).expect("an ageable mob").entity.data.set(ids::AGE_LOCKED, DataValue::Boolean(locked));
        self.set_age(id, BABY_START_AGE);
        self.mob_mut(id).expect("an ageable mob").ageable_mut().expect("an ageable mob").age_lock_particle_timer = 40;
    }

    pub fn set_age_locked(&mut self, id: EntityId, player: EntityId, hand: InteractionHand, mut item_in_hand: ItemStack, set_age_lock_data: impl FnOnce(&mut ServerLevel)) {
        set_age_lock_data(self);
        item_in_hand.consume(1, self, Some(player));
        self.player_mut(player).update_item_in_hand(hand, &item_in_hand);
        let mob = self.mob_mut(id).expect("a mob");
        let is_age_locked = mob.is_age_locked();
        if is_age_locked {
            mob.persistence_required = true;
        }
        let pos = mob.entity.block_position();
        let sound = if is_age_locked { "minecraft:item.golden_dandelion.use" } else { "minecraft:item.golden_dandelion.unuse" };
        self.play_sound(None, pos, sound_event(sound), SoundSource::Players, 1.0, 1.0);
    }

    pub fn age_up(&mut self, id: EntityId, seconds: i32, forced: bool) {
        let old_age = self.mob(id).expect("an ageable mob").ageable().expect("an ageable mob").age;
        let age = (old_age + seconds * 20).min(0);
        let delta = age - old_age;
        self.set_age(id, age);
        let ageable = self.mob_mut(id).expect("an ageable mob").ageable_mut().expect("an ageable mob");
        if forced {
            ageable.forced_age += delta;
            if ageable.forced_age_timer == 0 {
                ageable.forced_age_timer = 40;
            }
        }
        if ageable.age == 0 {
            let forced_age = ageable.forced_age;
            self.set_age(id, forced_age);
        }
    }

    pub fn can_age_up(&self, id: EntityId) -> bool {
        let mob = self.mob(id).expect("an ageable mob");
        if matches!(mob.kind, MobKind::SkeletonHorse(_) | MobKind::ZombieHorse(_)) {
            return false;
        }
        mob.is_baby() && !mob.is_age_locked()
    }

    pub fn set_age(&mut self, id: EntityId, age: i32) {
        let Some(mob) = self.mob_mut(id) else { return };
        if mob.set_age(age) {
            self.refresh_mob_dimensions(id);
            if self.is_villager(id) {
                self.villager_age_boundary_reached(id);
            } else if self.is_turtle(id) {
                self.turtle_age_boundary_reached(id);
            }
        }
    }

    pub fn ageable_mob_ai_step(&mut self, id: EntityId) {
        self.mob_ai_step(id);
        let mob = self.mob_mut(id).expect("an ageable mob");
        let ageable = mob.ageable_mut().expect("an ageable mob");
        if ageable.forced_age_timer > 0 {
            if ageable.forced_age_timer % 4 == 0 {
                let e = &mut mob.entity;
                let x = e.random_x(1.0);
                let y = e.random_y() + 0.5;
                let z = e.random_z(1.0);
                self.send_particles("minecraft:happy_villager", |_| {}, x, y, z, 1, (0.0, 0.0, 0.0), 0.0);
            }
            self.mob_mut(id).expect("an ageable mob").ageable_mut().expect("an ageable mob").forced_age_timer -= 1;
        }
        if self.is_alive(id) {
            let mob = self.mob(id).expect("an ageable mob");
            let age = mob.ageable().expect("an ageable mob").age;
            if mob.is_baby() && !mob.is_age_locked() {
                self.set_age(id, age + 1);
            } else if age > 0 {
                self.set_age(id, age - 1);
            }
        }
        let mob = self.mob_mut(id).expect("an ageable mob");
        let locked = mob.is_age_locked();
        let timer = mob.ageable().expect("an ageable mob").age_lock_particle_timer;
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
            self.mob_mut(id).expect("an ageable mob").ageable_mut().expect("an ageable mob").age_lock_particle_timer -= 1;
        }
    }

    pub fn refresh_mob_dimensions(&mut self, id: EntityId) {
        let Some(mob) = self.mob_mut(id) else { return };
        let old = mob.refresh_dimensions();
        let new = mob.entity.dimensions;
        let e = &mob.entity;
        let small = new.width <= 4.0 && new.height <= 4.0;
        if !e.first_tick && !e.no_physics && small && (new.width > old.width || new.height > old.height) {
            self.fudge_position_after_size_change(id, old);
        }
    }

    pub fn on_dimensions_loaded(&mut self, id: EntityId, previous: super::EntityDimensions, previous_cube_size: Option<i32>) {
        if let Some(previous_size) = previous_cube_size {
            if self.mob(id).is_some_and(|mob| mob.get_size() != previous_size) && self.entity(id).is_in_water() && self.entity_mut(id).random.next_int_bounded(20) == 0 {
                self.do_water_splash_effect(id);
            }
            return;
        }
        let e = self.entity(id);
        let new = e.dimensions;
        let small = new.width <= 4.0 && new.height <= 4.0;
        if !e.first_tick && !e.no_physics && small && (new.width > previous.width || new.height > previous.height) && !self.is_player(id) {
            self.fudge_position_after_size_change(id, previous);
        }
    }

    pub fn fudge_position_after_size_change(&mut self, id: EntityId, previous: super::EntityDimensions) -> bool {
        let context = crate::world::phys::shapes::collision_context::CollisionContext::of(self, id);
        let e = self.entity(id);
        match self.free_position_after_size_change(&context, e.position(), e.dimensions, previous) {
            Some(position) => {
                self.entity_mut(id).set_pos(position);
                true
            }
            None => false,
        }
    }

    pub fn fudge_position_after_size_change_detached(&self, entity: &mut super::Entity, previous: super::EntityDimensions) -> bool {
        let context = crate::world::phys::shapes::collision_context::CollisionContext::of_detached(entity);
        match self.free_position_after_size_change(&context, entity.position(), entity.dimensions, previous) {
            Some(position) => {
                entity.set_pos(position);
                true
            }
            None => false,
        }
    }

    fn free_position_after_size_change(
        &self,
        context: &crate::world::phys::shapes::collision_context::CollisionContext,
        position: wafer_core::Vec3,
        new: super::EntityDimensions,
        previous: super::EntityDimensions,
    ) -> Option<wafer_core::Vec3> {
        let old_center = position.add(0.0, previous.height as f64 / 2.0, 0.0);
        let width_delta = (new.width - previous.width).max(0.0) as f64 + 1.0e-6;
        let height_delta = (new.height - previous.height).max(0.0) as f64 + 1.0e-6;
        let allowed = wafer_core::AABB::of_size(old_center, width_delta, height_delta, width_delta);
        if let Some(free) = self.find_free_position(context, &allowed, old_center, new.width as f64, new.height as f64, new.width as f64) {
            return Some(free.add(0.0, -new.height as f64 / 2.0, 0.0));
        }
        if new.width > previous.width && new.height > previous.height {
            let allowed = wafer_core::AABB::of_size(old_center, width_delta, 1.0e-6, width_delta);
            if let Some(free) = self.find_free_position(context, &allowed, old_center, new.width as f64, previous.height as f64, new.width as f64) {
                return Some(free.add(0.0, -previous.height as f64 / 2.0 + 1.0e-6, 0.0));
            }
        }
        None
    }
}
