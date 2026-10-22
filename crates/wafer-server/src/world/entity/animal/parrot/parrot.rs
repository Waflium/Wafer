use wafer_core::random::RandomSource;
use wafer_core::{BlockPos, Vec3, mth};
use wafer_data::{block_tags, blocks, damage_types, item_tags, mob_effects, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use super::shoulder_riding_entity::ShoulderRidingEntity;
use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::difficulty::Difficulty;
use crate::world::effect::mob_effect_instance::MobEffectInstance;
use crate::world::entity::ageable_mob::AgeableMobGroupData;
use crate::world::entity::ai::MobAi;
use crate::world::entity::ai::control::flying_move_control::FlyingMoveControl;
use crate::world::entity::ai::control::move_control::MoveControlKind;
use crate::world::entity::ai::goal::float_goal::FloatGoal;
use crate::world::entity::ai::goal::follow_mob_goal::FollowMobGoal;
use crate::world::entity::ai::goal::follow_owner_goal::FollowOwnerGoal;
use crate::world::entity::ai::goal::goal::Goal;
use crate::world::entity::ai::goal::land_on_owners_shoulder_goal::LandOnOwnersShoulderGoal;
use crate::world::entity::ai::goal::look_at_player_goal::LookAtPlayerGoal;
use crate::world::entity::ai::goal::panic_goal::PanicGoal;
use crate::world::entity::ai::goal::sit_when_ordered_to_goal::SitWhenOrderedToGoal;
use crate::world::entity::ai::goal::water_avoiding_random_stroll_goal::WaterAvoidingRandomStrollGoal;
use crate::world::entity::ai::util::{air_and_water_random_pos, hover_random_pos, land_random_pos};
use crate::world::entity::animal::Animal;
use crate::world::entity::attributes::PARROT_ATTRIBUTES;
use crate::world::entity::data::DataValue;
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::spawn_group_data::SpawnGroupData;
use crate::world::entity::tamable_animal::{TamableAnimalPanicGoal, tamable_animal_defaults};
use crate::world::entity::{Entity, EntityDimensions, EntityId, calculate_view_vector, entity_event, sound_event};
use crate::world::item::ItemStack;
use crate::world::level::BlockGetter;
use crate::world::level::server_level_accessor::ServerLevelAccessor;
use crate::world::{InteractionHand, InteractionResult};

pub const DATA_VARIANT_ID: usize = 20;
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.5, 0.9).with_eye_height(0.54).with_passenger_attachments(&[Vec3::new(0.0, 0.4625f32 as f64, 0.0)]);
const MOVE_CONTROL: FlyingMoveControl = FlyingMoveControl::new(10, false);

