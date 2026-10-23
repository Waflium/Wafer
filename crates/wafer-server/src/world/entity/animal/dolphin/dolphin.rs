use wafer_core::random::RandomSource;
use wafer_core::{BlockPos, Vec3, mth};
use wafer_data::{EquipmentSlot, damage_types, fluid_tags, item_tags, mob_effects, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::effect::mob_effect_instance::MobEffectInstance;
use crate::world::entity::ageable_mob::get_speed_up_seconds_when_feeding;
use crate::world::entity::ai::MobAi;
use crate::world::entity::ai::control::look_control::LookControlKind;
use crate::world::entity::ai::control::move_control::MoveControlKind;
use crate::world::entity::ai::control::smooth_swimming_move_control::SmoothSwimmingMoveControl;
use crate::world::entity::ai::goal::avoid_entity_goal::{AvoidClass, AvoidEntityGoal};
use crate::world::entity::ai::goal::breath_air_goal::BreathAirGoal;
use crate::world::entity::ai::goal::dolphin_jump_goal::DolphinJumpGoal;
use crate::world::entity::ai::goal::follow_player_ridden_entity_goal::{EntityTypeToFollow, FollowPlayerRiddenEntityGoal};
use crate::world::entity::ai::goal::goal::{Goal, reduced_tick_delay};
use crate::world::entity::ai::goal::look_at_player_goal::LookAtPlayerGoal;
use crate::world::entity::ai::goal::melee_attack_goal::MeleeAttackGoal;
use crate::world::entity::ai::goal::random_look_around_goal::RandomLookAroundGoal;
use crate::world::entity::ai::goal::random_swimming_goal::RandomSwimmingGoal;
use crate::world::entity::ai::goal::target::hurt_by_target_goal::HurtByTargetGoal;
use crate::world::entity::ai::goal::try_find_liquid_goal::TryFindLiquidGoal;
use crate::world::entity::ai::targeting::targeting_conditions::TargetingConditions;
use crate::world::entity::ai::util::default_random_pos;
use crate::world::entity::animal::ageable_water_creature::AgeableWaterCreature;
use crate::world::entity::attributes::DOLPHIN_ATTRIBUTES;
use crate::world::entity::data::{DataValue, ageable_mob_defaults};
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::{Entity, EntityDimensions, EntityId, ItemEntity, LevelEntity, MoverType, sound_event};
use crate::world::item::ItemStack;
use crate::world::level::BlockGetter;
use crate::world::level::levelgen::structure::structure::StructureHolder;
use crate::world::{InteractionHand, InteractionResult};
use crate::world::level::storage::tag_value_input;

const DATA_GOT_FISH: usize = 18;
const DATA_MOISTNESS_LEVEL: usize = 19;
const SWIM_WITH_PLAYER_TARGETING: TargetingConditions<'static> = TargetingConditions::for_non_combat().range(10.0).ignore_line_of_sight();
pub const TOTAL_AIR_SUPPLY: i32 = 4800;
const TOTAL_MOISTNESS_LEVEL: i32 = 2400;
const BABY_SCALE: f32 = 0.65;
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.9, 0.6);
pub const BABY_DIMENSIONS: EntityDimensions =
    EntityDimensions::scalable(0.9 * BABY_SCALE, 0.6 * BABY_SCALE).with_eye_height(0.09375).with_passenger_attachments(&[Vec3::new(0.0, 0.3125, 0.0)]);
const MOVE_CONTROL: SmoothSwimmingMoveControl = SmoothSwimmingMoveControl::new(85, 10, 0.02, 0.1, true);

#[derive(Default)]
pub struct Dolphin {
    pub ageable_water_creature: AgeableWaterCreature,
    treasure_pos: Option<BlockPos>,
}

fn dolphin(mob: &Mob) -> &Dolphin {
    match &mob.kind {
        MobKind::Dolphin(dolphin) => dolphin,
        _ => unreachable!("not a dolphin"),
    }
}

fn dolphin_mut(mob: &mut Mob) -> &mut Dolphin {
    match &mut mob.kind {
        MobKind::Dolphin(dolphin) => dolphin,
        _ => unreachable!("not a dolphin"),
    }
}

