use std::sync::OnceLock;

use wafer_core::random::RandomSource;
use wafer_core::mth;
use wafer_data::{EquipmentSlot, damage_type_tags, item_tags, items, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use super::wolf_sound_variant::{self, WolfSoundSet};
use crate::core::particles::particle_options::ParticleOptions;
use crate::server::level::ServerLevel;
use crate::util::time_util;
use crate::util::valueproviders::IntProvider;
use crate::world::damagesource::DamageSource;
use crate::world::entity::ageable_mob::AgeableMobGroupData;
use crate::world::entity::ai::MobAi;
use crate::world::entity::ai::goal::avoid_entity_goal::AvoidEntityGoal;
use crate::world::entity::ai::goal::beg_goal::BegGoal;
use crate::world::entity::ai::goal::breed_goal::BreedGoal;
use crate::world::entity::ai::goal::float_goal::FloatGoal;
use crate::world::entity::ai::goal::follow_owner_goal::FollowOwnerGoal;
use crate::world::entity::ai::goal::goal::Goal;
use crate::world::entity::ai::goal::leap_at_target_goal::LeapAtTargetGoal;
use crate::world::entity::ai::goal::look_at_player_goal::LookAtPlayerGoal;
use crate::world::entity::ai::goal::melee_attack_goal::MeleeAttackGoal;
use crate::world::entity::ai::goal::random_look_around_goal::RandomLookAroundGoal;
use crate::world::entity::ai::goal::sit_when_ordered_to_goal::SitWhenOrderedToGoal;
use crate::world::entity::ai::goal::target::hurt_by_target_goal::HurtByTargetGoal;
use crate::world::entity::ai::goal::target::nearest_attackable_target_goal::{NearestAttackableTargetGoal, TargetType};
use crate::world::entity::ai::goal::target::non_tame_random_target_goal::NonTameRandomTargetGoal;
use crate::world::entity::ai::goal::target::owner_hurt_by_target_goal::OwnerHurtByTargetGoal;
use crate::world::entity::ai::goal::target::owner_hurt_target_goal::OwnerHurtTargetGoal;
use crate::world::entity::ai::goal::target::reset_universal_anger_target_goal::ResetUniversalAngerTargetGoal;
use crate::world::entity::ai::goal::water_avoiding_random_stroll_goal::WaterAvoidingRandomStrollGoal;
use crate::world::entity::animal::Animal;
use crate::world::entity::attributes::{Attribute, WOLF_ATTRIBUTES};
use crate::world::entity::crackiness;
use crate::world::entity::data::DataValue;
use crate::world::entity::entity_reference::EntityReference;
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::monster::zombie::zombie;
use crate::world::entity::neutral_mob::{self, NeutralMob};
use crate::world::entity::spawn_group_data::SpawnGroupData;
use crate::world::entity::tamable_animal::{self, TamableAnimal, TamableAnimalPanicGoal, tamable_animal_defaults};
use crate::world::entity::variant::variant_utils::{self, SpawnPrioritySelectors};
use crate::world::entity::{Entity, EntityDimensions, EntityId, sound_event};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::item::dye_color::DyeColor;
use crate::world::level::server_level_accessor::ServerLevelAccessor;
use crate::world::level::pathfinder::path_type::PathType;
use crate::world::{InteractionHand, InteractionResult};
use crate::world::level::storage::tag_value_input;

pub const DATA_INTERESTED_ID: usize = 20;
pub const DATA_COLLAR_COLOR: usize = 21;
pub const DATA_ANGER_END_TIME: usize = 22;
pub const DATA_VARIANT_ID: usize = 23;
pub const DATA_SOUND_VARIANT_ID: usize = 24;
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.6, 0.85).with_eye_height(0.68).with_passenger_attachments(&[wafer_core::Vec3::new(0.0, 0.81875, -0.0625)]);
pub const BABY_DIMENSIONS: EntityDimensions =
    EntityDimensions::scalable(0.3, 0.425).with_eye_height(0.34375).with_passenger_attachments(&[wafer_core::Vec3::new(0.0, 0.4375f32 as f64, 0.0)]);