const MOB_SOUND_MAP: [(&str, &str); 42] = [
    ("minecraft:blaze", "minecraft:entity.parrot.imitate.blaze"),
    ("minecraft:bogged", "minecraft:entity.parrot.imitate.bogged"),
    ("minecraft:breeze", "minecraft:entity.parrot.imitate.breeze"),
    ("minecraft:camel_husk", "minecraft:entity.parrot.imitate.camel_husk"),
    ("minecraft:cave_spider", "minecraft:entity.parrot.imitate.spider"),
    ("minecraft:creaking", "minecraft:entity.parrot.imitate.creaking"),
    ("minecraft:creeper", "minecraft:entity.parrot.imitate.creeper"),
    ("minecraft:drowned", "minecraft:entity.parrot.imitate.drowned"),
    ("minecraft:elder_guardian", "minecraft:entity.parrot.imitate.elder_guardian"),
    ("minecraft:ender_dragon", "minecraft:entity.parrot.imitate.ender_dragon"),
    ("minecraft:endermite", "minecraft:entity.parrot.imitate.endermite"),
    ("minecraft:evoker", "minecraft:entity.parrot.imitate.evoker"),
    ("minecraft:ghast", "minecraft:entity.parrot.imitate.ghast"),
    ("minecraft:happy_ghast", "minecraft:intentionally_empty"),
    ("minecraft:guardian", "minecraft:entity.parrot.imitate.guardian"),
    ("minecraft:hoglin", "minecraft:entity.parrot.imitate.hoglin"),
    ("minecraft:husk", "minecraft:entity.parrot.imitate.husk"),
    ("minecraft:illusioner", "minecraft:entity.parrot.imitate.illusioner"),
    ("minecraft:magma_cube", "minecraft:entity.parrot.imitate.magma_cube"),
    ("minecraft:parched", "minecraft:entity.parrot.imitate.parched"),
    ("minecraft:phantom", "minecraft:entity.parrot.imitate.phantom"),
    ("minecraft:piglin", "minecraft:entity.parrot.imitate.piglin"),
    ("minecraft:piglin_brute", "minecraft:entity.parrot.imitate.piglin_brute"),
    ("minecraft:pillager", "minecraft:entity.parrot.imitate.pillager"),
    ("minecraft:ravager", "minecraft:entity.parrot.imitate.ravager"),
    ("minecraft:shulker", "minecraft:entity.parrot.imitate.shulker"),
    ("minecraft:silverfish", "minecraft:entity.parrot.imitate.silverfish"),
    ("minecraft:skeleton", "minecraft:entity.parrot.imitate.skeleton"),
    ("minecraft:slime", "minecraft:entity.parrot.imitate.slime"),
    ("minecraft:spider", "minecraft:entity.parrot.imitate.spider"),
    ("minecraft:stray", "minecraft:entity.parrot.imitate.stray"),
    ("minecraft:vex", "minecraft:entity.parrot.imitate.vex"),
    ("minecraft:vindicator", "minecraft:entity.parrot.imitate.vindicator"),
    ("minecraft:warden", "minecraft:entity.parrot.imitate.warden"),
    ("minecraft:witch", "minecraft:entity.parrot.imitate.witch"),
    ("minecraft:wither", "minecraft:entity.parrot.imitate.wither"),
    ("minecraft:wither_skeleton", "minecraft:entity.parrot.imitate.wither_skeleton"),
    ("minecraft:zoglin", "minecraft:entity.parrot.imitate.zoglin"),
    ("minecraft:zombie", "minecraft:entity.parrot.imitate.zombie"),
    ("minecraft:zombie_horse", "minecraft:entity.parrot.imitate.zombie_horse"),
    ("minecraft:zombie_nautilus", "minecraft:entity.parrot.imitate.zombie_nautilus"),
    ("minecraft:zombie_villager", "minecraft:entity.parrot.imitate.zombie_villager"),
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Variant {
    RedBlue,
    Blue,
    Green,
    YellowBlue,
    Gray,
}

impl Variant {
    pub const VALUES: [Variant; 5] = [Variant::RedBlue, Variant::Blue, Variant::Green, Variant::YellowBlue, Variant::Gray];

    pub fn by_id(id: i32) -> Variant {
        Variant::VALUES[id.clamp(0, 4) as usize]
    }

    pub fn by_name(name: &str) -> Option<Variant> {
        Variant::VALUES.into_iter().find(|variant| variant.get_serialized_name() == name)
    }

    pub fn get_serialized_name(self) -> &'static str {
        match self {
            Variant::RedBlue => "red_blue",
            Variant::Blue => "blue",
            Variant::Green => "green",
            Variant::YellowBlue => "yellow_blue",
            Variant::Gray => "gray",
        }
    }
}

pub struct Parrot {
    pub shoulder_riding_entity: ShoulderRidingEntity,
    pub flap: f32,
    pub flap_speed: f32,
    pub o_flap_speed: f32,
    pub o_flap: f32,
    flapping: f32,
    next_flap: f32,
    party_parrot: bool,
    jukebox: Option<BlockPos>,
}

impl Default for Parrot {
    fn default() -> Parrot {
        Parrot {
            shoulder_riding_entity: ShoulderRidingEntity::default(),
            flap: 0.0,
            flap_speed: 0.0,
            o_flap_speed: 0.0,
            o_flap: 0.0,
            flapping: 1.0,
            next_flap: 1.0,
            party_parrot: false,
            jukebox: None,
        }
    }
}

pub fn parrot(mob: &Mob) -> &Parrot {
    match &mob.kind {
        MobKind::Parrot(parrot) => parrot,
        _ => unreachable!("not a parrot"),
    }
}

pub fn parrot_mut(mob: &mut Mob) -> &mut Parrot {
    match &mut mob.kind {
        MobKind::Parrot(parrot) => parrot,
        _ => unreachable!("not a parrot"),
    }
}

pub fn get_variant(entity: &Entity) -> Variant {
    match entity.data.get(DATA_VARIANT_ID) {
        DataValue::Int(id) => Variant::by_id(*id),
        _ => Variant::RedBlue,
    }
}

pub fn set_variant(entity: &mut Entity, variant: Variant) {
    entity.data.set(DATA_VARIANT_ID, DataValue::Int(variant as i32));
}

pub fn get_pitch(random: &mut impl RandomSource) -> f32 {
    (random.next_float() - random.next_float()) * 0.2 + 1.0
}