pub fn got_fish(entity: &Entity) -> bool {
    matches!(entity.data.get(DATA_GOT_FISH), DataValue::Boolean(true))
}

pub fn set_got_fish(entity: &mut Entity, got_fish: bool) {
    entity.data.set(DATA_GOT_FISH, DataValue::Boolean(got_fish));
}

pub fn get_moistness_level(entity: &Entity) -> i32 {
    match entity.data.get(DATA_MOISTNESS_LEVEL) {
        DataValue::Int(level) => *level,
        _ => TOTAL_MOISTNESS_LEVEL,
    }
}

pub fn set_moisntess_level(entity: &mut Entity, level: i32) {
    entity.data.set(DATA_MOISTNESS_LEVEL, DataValue::Int(level));
}

pub fn get_hurt_sound() -> &'static str {
    "minecraft:entity.dolphin.hurt"
}

pub fn get_death_sound() -> &'static str {
    "minecraft:entity.dolphin.death"
}

pub fn get_ambient_sound(entity: &Entity) -> &'static str {
    if entity.is_in_water() { "minecraft:entity.dolphin.ambient_water" } else { "minecraft:entity.dolphin.ambient" }
}

pub fn get_swim_splash_sound() -> &'static str {
    "minecraft:entity.dolphin.splash"
}

pub fn get_swim_sound() -> &'static str {
    "minecraft:entity.dolphin.swim"
}

pub fn get_max_head_x_rot() -> i32 {
    1
}

pub fn get_max_head_y_rot() -> i32 {
    1
}

impl Dolphin {
    pub fn create() -> Mob {
        let mut data = ageable_mob_defaults();
        data.extend([DataValue::Boolean(false), DataValue::Int(TOTAL_MOISTNESS_LEVEL)]);
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:dolphin"), DIMENSIONS, data);
        let mut mob = Mob::new(entity, LivingEntity::new(&DOLPHIN_ATTRIBUTES), MobKind::Dolphin(Dolphin::default()));
        AgeableWaterCreature::init(&mut mob);
        mob.ai.move_control.kind = MoveControlKind::SmoothSwimming(MOVE_CONTROL);
        mob.ai.look_control.kind = LookControlKind::SmoothSwimming(10);
        mob.can_pick_up_loot = true;
        mob
    }

    pub fn register_goals(ai: &mut MobAi) {
        let goal_selector = &mut ai.goal_selector;
        goal_selector.add_goal(0, Goal::BreathAir(BreathAirGoal));
        goal_selector.add_goal(0, Goal::TryFindLiquid(TryFindLiquidGoal::new(&fluid_tags::DOLPHIN_TRIES_TO_FIND)));
        goal_selector.add_goal(1, Goal::DolphinSwimToTreasure(DolphinSwimToTreasureGoal::default()));
        goal_selector.add_goal(2, Goal::DolphinSwimWithPlayer(DolphinSwimWithPlayerGoal::new(4.0)));
        goal_selector.add_goal(4, Goal::RandomSwimming(RandomSwimmingGoal::new(1.0, 10)));
        goal_selector.add_goal(4, Goal::RandomLookAround(RandomLookAroundGoal::default()));
        goal_selector.add_goal(5, Goal::LookAtPlayer(LookAtPlayerGoal::new(6.0)));
        goal_selector.add_goal(5, Goal::DolphinJump(DolphinJumpGoal::new(10)));
        goal_selector.add_goal(6, Goal::MeleeAttack(MeleeAttackGoal::new(1.2f32 as f64, true)));
        goal_selector.add_goal(7, Goal::DolphinMoveToItem(MoveToItemGoal::default()));
        goal_selector.add_goal(8, Goal::DolphinPlayWithItems(PlayWithItemsGoal));
        goal_selector.add_goal(8, Goal::FollowPlayerRiddenEntity(FollowPlayerRiddenEntityGoal::new(EntityTypeToFollow::AbstractBoat)));
        goal_selector.add_goal(8, Goal::FollowPlayerRiddenEntity(FollowPlayerRiddenEntityGoal::new(EntityTypeToFollow::AbstractNautilus)));
        goal_selector.add_goal(9, Goal::AvoidEntity(AvoidEntityGoal::of_class(AvoidClass::Guardian, 8.0, 1.0, 1.0)));
        ai.target_selector.add_goal(1, Goal::HurtByTarget(HurtByTargetGoal::new(&["minecraft:guardian", "minecraft:elder_guardian"]).set_alert_others(&[])));
    }