pub const VARIANT_REGISTRY: &str = "minecraft:wolf_variant";
const START_HEALTH: f64 = 8.0;
const TAME_HEALTH: f32 = 40.0;
const ARMOR_REPAIR_UNIT: f32 = 0.125;
const DEFAULT_COLLAR_COLOR: DyeColor = DyeColor::VALUES[14];
const PERSISTENT_ANGER_TIME: IntProvider = time_util::range_of_seconds(20, 39);

#[derive(Default)]
pub struct Wolf {
    pub tamable_animal: TamableAnimal,
    interested_angle: f32,
    interested_angle_o: f32,
    is_wet: bool,
    is_shaking: bool,
    shake_anim: f32,
    shake_anim_o: f32,
    persistent_anger_target: Option<EntityReference>,
}

impl NeutralMob for Wolf {
    fn get_persistent_anger_end_time(&self, entity: &Entity) -> i64 {
        match entity.data.get(DATA_ANGER_END_TIME) {
            DataValue::Long(end_time) => *end_time,
            _ => neutral_mob::NO_ANGER_END_TIME,
        }
    }

    fn set_persistent_anger_end_time(&mut self, entity: &mut Entity, end_time: i64) {
        entity.data.set(DATA_ANGER_END_TIME, DataValue::Long(end_time));
    }

    fn get_persistent_anger_target(&self) -> Option<EntityReference> {
        self.persistent_anger_target
    }

    fn set_persistent_anger_target(&mut self, persistent_anger_target: Option<EntityReference>) {
        self.persistent_anger_target = persistent_anger_target;
    }
}

pub fn wolf(mob: &Mob) -> &Wolf {
    match &mob.kind {
        MobKind::Wolf(wolf) => wolf,
        _ => unreachable!("not a wolf"),
    }
}

pub fn wolf_mut(mob: &mut Mob) -> &mut Wolf {
    match &mut mob.kind {
        MobKind::Wolf(wolf) => wolf,
        _ => unreachable!("not a wolf"),
    }
}

fn spawn_selectors() -> &'static SpawnPrioritySelectors {
    static SELECTORS: OnceLock<SpawnPrioritySelectors> = OnceLock::new();
    SELECTORS.get_or_init(|| SpawnPrioritySelectors::load(VARIANT_REGISTRY, crate::core::data_registries::json_entries("minecraft:wolf_variant")))
}

pub fn prey_selector(_mob: EntityId, target: EntityId, level: &ServerLevel) -> bool {
    matches!(registry::ENTITY_TYPE[level.entity(target).entity_type as usize], "minecraft:sheep" | "minecraft:rabbit" | "minecraft:fox")
}

pub fn is_food(item_stack: &ItemStack) -> bool {
    item_stack.get_item().is(&item_tags::WOLF_FOOD)
}

pub fn get_variant(entity: &Entity) -> i32 {
    match entity.data.get(DATA_VARIANT_ID) {
        DataValue::WolfVariant(variant) => *variant,
        _ => 0,
    }
}

pub fn set_variant(entity: &mut Entity, variant: i32) {
    entity.data.set(DATA_VARIANT_ID, DataValue::WolfVariant(variant));
}

pub fn get_sound_variant(entity: &Entity) -> i32 {
    match entity.data.get(DATA_SOUND_VARIANT_ID) {
        DataValue::WolfSoundVariant(variant) => *variant,
        _ => 0,
    }
}

pub fn set_sound_variant(entity: &mut Entity, sound_variant: i32) {
    entity.data.set(DATA_SOUND_VARIANT_ID, DataValue::WolfSoundVariant(sound_variant));
}

pub fn get_collar_color(entity: &Entity) -> DyeColor {
    match entity.data.get(DATA_COLLAR_COLOR) {
        DataValue::Int(id) => DyeColor::by_id(*id),
        _ => DEFAULT_COLLAR_COLOR,
    }
}