fn get_imitated_sound(entity_type: &str) -> &'static str {
    MOB_SOUND_MAP.iter().find(|(key, _)| *key == entity_type).map_or("minecraft:entity.parrot.ambient", |(_, sound)| sound)
}

pub fn get_ambient(level: &mut ServerLevel) -> &'static str {
    if level.difficulty != Difficulty::Peaceful && level.random.next_int_bounded(1000) == 0 {
        let index = level.random.next_int_bounded(MOB_SOUND_MAP.len() as i32);
        get_imitated_sound(MOB_SOUND_MAP[index as usize].0)
    } else {
        "minecraft:entity.parrot.ambient"
    }
}

fn imitated_type(level: &ServerLevel, entity: EntityId) -> Option<&'static str> {
    let entity_type = registry::ENTITY_TYPE[level.entity(entity).entity_type as usize];
    MOB_SOUND_MAP.iter().any(|(key, _)| *key == entity_type).then_some(entity_type)
}

pub fn imitate_nearby_mobs(level: &mut ServerLevel, entity: EntityId) -> bool {
    if !level.is_alive(entity) || level.entity(entity).is_silent() || level.random.next_int_bounded(2) != 0 {
        return false;
    }
    let area = level.entity(entity).bounding_box().inflate_all(20.0);
    let mobs = level.get_entities_matching(None, &area, |level, candidate| level.is_mob(candidate) && imitated_type(level, candidate).is_some());
    if mobs.is_empty() {
        return false;
    }
    let mob = mobs[level.random.next_int_bounded(mobs.len() as i32) as usize];
    if level.entity(mob).is_silent() {
        return false;
    }
    let sound = get_imitated_sound(imitated_type(level, mob).expect("an imitated mob"));
    let e = level.entity(entity);
    let (x, y, z) = (e.x(), e.y(), e.z());
    let source = level.sound_source(entity);
    let pitch = get_pitch(&mut level.random);
    level.play_sound_at(None, x, y, z, sound_event(sound), source, 0.7, pitch);
    true
}

pub fn get_hurt_sound() -> &'static str {
    "minecraft:entity.parrot.hurt"
}

pub fn get_death_sound() -> &'static str {
    "minecraft:entity.parrot.death"
}

pub fn get_step_sound() -> &'static str {
    "minecraft:entity.parrot.step"
}

impl Parrot {
    pub fn create() -> Mob {
        let mut data = tamable_animal_defaults();
        data.push(DataValue::Int(Variant::RedBlue as i32));
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:parrot"), DIMENSIONS, data);
        let mut mob = Mob::new(entity, LivingEntity::new(&PARROT_ATTRIBUTES), MobKind::Parrot(Parrot::default()));
        Animal::init(&mut mob);
        mob.ai.move_control.kind = MoveControlKind::Flying(MOVE_CONTROL);
        mob.set_pathfinding_malus(crate::world::level::pathfinder::path_type::PathType::FireInNeighbor, -1.0);
        mob.set_pathfinding_malus(crate::world::level::pathfinder::path_type::PathType::Fire, -1.0);
        mob.set_pathfinding_malus(crate::world::level::pathfinder::path_type::PathType::Cocoa, -1.0);
        mob
    }

    pub fn register_goals(ai: &mut MobAi) {
        ai.navigation.path_finder.node_evaluator.node_evaluator_mut().can_open_doors = false;
        ai.navigation.set_can_float(true);
        let float_goal = FloatGoal::new(&mut ai.navigation);
        let goal_selector = &mut ai.goal_selector;
        goal_selector.add_goal(0, Goal::TamableAnimalPanic(TamableAnimalPanicGoal { panic_goal: PanicGoal::new(1.25) }));
        goal_selector.add_goal(0, Goal::Float(float_goal));
        goal_selector.add_goal(1, Goal::LookAtPlayer(LookAtPlayerGoal::new(8.0)));
        goal_selector.add_goal(2, Goal::SitWhenOrderedTo(SitWhenOrderedToGoal));
        goal_selector.add_goal(2, Goal::FollowOwner(FollowOwnerGoal::new(1.0, 5.0, 1.0)));
        goal_selector.add_goal(2, Goal::ParrotWander(ParrotWanderGoal::new(1.0)));
        goal_selector.add_goal(3, Goal::LandOnOwnersShoulder(LandOnOwnersShoulderGoal::default()));
        goal_selector.add_goal(3, Goal::FollowMob(FollowMobGoal::new(1.0, 3.0, 7.0)));
    }

    pub fn add_additional_save_data(mob: &Mob, output: &mut Vec<(String, Nbt)>) {
        crate::world::entity::tamable_animal::TamableAnimal::add_additional_save_data(mob, output);
        output.push(("Variant".into(), Nbt::Int(get_variant(&mob.entity) as i32)));
    }