    pub fn add_additional_save_data(mob: &Mob, output: &mut Vec<(String, Nbt)>) {
        output.push(("GotFish".into(), Nbt::Byte(got_fish(&mob.entity) as i8)));
        output.push(("Moistness".into(), Nbt::Int(get_moistness_level(&mob.entity))));
    }

    pub fn read_additional_save_data(mob: &mut Mob, input: &Nbt) {
        let got_fish = tag_value_input::get_i64(input, "GotFish").is_some_and(|v| v != 0);
        set_got_fish(&mut mob.entity, got_fish);
        let moistness = tag_value_input::get_i64(input, "Moistness").map_or(TOTAL_MOISTNESS_LEVEL, |v| v as i32);
        set_moisntess_level(&mut mob.entity, moistness);
    }
}

pub fn finalize_spawn(mob: &mut Mob) {
    mob.entity.set_air_supply(TOTAL_AIR_SUPPLY);
    mob.entity.set_x_rot(0.0);
}

impl ServerLevel {
    pub fn is_dolphin(&self, id: EntityId) -> bool {
        self.mob(id).is_some_and(|mob| matches!(mob.kind, MobKind::Dolphin(_)))
    }

    pub fn dolphin_get_breed_offspring(&mut self, _id: EntityId, _partner: EntityId) -> Option<Box<Mob>> {
        Some(Box::new(Dolphin::create()))
    }

    pub fn dolphin_play_attack_sound(&mut self, id: EntityId) {
        self.entity_play_sound(id, sound_event("minecraft:entity.dolphin.attack"), 1.0, 1.0);
    }

    pub fn dolphin_can_attack(&self, id: EntityId, target: EntityId) -> bool {
        !self.is_baby(id) && self.mob_can_attack(id, target)
    }

    pub fn dolphin_pick_up_item(&mut self, id: EntityId, entity: EntityId) {
        if self.item_by_slot(id, EquipmentSlot::Mainhand).is_some() {
            return;
        }
        let Some(LevelEntity::Item(item)) = self.entities.get(&entity) else { return };
        let item_stack = item.item.clone();
        if self.can_hold_item(id, &item_stack) {
            self.on_item_pickup(id, entity);
            let count = item_stack.count;
            self.set_item_slot(id, EquipmentSlot::Mainhand, item_stack);
            self.set_guaranteed_drop(id, EquipmentSlot::Mainhand);
            self.take(id, entity, count);
            self.discard(entity);
        }
    }

    pub fn dolphin_tick(&mut self, id: EntityId) {
        self.mob_tick(id);
        if self.entity_ref(id).is_none_or(|e| e.is_removed()) {
            return;
        }
        if self.mob(id).expect("a dolphin").is_no_ai() {
            self.entity_mut(id).set_air_supply(TOTAL_AIR_SUPPLY);
            return;
        }
        if self.is_in_water_or_rain(id) {
            set_moisntess_level(self.entity_mut(id), TOTAL_MOISTNESS_LEVEL);
        } else {
            let moistness = get_moistness_level(self.entity(id)) - 1;
            set_moisntess_level(self.entity_mut(id), moistness);
            if get_moistness_level(self.entity(id)) <= 0 {
                self.hurt(id, DamageSource::new(damage_types::DRY_OUT), 1.0);
            }
            if self.entity(id).on_ground {
                let e = self.entity_mut(id);
                let dx = ((e.random.next_float() * 2.0 - 1.0) * 0.2) as f64;
                let dz = ((e.random.next_float() * 2.0 - 1.0) * 0.2) as f64;
                e.set_delta_movement(e.delta_movement.add(dx, 0.5, dz));
                let y_rot = e.random.next_float() * 360.0;
                e.set_y_rot(y_rot);
                self.set_on_ground(id, false);
                self.entity_mut(id).needs_sync = true;
            }
        }
    }