pub fn set_collar_color(entity: &mut Entity, color: DyeColor) {
    entity.data.set(DATA_COLLAR_COLOR, DataValue::Int(color.id));
}

pub fn is_interested(entity: &Entity) -> bool {
    matches!(entity.data.get(DATA_INTERESTED_ID), DataValue::Boolean(true))
}

pub fn apply_taming_side_effects(mob: &mut Mob) {
    let tame = tamable_animal::is_tame(&mob.entity);
    let max_health = mob.living.attributes.instance(Attribute::MaxHealth).expect("wolves have a max health");
    if tame {
        max_health.set_base_value(TAME_HEALTH as f64);
        mob.set_health(TAME_HEALTH);
    } else {
        max_health.set_base_value(START_HEALTH);
    }
}

pub fn get_max_head_x_rot(mob: &Mob) -> i32 {
    if tamable_animal::is_in_sitting_pose(&mob.entity) { 20 } else { 40 }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct WolfPackData {
    pub ageable_mob_group_data: AgeableMobGroupData,
    pub type_: i32,
}

impl WolfPackData {
    pub fn new(type_: i32) -> WolfPackData {
        WolfPackData { ageable_mob_group_data: AgeableMobGroupData::new(false), type_ }
    }
}

pub fn finalize_spawn(level: &mut (impl ServerLevelAccessor + ?Sized), mob: &mut Mob, group_data: Option<SpawnGroupData>) -> Option<SpawnGroupData> {
    let mut group_data = group_data;
    if let Some(SpawnGroupData::WolfPack(wolf_group_data)) = group_data {
        set_variant(&mut mob.entity, wolf_group_data.type_);
    } else if let Some(selected_variant) = variant_utils::select_variant_to_spawn(level, mob.entity.block_position(), spawn_selectors()) {
        set_variant(&mut mob.entity, selected_variant);
        group_data = Some(SpawnGroupData::WolfPack(WolfPackData::new(selected_variant)));
    }
    let sound_variant = variant_utils::pick_random_sound_variant(wolf_sound_variant::REGISTRY, level.get_random());
    set_sound_variant(&mut mob.entity, sound_variant);
    group_data
}

impl Wolf {
    pub fn create() -> Mob {
        let mut data = tamable_animal_defaults();
        let pale = variant_utils::read_variant(VARIANT_REGISTRY, "minecraft:pale").unwrap_or(0);
        let classic = variant_utils::read_variant(wolf_sound_variant::REGISTRY, "minecraft:classic").unwrap_or(0);
        data.extend([
            DataValue::Boolean(false),
            DataValue::Int(DEFAULT_COLLAR_COLOR.id),
            DataValue::Long(neutral_mob::NO_ANGER_END_TIME),
            DataValue::WolfVariant(pale),
            DataValue::WolfSoundVariant(classic),
        ]);
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:wolf"), DIMENSIONS, data);
        let mut mob = Mob::new(entity, LivingEntity::new(&WOLF_ATTRIBUTES), MobKind::Wolf(Wolf::default()));
        Animal::init(&mut mob);
        tamable_animal::set_tame(&mut mob, false, false);
        mob.set_pathfinding_malus(PathType::PowderSnow, -1.0);
        mob.set_pathfinding_malus(PathType::OnTopOfPowderSnow, -1.0);
        mob
    }

    pub fn register_goals(ai: &mut MobAi, follow_distance: f64) {
        let float_goal = FloatGoal::new(&mut ai.navigation);
        let goal_selector = &mut ai.goal_selector;
        goal_selector.add_goal(1, Goal::Float(float_goal));
        goal_selector.add_goal(1, Goal::TamableAnimalPanic(TamableAnimalPanicGoal::new(1.5, |_, _| &damage_type_tags::PANIC_ENVIRONMENTAL_CAUSES)));
        goal_selector.add_goal(2, Goal::SitWhenOrderedTo(SitWhenOrderedToGoal));
        goal_selector.add_goal(3, Goal::WolfAvoidEntity(WolfAvoidEntityGoal::new("minecraft:llama", 24.0, 1.5, 1.5)));
        goal_selector.add_goal(4, Goal::LeapAtTarget(LeapAtTargetGoal::new(0.4)));
        goal_selector.add_goal(5, Goal::MeleeAttack(MeleeAttackGoal::new(1.0, true)));
        goal_selector.add_goal(6, Goal::FollowOwner(FollowOwnerGoal::new(1.0, 10.0, 2.0)));
        goal_selector.add_goal(7, Goal::Breed(BreedGoal::new(1.0)));
        goal_selector.add_goal(8, Goal::WaterAvoidingRandomStroll(WaterAvoidingRandomStrollGoal::new(1.0)));
        goal_selector.add_goal(9, Goal::Beg(BegGoal::new(8.0)));
        goal_selector.add_goal(10, Goal::LookAtPlayer(LookAtPlayerGoal::new(8.0)));
        goal_selector.add_goal(10, Goal::RandomLookAround(RandomLookAroundGoal::default()));
        let target_selector = &mut ai.target_selector;
        target_selector.add_goal(1, Goal::OwnerHurtByTarget(OwnerHurtByTargetGoal::new()));
        target_selector.add_goal(2, Goal::OwnerHurtTarget(OwnerHurtTargetGoal::new()));
        target_selector.add_goal(3, Goal::HurtByTarget(HurtByTargetGoal::new(&[]).set_alert_others(&[])));
        target_selector.add_goal(4, Goal::NearestAttackableTarget(NearestAttackableTargetGoal::with(TargetType::Player, 10, true, false, Some(|mob, target, level| level.is_angry_at(mob, target)))));
        target_selector.add_goal(5, Goal::NonTameRandomTarget(NonTameRandomTargetGoal::new(TargetType::Animal, false, Some(prey_selector), follow_distance)));
        target_selector.add_goal(6, Goal::NonTameRandomTarget(NonTameRandomTargetGoal::new(TargetType::Turtle, false, Some(zombie::baby_on_land_selector), follow_distance)));
        target_selector.add_goal(7, Goal::NearestAttackableTarget(NearestAttackableTargetGoal::new(TargetType::AbstractSkeleton, false)));
        target_selector.add_goal(8, Goal::ResetUniversalAngerTarget(ResetUniversalAngerTargetGoal::new(true)));
    }

    pub fn get_sound_set(mob: &Mob) -> &'static WolfSoundSet {
        let sound_variant = wolf_sound_variant::sound_variant(get_sound_variant(&mob.entity));
        if mob.is_baby() { &sound_variant.baby_sounds } else { &sound_variant.adult_sounds }
    }

    pub fn get_hurt_sound(mob: &Mob) -> &'static str {
        &Wolf::get_sound_set(mob).hurt_sound
    }

    pub fn get_death_sound(mob: &Mob) -> &'static str {
        &Wolf::get_sound_set(mob).death_sound
    }

    pub fn get_step_sound(mob: &Mob) -> &'static str {
        &Wolf::get_sound_set(mob).step_sound
    }

    pub fn add_additional_save_data(mob: &Mob, output: &mut Vec<(String, Nbt)>) {
        TamableAnimal::add_additional_save_data(mob, output);
        output.push(("CollarColor".into(), Nbt::Byte(get_collar_color(&mob.entity).id as i8)));
        output.push(("variant".into(), Nbt::String(variant_utils::variant_name(VARIANT_REGISTRY, get_variant(&mob.entity)).into())));
        wolf(mob).add_persistent_anger_save_data(&mob.entity, output);
        output.push(("sound_variant".into(), Nbt::String(variant_utils::variant_name(wolf_sound_variant::REGISTRY, get_sound_variant(&mob.entity)).into())));
    }

    pub fn read_additional_save_data(mob: &mut Mob, input: &Nbt) {
        TamableAnimal::read_additional_save_data(mob, input);
        let read = |key: &str, registry: &str| variant_utils::read_variant_field(input, key, registry);
        if let Some(variant) = read("variant", VARIANT_REGISTRY) {
            set_variant(&mut mob.entity, variant);
        }
        let collar_color = tag_value_input::read_byte(input, "CollarColor").map_or(DEFAULT_COLLAR_COLOR, |id| DyeColor::by_id(id as i32));
        set_collar_color(&mut mob.entity, collar_color);
        if let Some(sound_variant) = read("sound_variant", wolf_sound_variant::REGISTRY) {
            set_sound_variant(&mut mob.entity, sound_variant);
        }
    }
}