    pub fn read_additional_save_data(mob: &mut Mob, input: &Nbt) {
        crate::world::entity::tamable_animal::TamableAnimal::read_additional_save_data(mob, input);
        let variant = input.get("Variant").and_then(Nbt::as_i64).map_or(Variant::RedBlue, |id| Variant::by_id(id as i32));
        set_variant(&mut mob.entity, variant);
    }
}

pub fn finalize_spawn(level: &mut (impl ServerLevelAccessor + ?Sized), mob: &mut Mob, group_data: Option<SpawnGroupData>) -> Option<SpawnGroupData> {
    let variant = Variant::VALUES[level.get_random().next_int_bounded(Variant::VALUES.len() as i32) as usize];
    set_variant(&mut mob.entity, variant);
    group_data.or(Some(SpawnGroupData::AgeableMob(AgeableMobGroupData::new(false))))
}

impl ServerLevel {
    pub fn is_parrot(&self, id: EntityId) -> bool {
        self.mob(id).is_some_and(|mob| matches!(mob.kind, MobKind::Parrot(_)))
    }

    pub fn parrot_tick(&mut self, id: EntityId) {
        parrot_mut(self.mob_mut(id).expect("a parrot")).shoulder_riding_entity.ride_cooldown_counter += 1;
        self.mob_tick(id);
    }

    pub fn parrot_ai_step(&mut self, id: EntityId) {
        let position = self.entity(id).position();
        let jukebox = parrot(self.mob(id).expect("a parrot")).jukebox;
        if jukebox.is_none_or(|jukebox| !jukebox.closer_to_center_than(position, 3.46) || !self.block_state(jukebox).is_block(blocks::JUKEBOX)) {
            let parrot = parrot_mut(self.mob_mut(id).expect("a parrot"));
            parrot.party_parrot = false;
            parrot.jukebox = None;
        }
        if self.random.next_int_bounded(400) == 0 {
            imitate_nearby_mobs(self, id);
        }
        self.animal_ai_step(id);
        if self.mob(id).is_some() {
            self.parrot_calculate_flapping(id);
        }
    }

    fn parrot_calculate_flapping(&mut self, id: EntityId) {
        let is_passenger = self.is_passenger(id);
        let mob = self.mob_mut(id).expect("a parrot");
        let on_ground = mob.entity.on_ground;
        let parrot = parrot_mut(mob);
        parrot.o_flap = parrot.flap;
        parrot.o_flap_speed = parrot.flap_speed;
        parrot.flap_speed += (if !on_ground && !is_passenger { 4 } else { -1 }) as f32 * 0.3;
        parrot.flap_speed = mth::clamp_f(parrot.flap_speed, 0.0, 1.0);
        if !on_ground && parrot.flapping < 1.0 {
            parrot.flapping = 1.0;
        }
        parrot.flapping *= 0.9;
        let flapping = parrot.flapping;
        let movement = mob.entity.delta_movement;
        if !on_ground && movement.y < 0.0 {
            mob.entity.set_delta_movement(movement.multiply(1.0, 0.6, 1.0));
        }
        parrot_mut(mob).flap += flapping * 2.0;
    }

    pub fn parrot_is_flapping(&self, id: EntityId) -> bool {
        let mob = self.mob(id).expect("a parrot");
        mob.entity.fly_dist > parrot(mob).next_flap
    }

    pub fn parrot_on_flap(&mut self, id: EntityId) {
        self.entity_play_sound(id, sound_event("minecraft:entity.parrot.fly"), 0.15, 1.0);
        let mob = self.mob_mut(id).expect("a parrot");
        let fly_dist = mob.entity.fly_dist;
        let parrot = parrot_mut(mob);
        parrot.next_flap = fly_dist + parrot.flap_speed / 2.0;
    }