    pub fn dolphin_mob_interact(&mut self, id: EntityId, player: EntityId, hand: InteractionHand) -> InteractionResult {
        let mut item_stack = self.player(player).item_in_hand(hand).unwrap_or_else(ItemStack::empty);
        if !item_stack.is_empty() && item_stack.get_item().is(&item_tags::FISHES) {
            self.entity_play_sound(id, sound_event("minecraft:entity.dolphin.eat"), 1.0, 1.0);
            if self.can_age_up(id) {
                item_stack.consume(1, self, Some(player));
                self.player_mut(player).update_item_in_hand(hand, &item_stack);
                let age = self.mob(id).and_then(Mob::ageable).expect("a dolphin").age;
                self.age_up(id, get_speed_up_seconds_when_feeding(-age), true);
            } else {
                set_got_fish(self.entity_mut(id), true);
                item_stack.consume(1, self, Some(player));
                self.player_mut(player).update_item_in_hand(hand, &item_stack);
            }
            return InteractionResult::SUCCESS;
        }
        self.ageable_mob_mob_interact(id, player, hand)
    }

    fn dolphin_close_to_next_pos(&self, id: EntityId) -> bool {
        let target = self.mob(id).expect("a dolphin").ai.navigation.get_target_pos();
        target.is_some_and(|target| target.closer_to_center_than(self.entity(id).position(), 12.0))
    }

    pub fn dolphin_travel_in_water(&mut self, id: EntityId, input: Vec3) {
        let speed = self.living(id).speed;
        self.entity_mut(id).move_relative(speed, input);
        let movement = self.entity(id).delta_movement;
        self.move_entity(id, MoverType::SelfMove, movement);
        let e = self.entity_mut(id);
        e.set_delta_movement(e.delta_movement.scale(0.9));
        if self.get_target(id).is_none() {
            let e = self.entity_mut(id);
            e.set_delta_movement(e.delta_movement.add(0.0, -0.005, 0.0));
        }
    }
}

#[derive(Default)]
pub struct DolphinSwimToTreasureGoal {
    stuck: bool,
}

impl DolphinSwimToTreasureGoal {
    pub fn can_use(&self, level: &ServerLevel, dolphin: EntityId) -> bool {
        got_fish(level.entity(dolphin)) && level.entity(dolphin).air_supply() >= 100
    }

    pub fn can_continue_to_use(&self, level: &ServerLevel, id: EntityId) -> bool {
        let Some(treasure_pos) = dolphin(level.mob(id).expect("a dolphin")).treasure_pos else { return false };
        let e = level.entity(id);
        !BlockPos::containing(treasure_pos.x as f64, e.y(), treasure_pos.z as f64).closer_to_center_than(e.position(), 4.0) && !self.stuck && e.air_supply() >= 100
    }

    pub fn start(&mut self, level: &mut ServerLevel, id: EntityId) {
        self.stuck = false;
        level.get_navigation(id, |navigation, _| navigation.stop());
        let dolphin_pos = level.entity(id).block_position();
        let dolphin_located: Vec<StructureHolder> = crate::commands::arguments::registry_tag("minecraft:worldgen/structure", "minecraft:dolphin_located")
            .map(|tag| tag.iter().map(|&structure| StructureHolder(structure as u16)).collect())
            .unwrap_or_default();
        match level.find_nearest_map_structure(&dolphin_located, dolphin_pos, 50, false) {
            Some(treasure_pos) => {
                dolphin_mut(level.mob_mut(id).expect("a dolphin")).treasure_pos = Some(treasure_pos);
                level.broadcast_entity_event(id, 38);
            }
            None => self.stuck = true,
        }
    }

    pub fn stop(&mut self, level: &mut ServerLevel, id: EntityId) {
        let treasure_pos = dolphin(level.mob(id).expect("a dolphin")).treasure_pos;
        let e = level.entity(id);
        if treasure_pos.is_none_or(|treasure_pos| BlockPos::containing(treasure_pos.x as f64, e.y(), treasure_pos.z as f64).closer_to_center_than(e.position(), 4.0)) || self.stuck {
            set_got_fish(level.entity_mut(id), false);
        }
    }