pub struct WolfAvoidEntityGoal {
    pub avoid_entity_goal: AvoidEntityGoal,
}

impl WolfAvoidEntityGoal {
    pub fn new(avoid_class: &'static str, max_dist: f32, walk_speed_modifier: f64, sprint_speed_modifier: f64) -> WolfAvoidEntityGoal {
        WolfAvoidEntityGoal { avoid_entity_goal: AvoidEntityGoal::new(avoid_class, max_dist, walk_speed_modifier, sprint_speed_modifier) }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        if !self.avoid_entity_goal.can_use(level, mob) {
            return false;
        }
        let Some(llama) = self.avoid_entity_goal.to_avoid.filter(|&to_avoid| level.mob(to_avoid).and_then(Mob::llama_strength).is_some()) else { return false };
        !level.is_tame(mob) && self.avoid_llama(level, mob, llama)
    }

    fn avoid_llama(&self, level: &mut ServerLevel, mob: EntityId, llama: EntityId) -> bool {
        let strength = level.mob(llama).and_then(Mob::llama_strength).expect("a llama");
        strength >= level.entity_mut(mob).random.next_int_bounded(5)
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        level.set_target(mob, None);
        self.avoid_entity_goal.start(level, mob);
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        level.set_target(mob, None);
        self.avoid_entity_goal.tick(level, mob);
    }
}