    pub fn parrot_mob_interact(&mut self, id: EntityId, player: EntityId, hand: InteractionHand) -> InteractionResult {
        let mut item_stack = self.player(player).item_in_hand(hand).unwrap_or_else(ItemStack::empty);
        if !self.is_tame(id) && item_stack.get_item().is(&item_tags::PARROT_FOOD) {
            self.use_player_item(player, hand, &mut item_stack);
            if !self.entity(id).is_silent() {
                let e = self.entity(id);
                let (x, y, z) = (e.x(), e.y(), e.z());
                let random = &mut self.entity_mut(id).random;
                let pitch = 1.0 + (random.next_float() - random.next_float()) * 0.2;
                let source = self.sound_source(id);
                self.play_sound_at(None, x, y, z, sound_event("minecraft:entity.parrot.eat"), source, 1.0, pitch);
            }
            if self.entity_mut(id).random.next_int_bounded(10) == 0 {
                self.tame(id, player);
                self.broadcast_entity_event(id, entity_event::TAMING_SUCCEEDED);
            } else {
                self.broadcast_entity_event(id, entity_event::TAMING_FAILED);
            }
            return InteractionResult::SUCCESS;
        }
        if !item_stack.get_item().is(&item_tags::PARROT_POISONOUS_FOOD) {
            if !self.parrot_is_flying(id) && self.is_tame(id) && self.is_owned_by(id, player) {
                let ordered_to_sit = self.is_ordered_to_sit(id);
                self.set_ordered_to_sit(id, !ordered_to_sit);
                return InteractionResult::SUCCESS;
            }
            return self.animal_mob_interact(id, player, hand);
        }
        self.use_player_item(player, hand, &mut item_stack);
        self.add_effect(id, MobEffectInstance::new(mob_effects::POISON, 900, 0), None);
        if self.player(player).game_mode.game_type.is_creative() || !self.entity(id).is_invulnerable() {
            self.hurt(id, DamageSource::entity(damage_types::PLAYER_ATTACK, player), f32::MAX);
        }
        InteractionResult::SUCCESS
    }

    pub fn parrot_is_flying(&self, id: EntityId) -> bool {
        !self.entity(id).on_ground
    }

    pub fn parrot_hurt_server(&mut self, id: EntityId, source: DamageSource, damage: f32) -> bool {
        if self.is_invulnerable_to(id, &source) {
            return false;
        }
        self.set_ordered_to_sit(id, false);
        self.living_entity_hurt_server(id, source, damage)
    }

    pub fn parrot_voice_pitch(&mut self, id: EntityId) -> f32 {
        get_pitch(&mut self.entity_mut(id).random)
    }
}

pub struct ParrotWanderGoal {
    pub water_avoiding_random_stroll_goal: WaterAvoidingRandomStrollGoal,
}

impl ParrotWanderGoal {
    pub fn new(speed_modifier: f64) -> ParrotWanderGoal {
        ParrotWanderGoal { water_avoiding_random_stroll_goal: WaterAvoidingRandomStrollGoal::new(speed_modifier) }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        let probability = self.water_avoiding_random_stroll_goal.probability;
        self.water_avoiding_random_stroll_goal.random_stroll_goal.can_use(level, mob, |level, mob| get_position(level, mob, probability))
    }
}

fn get_position(level: &mut ServerLevel, mob: EntityId, probability: f32) -> Option<Vec3> {
    let mut pos = None;
    if level.entity(mob).is_in_water() {
        pos = land_random_pos::get_pos(level, mob, 15, 15);
    }
    if level.entity_mut(mob).random.next_float() >= probability {
        pos = get_tree_pos(level, mob);
    }
    pos.or_else(|| water_avoiding_random_flying_goal_get_position(level, mob))
}

fn water_avoiding_random_flying_goal_get_position(level: &mut ServerLevel, mob: EntityId) -> Option<Vec3> {
    let e = level.entity(mob);
    let wander_direction = calculate_view_vector(e.x_rot_o, level.living(mob).y_head_rot_o);
    let max_xz_radians = std::f32::consts::FRAC_PI_2;
    hover_random_pos::get_pos(level, mob, 8, 7, wander_direction.x, wander_direction.z, max_xz_radians, 3, 1)
        .or_else(|| air_and_water_random_pos::get_pos(level, mob, 8, 4, -2, wander_direction.x, wander_direction.z, max_xz_radians as f64))
}

fn get_tree_pos(level: &mut ServerLevel, mob: EntityId) -> Option<Vec3> {
    let e = level.entity(mob);
    let mob_pos = e.block_position();
    let from = BlockPos::new(mth::floor(e.x() - 3.0), mth::floor(e.y() - 6.0), mth::floor(e.z() - 3.0));
    let to = BlockPos::new(mth::floor(e.x() + 3.0), mth::floor(e.y() + 6.0), mth::floor(e.z() + 3.0));
    for pos in BlockPos::between_closed(from, to) {
        if mob_pos == pos {
            continue;
        }
        let state = level.block_state(pos.below());
        let can_sit_on = state.block().is_instance_of("LeavesBlock") || state.is(&block_tags::LOGS);
        if can_sit_on && level.block_state(pos).is_air() && level.block_state(pos.above()).is_air() {
            return Some(Vec3::at_bottom_center_of(pos));
        }
    }
    None
}