    pub fn tick(&mut self, level: &mut ServerLevel, id: EntityId) {
        let Some(treasure_pos) = dolphin(level.mob(id).expect("a dolphin")).treasure_pos else { return };
        if !(level.dolphin_close_to_next_pos(id) || level.mob(id).expect("a dolphin").ai.navigation.is_done()) {
            return;
        }
        let treasure_pos = Vec3::at_center_of(treasure_pos);
        let mut next_pos = default_random_pos::get_pos_towards(level, id, 16, 1, treasure_pos, (std::f32::consts::PI / 8.0) as f64);
        if next_pos.is_none() {
            next_pos = default_random_pos::get_pos_towards(level, id, 8, 4, treasure_pos, (std::f32::consts::PI / 2.0) as f64);
        }
        if let Some(pos) = next_pos {
            let next = BlockPos::containing_vec(pos);
            if !level.fluid_state(next).is(&fluid_tags::WATER) || !level.block_state(next).is_pathfindable_water() {
                next_pos = default_random_pos::get_pos_towards(level, id, 8, 5, treasure_pos, (std::f32::consts::PI / 2.0) as f64);
            }
        }
        let Some(next_pos) = next_pos else {
            self.stuck = true;
            return;
        };
        let mob = level.mob_mut(id).expect("a dolphin");
        let (y_max, x_max) = ((get_max_head_y_rot() + 20) as f32, get_max_head_x_rot() as f32);
        mob.ai.look_control.set_look_at(next_pos.x, next_pos.y, next_pos.z, y_max, x_max);
        level.get_navigation(id, |navigation, level| navigation.move_to(level, id, next_pos.x, next_pos.y, next_pos.z, 1.3));
        if level.random.next_int_bounded(reduced_tick_delay(80)) == 0 {
            level.broadcast_entity_event(id, 38);
        }
    }
}

pub struct DolphinSwimWithPlayerGoal {
    speed_modifier: f64,
    player: Option<EntityId>,
}

impl DolphinSwimWithPlayerGoal {
    pub fn new(speed_modifier: f64) -> DolphinSwimWithPlayerGoal {
        DolphinSwimWithPlayerGoal { speed_modifier, player: None }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, dolphin: EntityId) -> bool {
        let e = level.entity(dolphin);
        let (x, y, z) = (e.x(), e.y(), e.z());
        self.player = level.get_nearest_player(&SWIM_WITH_PLAYER_TARGETING, Some(dolphin), x, y, z);
        match self.player {
            Some(player) => level.entity(player).is_swimming() && level.get_target(dolphin) != Some(player),
            None => false,
        }
    }

    pub fn can_continue_to_use(&self, level: &ServerLevel, dolphin: EntityId) -> bool {
        self.player.is_some_and(|player| level.entity(player).is_swimming() && level.entity(dolphin).distance_to_sqr(level.entity(player)) < 256.0)
    }

    pub fn start(&mut self, level: &mut ServerLevel, dolphin: EntityId) {
        let player = self.player.expect("a player");
        level.add_effect(player, MobEffectInstance::new(mob_effects::DOLPHINS_GRACE, 100, 0), Some(dolphin));
    }

    pub fn stop(&mut self, level: &mut ServerLevel, dolphin: EntityId) {
        self.player = None;
        level.get_navigation(dolphin, |navigation, _| navigation.stop());
    }

    pub fn tick(&mut self, level: &mut ServerLevel, dolphin: EntityId) {
        let player = self.player.expect("a player");
        let (y_max, x_max) = ((get_max_head_y_rot() + 20) as f32, get_max_head_x_rot() as f32);
        level.set_look_at_entity(dolphin, player, y_max, x_max);
        if level.entity(dolphin).distance_to_sqr(level.entity(player)) < 6.25 {
            level.get_navigation(dolphin, |navigation, _| navigation.stop());
        } else {
            let speed_modifier = self.speed_modifier;
            level.get_navigation(dolphin, |navigation, level| navigation.move_to_entity(level, dolphin, player, speed_modifier));
        }
        if level.entity(player).is_swimming() && level.random.next_int_bounded(6) == 0 {
            level.add_effect(player, MobEffectInstance::new(mob_effects::DOLPHINS_GRACE, 100, 0), Some(dolphin));
        }
    }
}