impl ServerLevel {
    pub fn is_wolf(&self, id: EntityId) -> bool {
        self.mob(id).is_some_and(|mob| matches!(mob.kind, MobKind::Wolf(_)))
    }

    pub fn set_is_interested(&mut self, id: EntityId, value: bool) {
        self.entity_mut(id).data.set(DATA_INTERESTED_ID, DataValue::Boolean(value));
    }

    pub fn wolf_get_ambient_sound(&mut self, id: EntityId) -> &'static str {
        let sound_set = Wolf::get_sound_set(self.mob(id).expect("a wolf"));
        if self.is_angry(id) {
            &sound_set.growl_sound
        } else if self.entity_mut(id).random.next_int_bounded(3) == 0 {
            if self.is_tame(id) && self.health(id) < 20.0 { &sound_set.whine_sound } else { &sound_set.pant_sound }
        } else {
            &sound_set.ambient_sound
        }
    }

    pub fn wolf_get_hurt_sound(&self, id: EntityId, source: &DamageSource) -> &'static str {
        if self.can_armor_absorb(id, source) { "minecraft:item.wolf_armor.damage" } else { Wolf::get_hurt_sound(self.mob(id).expect("a wolf")) }
    }

    pub fn wolf_ai_step(&mut self, id: EntityId) {
        self.animal_ai_step(id);
        if self.mob(id).is_none() {
            return;
        }
        let path_finding = self.is_path_finding(id);
        let mob = self.mob_mut(id).expect("a wolf");
        let on_ground = mob.entity.on_ground;
        let wolf = wolf_mut(mob);
        if wolf.is_wet && !wolf.is_shaking && !path_finding && on_ground {
            wolf.is_shaking = true;
            wolf.shake_anim = 0.0;
            wolf.shake_anim_o = 0.0;
            self.broadcast_entity_event(id, crate::world::entity::entity_event::SHAKE_WETNESS);
        }
        self.update_persistent_anger(id, true);
    }

    pub fn wolf_tick(&mut self, id: EntityId) {
        self.mob_tick(id);
        self.then_tick(move |level| level.wolf_tick_end(id));
    }

    fn wolf_tick_end(&mut self, id: EntityId) {
        if !self.is_alive(id) {
            return;
        }
        let interested = is_interested(self.entity(id));
        let wolf = wolf_mut(self.mob_mut(id).expect("a wolf"));
        wolf.interested_angle_o = wolf.interested_angle;
        if interested {
            wolf.interested_angle += (1.0 - wolf.interested_angle) * 0.4;
        } else {
            wolf.interested_angle += (0.0 - wolf.interested_angle) * 0.4;
        }
        if self.is_in_water_or_rain(id) {
            let wolf = wolf_mut(self.mob_mut(id).expect("a wolf"));
            wolf.is_wet = true;
            if wolf.is_shaking {
                self.broadcast_entity_event(id, crate::world::entity::entity_event::CANCEL_SHAKE_WETNESS);
                self.cancel_shake(id);
            }
            return;
        }
        let shaking = self::wolf(self.mob(id).expect("a wolf"));
        if !((shaking.is_wet || shaking.is_shaking) && shaking.is_shaking) {
            return;
        }
        if shaking.shake_anim == 0.0 {
            let volume = self.mob(id).expect("a wolf").get_sound_volume();
            let random = &mut self.entity_mut(id).random;
            let pitch = (random.next_float() - random.next_float()) * 0.2 + 1.0;
            self.entity_play_sound(id, sound_event("minecraft:entity.wolf.shake"), volume, pitch);
            self.entity_game_event(id, crate::world::level::gameevent::game_event::ENTITY_ACTION);
        }
        let mob = self.mob_mut(id).expect("a wolf");
        let wolf = wolf_mut(mob);
        wolf.shake_anim_o = wolf.shake_anim;
        wolf.shake_anim += 0.05;
        if wolf.shake_anim_o >= 2.0 {
            wolf.is_wet = false;
            wolf.is_shaking = false;
            wolf.shake_anim_o = 0.0;
            wolf.shake_anim = 0.0;
        }
        if wolf.shake_anim > 0.4 {
            let shake_count = (mth::sin(((wolf.shake_anim - 0.4) * std::f32::consts::PI) as f64) * 7.0) as i32;
            let random = &mut mob.entity.random;
            for _ in 0..shake_count {
                random.next_float();
                random.next_float();
            }
        }
    }

    fn cancel_shake(&mut self, id: EntityId) {
        let wolf = wolf_mut(self.mob_mut(id).expect("a wolf"));
        wolf.is_shaking = false;
        wolf.shake_anim = 0.0;
        wolf.shake_anim_o = 0.0;
    }

    pub fn wolf_die(&mut self, id: EntityId, source: DamageSource) {
        let wolf = wolf_mut(self.mob_mut(id).expect("a wolf"));
        wolf.is_wet = false;
        wolf.is_shaking = false;
        wolf.shake_anim_o = 0.0;
        wolf.shake_anim = 0.0;
        self.tamable_animal_die(id, source);
    }

    pub fn wolf_hurt_server(&mut self, id: EntityId, source: DamageSource, damage: f32) -> bool {
        if self.is_invulnerable_to(id, &source) {
            return false;
        }
        self.set_ordered_to_sit(id, false);
        self.living_entity_hurt_server(id, source, damage)
    }

    pub fn wolf_actually_hurt(&mut self, id: EntityId, source: &DamageSource, damage: f32) {
        if !self.can_armor_absorb(id, source) {
            self.animal_actually_hurt(id, source, damage);
            return;
        }
        let armor_before = self.get_body_armor_item(id);
        let (damage_before, max_damage) = (armor_before.damage_value(), armor_before.max_damage());
        self.hurt_and_break(id, EquipmentSlot::Body, mth::ceil(damage as f64));
        if crackiness::WOLF_ARMOR.by_damage(damage_before, max_damage) != crackiness::WOLF_ARMOR.by_item(&self.get_body_armor_item(id)) {
            self.entity_play_sound(id, sound_event("minecraft:item.wolf_armor.crack"), 1.0, 1.0);
            let particle = ParticleOptions { particle_type: "minecraft:item", options: Nbt::Compound(vec![("item".into(), Nbt::Compound(vec![("id".into(), Nbt::String("minecraft:armadillo_scute".into()))].into()))].into()) };
            let e = self.entity(id);
            let (x, y, z) = (e.x(), e.y() + 1.0, e.z());
            self.send_particles("minecraft:item", |w| particle.write_options(w), x, y, z, 20, (0.2, 0.1, 0.2), 0.1);
        }
    }

    fn can_armor_absorb(&self, id: EntityId, source: &DamageSource) -> bool {
        self.get_body_armor_item(id).is(items::WOLF_ARMOR) && !source.is(&damage_type_tags::BYPASSES_WOLF_ARMOR)
    }

    pub fn wolf_hurt_armor(&mut self, id: EntityId, damage_source: &DamageSource, damage: f32) {
        self.do_hurt_equipment(id, damage_source, damage, &[EquipmentSlot::Body]);
    }

    pub fn wolf_can_shear_equipment(&self, id: EntityId, player: EntityId) -> bool {
        self.is_owned_by(id, player)
    }

    pub fn wolf_mob_interact(&mut self, id: EntityId, player: EntityId, hand: InteractionHand) -> InteractionResult {
        let mut item_stack = self.player(player).item_in_hand(hand).unwrap_or_else(ItemStack::empty);
        if self.is_tame(id) {
            if self.is_food(id, &item_stack) && self.health(id) < self.max_health(id) {
                self.tamable_animal_feed(id, player, hand, &mut item_stack, 2.0, 2.0);
                return InteractionResult::SUCCESS;
            }
            if !item_stack.get_item().is(&item_tags::WOLF_COLLAR_DYES) || !self.is_owned_by(id, player) {
                if self.is_equippable_in_slot(id, &item_stack, EquipmentSlot::Body) && !self.is_wearing_body_armor(id) && self.is_owned_by(id, player) && !self.is_baby(id) {
                    self.set_item_slot_and_drop_when_killed(id, EquipmentSlot::Body, item_stack.copy_with_count(1));
                    item_stack.consume(1, self, Some(player));
                    self.player_mut(player).update_item_in_hand(hand, &item_stack);
                    return InteractionResult::SUCCESS;
                }
                if self.is_in_sitting_pose(id)
                    && self.is_wearing_body_armor(id)
                    && self.is_owned_by(id, player)
                    && self.get_body_armor_item(id).is_damaged()
                    && self.get_body_armor_item(id).is_valid_repair_item(&item_stack)
                {
                    item_stack.shrink(1);
                    self.player_mut(player).update_item_in_hand(hand, &item_stack);
                    self.entity_play_sound(id, sound_event("minecraft:item.wolf_armor.repair"), 1.0, 1.0);
                    let mut armor = self.get_body_armor_item(id);
                    let repair_unit = (armor.max_damage() as f32 * ARMOR_REPAIR_UNIT) as i32;
                    armor.set_damage_value((armor.damage_value() - repair_unit).max(0));
                    self.set_item_slot(id, EquipmentSlot::Body, armor);
                    return InteractionResult::SUCCESS;
                }
                let interaction_result = self.animal_mob_interact(id, player, hand);
                if !interaction_result.consumes_action() && self.is_owned_by(id, player) {
                    let ordered_to_sit = !self.is_ordered_to_sit(id);
                    self.set_ordered_to_sit(id, ordered_to_sit);
                    self.living_mut(id).jumping = false;
                    self.get_navigation(id, |navigation, _| navigation.stop());
                    self.set_target(id, None);
                    return InteractionResult::SUCCESS.without_item();
                }
                return interaction_result;
            }
            let color = item_stack.get(dc::DYE).and_then(Nbt::as_str).and_then(DyeColor::by_name);
            if let Some(color) = color.filter(|&color| color != get_collar_color(self.entity(id))) {
                set_collar_color(self.entity_mut(id), color);
                item_stack.consume(1, self, Some(player));
                self.player_mut(player).update_item_in_hand(hand, &item_stack);
                return InteractionResult::SUCCESS;
            }
        } else if item_stack.is(items::BONE) && !self.is_angry(id) {
            item_stack.consume(1, self, Some(player));
            self.player_mut(player).update_item_in_hand(hand, &item_stack);
            self.try_to_tame(id, player);
            return InteractionResult::SUCCESS_SERVER;
        }
        self.animal_mob_interact(id, player, hand)
    }

    fn try_to_tame(&mut self, id: EntityId, player: EntityId) {
        if self.entity_mut(id).random.next_int_bounded(3) == 0 {
            self.tame(id, player);
            self.get_navigation(id, |navigation, _| navigation.stop());
            self.set_target(id, None);
            self.set_ordered_to_sit(id, true);
            self.broadcast_entity_event(id, crate::world::entity::entity_event::TAMING_SUCCEEDED);
        } else {
            self.broadcast_entity_event(id, crate::world::entity::entity_event::TAMING_FAILED);
        }
    }

    pub fn wolf_start_persistent_anger_timer(&mut self, id: EntityId) {
        let remaining_time = PERSISTENT_ANGER_TIME.sample(&mut self.entity_mut(id).random);
        self.set_time_to_remain_angry(id, remaining_time as i64);
    }

    pub fn wolf_get_breed_offspring(&mut self, id: EntityId, partner: EntityId) -> Option<Box<Mob>> {
        let mut baby = Box::new(Wolf::create());
        if self.is_wolf(partner) {
            let variant = if self.entity_mut(id).random.next_boolean() { get_variant(self.entity(id)) } else { get_variant(self.entity(partner)) };
            set_variant(&mut baby.entity, variant);
            if self.is_tame(id) {
                tamable_animal::set_owner_reference(&mut baby.entity, self.get_owner_reference(id));
                tamable_animal::set_tame(&mut baby, true, true);
                let parent1_collar_color = get_collar_color(self.entity(id));
                let parent2_collar_color = get_collar_color(self.entity(partner));
                set_collar_color(&mut baby.entity, DyeColor::get_mixed_color(self, parent1_collar_color, parent2_collar_color));
            }
            let sound_variant = variant_utils::pick_random_sound_variant(wolf_sound_variant::REGISTRY, &mut self.entity_mut(id).random);
            set_sound_variant(&mut baby.entity, sound_variant);
        }
        Some(baby)
    }

    pub fn wolf_can_mate(&self, id: EntityId, partner: EntityId) -> bool {
        if partner == id || !self.is_tame(id) || !self.is_wolf(partner) || !self.is_tame(partner) {
            return false;
        }
        if self.is_in_sitting_pose(partner) { false } else { self.is_in_love(id) && self.is_in_love(partner) }
    }

    pub fn wolf_wants_to_attack(&self, target: EntityId, owner: EntityId) -> bool {
        let target_type = registry::ENTITY_TYPE[self.entity(target).entity_type as usize];
        if matches!(target_type, "minecraft:creeper" | "minecraft:ghast" | "minecraft:armor_stand") {
            return false;
        }
        if self.is_wolf(target) {
            return !self.is_tame(target) || self.get_owner(target) != Some(owner);
        }
        if self.is_player(target) && self.is_player(owner) && !self.can_harm_player(owner, target) {
            return false;
        }
        !(self.is_tamable_animal(target) && self.is_tame(target))
    }
}