fn allowed_items(level: &ServerLevel, entity: EntityId) -> bool {
    match level.entities.get(&entity) {
        Some(LevelEntity::Item(item)) => item.pickup_delay <= 0 && level.is_alive(entity) && item.entity.is_in_water(),
        _ => false,
    }
}

fn items_nearby(level: &ServerLevel, id: EntityId) -> Vec<EntityId> {
    let area = level.entity(id).bounding_box().inflate(8.0, 8.0, 8.0);
    level.items_in(&area, None).into_iter().filter(|&item| allowed_items(level, item)).collect()
}

fn drop(level: &mut ServerLevel, id: EntityId, item_stack: ItemStack) {
    if item_stack.is_empty() {
        return;
    }
    let e = level.entity(id);
    let y_hand_pos = e.eye_y() - 0.3f32 as f64;
    let (x, z, y_rot, x_rot, uuid) = (e.x(), e.z(), e.y_rot, e.x_rot, e.uuid);
    let mut thrown_item = ItemEntity::new(Vec3::new(x, y_hand_pos, z), item_stack);
    thrown_item.pickup_delay = 40;
    thrown_item.thrower = Some(uuid);
    let pow = 0.3f32;
    let random = &mut level.entity_mut(id).random;
    let dir = random.next_float() * (std::f32::consts::PI * 2.0);
    let pow2 = 0.02 * random.next_float();
    let y_rad = y_rot * mth::DEG_TO_RAD;
    let x_rad = x_rot * mth::DEG_TO_RAD;
    thrown_item.entity.set_delta_movement(Vec3::new(
        (pow * -mth::sin(y_rad as f64) * mth::cos(x_rad as f64) + mth::cos(dir as f64) * pow2) as f64,
        (pow * mth::sin(x_rad as f64) * 1.5) as f64,
        (pow * mth::cos(y_rad as f64) * mth::cos(x_rad as f64) + mth::sin(dir as f64) * pow2) as f64,
    ));
    level.add_fresh_entity(LevelEntity::Item(thrown_item));
}

fn drop_item(level: &mut ServerLevel, id: EntityId) -> bool {
    let Some(item_stack) = level.item_by_slot(id, EquipmentSlot::Mainhand) else { return false };
    drop(level, id, item_stack);
    level.set_item_slot(id, EquipmentSlot::Mainhand, ItemStack::empty());
    true
}

#[derive(Default)]
pub struct MoveToItemGoal {
    cooldown: i32,
}

impl MoveToItemGoal {
    pub fn can_use(&self, level: &ServerLevel, id: EntityId) -> bool {
        if self.cooldown > level.entity(id).tick_count {
            return false;
        }
        !items_nearby(level, id).is_empty()
    }

    pub fn stop(&mut self, level: &mut ServerLevel, id: EntityId) {
        if drop_item(level, id) {
            let tick_count = level.entity(id).tick_count;
            self.cooldown = tick_count + level.entity_mut(id).random.next_int_bounded(100);
        }
    }

    pub fn start(&mut self, level: &mut ServerLevel, id: EntityId) {
        if let Some(&item) = items_nearby(level, id).first() {
            level.get_navigation(id, |navigation, level| navigation.move_to_entity(level, id, item, 1.2f32 as f64));
            level.entity_play_sound(id, sound_event("minecraft:entity.dolphin.play"), 1.0, 1.0);
        }
        self.cooldown = 0;
    }

    pub fn tick(&mut self, level: &mut ServerLevel, id: EntityId) {
        if let Some(&item) = items_nearby(level, id).first() {
            level.get_navigation(id, |navigation, level| navigation.move_to_entity(level, id, item, 1.2f32 as f64));
        }
    }
}

pub struct PlayWithItemsGoal;

impl PlayWithItemsGoal {
    pub fn can_use(&self, level: &ServerLevel, id: EntityId) -> bool {
        level.item_by_slot_ref(id, EquipmentSlot::Mainhand).is_some()
    }

    pub fn stop(&mut self, level: &mut ServerLevel, id: EntityId) {
        drop_item(level, id);
    }

    pub fn tick(&mut self, level: &mut ServerLevel, id: EntityId) {
        drop_item(level, id);
    }
}


