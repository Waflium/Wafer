use std::sync::OnceLock;

use wafer_core::random::RandomSource;
use wafer_core::{AABB, BlockPos, Vec3, mth};
use wafer_data::properties::HORIZONTAL_FACING;
use wafer_data::{block_tags, item_tags, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use super::cat_sound_variant::{self, CatSoundSet};
use crate::server::level::ServerLevel;
use crate::world::attribute::environment_attributes;
use crate::world::difficulty_instance::DifficultyInstance;
use crate::world::entity::ai::MobAi;
use crate::world::entity::ai::goal::avoid_entity_goal::AvoidEntityGoal;
use crate::world::entity::ai::goal::breed_goal::BreedGoal;
use crate::world::entity::ai::goal::cat_lie_on_block_goal::CatLieOnBlockGoal;
use crate::world::entity::ai::goal::cat_sit_on_block_goal::CatSitOnBlockGoal;
use crate::world::entity::ai::goal::float_goal::FloatGoal;
use crate::world::entity::ai::goal::follow_owner_goal::FollowOwnerGoal;
use crate::world::entity::ai::goal::goal::Goal;
use crate::world::entity::ai::goal::leap_at_target_goal::LeapAtTargetGoal;
use crate::world::entity::ai::goal::look_at_player_goal::LookAtPlayerGoal;
use crate::world::entity::ai::goal::ocelot_attack_goal::OcelotAttackGoal;
use crate::world::entity::ai::goal::panic_goal::PanicGoal;
use crate::world::entity::ai::goal::sit_when_ordered_to_goal::SitWhenOrderedToGoal;
use crate::world::entity::ai::goal::target::nearest_attackable_target_goal::TargetType;
use crate::world::entity::ai::goal::target::non_tame_random_target_goal::NonTameRandomTargetGoal;
use crate::world::entity::ai::goal::tempt_goal::TemptGoal;
use crate::world::entity::ai::goal::water_avoiding_random_stroll_goal::WaterAvoidingRandomStrollGoal;
use crate::world::entity::ageable_mob;
use crate::world::entity::animal::Animal;
use crate::world::entity::attributes::CAT_ATTRIBUTES;
use crate::world::entity::data::{DataValue, Pose};
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::item::ItemEntity;
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{self, Mob, MobKind};
use crate::world::entity::monster::zombie::zombie;
use crate::world::entity::spawn_group_data::SpawnGroupData;
use crate::world::entity::tamable_animal::{self, TamableAnimal, TamableAnimalPanicGoal, tamable_animal_defaults};
use crate::world::entity::variant::variant_utils::{self, SpawnPrioritySelectors};
use crate::world::entity::{Entity, EntityDimensions, EntityId, LevelEntity, entity_event, sound_event};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::item::dye_color::DyeColor;
use crate::world::level::BlockGetter;
use crate::world::level::server_level_accessor::ServerLevelAccessor;
use crate::world::{InteractionHand, InteractionResult};
use crate::world::level::storage::tag_value_input;

pub const TEMPT_SPEED_MOD: f64 = 0.6;
pub const SPRINT_SPEED_MOD: f64 = 1.33;
pub const DATA_VARIANT_ID: usize = 20;
pub const IS_LYING: usize = 21;
pub const RELAX_STATE_ONE: usize = 22;
pub const DATA_COLLAR_COLOR: usize = 23;
pub const DATA_SOUND_VARIANT_ID: usize = 24;
pub const VARIANT_REGISTRY: &str = "minecraft:cat_variant";
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.6, 0.7).with_eye_height(0.35).with_passenger_attachments(&[Vec3::new(0.0, 0.5125f32 as f64, 0.0)]);
pub const BABY_DIMENSIONS: EntityDimensions =
    EntityDimensions::scalable(0.3, 0.35).with_eye_height(0.34375).with_passenger_attachments(&[Vec3::new(0.0, 0.3125f32 as f64, 0.0)]);
const DEFAULT_COLLAR_COLOR: DyeColor = DyeColor::VALUES[14];
const CAT_MORNING_GIFT: &str = "minecraft:gameplay/cat_morning_gift";

#[derive(Default)]
pub struct Cat {
    pub tamable_animal: TamableAnimal,
    lie_down_amount: f32,
    lie_down_amount_o: f32,
    lie_down_amount_tail: f32,
    lie_down_amount_o_tail: f32,
    is_lying_on_top_of_sleeping_player: bool,
    relax_state_one_amount: f32,
    relax_state_one_amount_o: f32,
}

pub fn cat_mut(mob: &mut Mob) -> &mut Cat {
    match &mut mob.kind {
        MobKind::Cat(cat) => cat,
        _ => unreachable!("not a cat"),
    }
}

fn spawn_selectors() -> &'static SpawnPrioritySelectors {
    static SELECTORS: OnceLock<SpawnPrioritySelectors> = OnceLock::new();
    SELECTORS.get_or_init(|| SpawnPrioritySelectors::load(VARIANT_REGISTRY, crate::core::data_registries::json_entries("minecraft:cat_variant")))
}

pub fn is_food(item_stack: &ItemStack) -> bool {
    item_stack.get_item().is(&item_tags::CAT_FOOD)
}

pub fn get_variant(entity: &Entity) -> i32 {
    match entity.data.get(DATA_VARIANT_ID) {
        DataValue::CatVariant(variant) => *variant,
        _ => 0,
    }
}

pub fn set_variant(entity: &mut Entity, variant: i32) {
    entity.data.set(DATA_VARIANT_ID, DataValue::CatVariant(variant));
}

pub fn get_sound_variant(entity: &Entity) -> i32 {
    match entity.data.get(DATA_SOUND_VARIANT_ID) {
        DataValue::CatSoundVariant(variant) => *variant,
        _ => 0,
    }
}

pub fn set_sound_variant(entity: &mut Entity, sound_variant: i32) {
    entity.data.set(DATA_SOUND_VARIANT_ID, DataValue::CatSoundVariant(sound_variant));
}

pub fn is_lying(entity: &Entity) -> bool {
    matches!(entity.data.get(IS_LYING), DataValue::Boolean(true))
}

pub fn is_relax_state_one(entity: &Entity) -> bool {
    matches!(entity.data.get(RELAX_STATE_ONE), DataValue::Boolean(true))
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

pub fn remove_when_far_away(mob: &Mob) -> bool {
    !tamable_animal::is_tame(&mob.entity) && mob.entity.tick_count > 2400
}

pub fn reassess_tame_goals(mob: &mut Mob) {
    let goal_selector = &mut mob.ai.goal_selector;
    let avoid_players_goal = match goal_selector.remove_goal(|goal| matches!(goal, Goal::CatAvoidEntity(_))) {
        Some(wrapped_goal) => {
            let running = wrapped_goal.is_running();
            let Goal::CatAvoidEntity(mut avoid_players_goal) = wrapped_goal.into_goal() else { unreachable!("a cat avoid entity goal") };
            if running {
                avoid_players_goal.avoid_entity_goal.stop();
            }
            avoid_players_goal
        }
        None => CatAvoidEntityGoal::new("minecraft:player", 16.0, 0.8, 1.33),
    };
    if !tamable_animal::is_tame(&mob.entity) {
        mob.ai.goal_selector.add_goal(4, Goal::CatAvoidEntity(avoid_players_goal));
    }
}

pub fn finalize_spawn(
    level: &mut (impl ServerLevelAccessor + ?Sized),
    mob: &mut Mob,
    difficulty: &DifficultyInstance,
    spawn_reason: EntitySpawnReason,
    group_data: Option<SpawnGroupData>,
) -> Option<SpawnGroupData> {
    let group_data = ageable_mob::finalize_spawn(level, mob, group_data);
    let group_data = mob::mob_finalize_spawn(level, mob, difficulty, spawn_reason, group_data);
    if let Some(variant) = variant_utils::select_variant_to_spawn(level, mob.entity.block_position(), spawn_selectors()) {
        set_variant(&mut mob.entity, variant);
    }
    let sound_variant = variant_utils::pick_random_sound_variant(cat_sound_variant::REGISTRY, level.get_random());
    set_sound_variant(&mut mob.entity, sound_variant);
    group_data
}

impl Cat {
    pub fn create() -> Mob {
        let mut data = tamable_animal_defaults();
        let black = variant_utils::read_variant(VARIANT_REGISTRY, "minecraft:black").unwrap_or(0);
        let classic = variant_utils::read_variant(cat_sound_variant::REGISTRY, "minecraft:classic").unwrap_or(0);
        data.extend([DataValue::CatVariant(black), DataValue::Boolean(false), DataValue::Boolean(false), DataValue::Int(DEFAULT_COLLAR_COLOR.id), DataValue::CatSoundVariant(classic)]);
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:cat"), DIMENSIONS, data);
        let mut mob = Mob::new(entity, LivingEntity::new(&CAT_ATTRIBUTES), MobKind::Cat(Cat::default()));
        Animal::init(&mut mob);
        reassess_tame_goals(&mut mob);
        mob
    }

    pub fn register_goals(ai: &mut MobAi, follow_distance: f64) {
        let tempt_goal = CatTemptGoal::new(TEMPT_SPEED_MOD, is_food, true);
        let float_goal = FloatGoal::new(&mut ai.navigation);
        let goal_selector = &mut ai.goal_selector;
        goal_selector.add_goal(1, Goal::Float(float_goal));
        goal_selector.add_goal(1, Goal::TamableAnimalPanic(TamableAnimalPanicGoal { panic_goal: PanicGoal::new(1.5) }));
        goal_selector.add_goal(2, Goal::SitWhenOrderedTo(SitWhenOrderedToGoal));
        goal_selector.add_goal(3, Goal::CatRelaxOnOwner(CatRelaxOnOwnerGoal::default()));
        goal_selector.add_goal(4, Goal::CatTempt(tempt_goal));
        goal_selector.add_goal(5, Goal::CatLieOnBlock(CatLieOnBlockGoal::new(1.1, 8)));
        goal_selector.add_goal(6, Goal::FollowOwner(FollowOwnerGoal::new(1.0, 10.0, 5.0)));
        goal_selector.add_goal(7, Goal::CatSitOnBlock(CatSitOnBlockGoal::new(0.8)));
        goal_selector.add_goal(8, Goal::LeapAtTarget(LeapAtTargetGoal::new(0.3)));
        goal_selector.add_goal(9, Goal::OcelotAttack(OcelotAttackGoal::default()));
        goal_selector.add_goal(10, Goal::Breed(BreedGoal::new(0.8)));
        goal_selector.add_goal(11, Goal::WaterAvoidingRandomStroll(WaterAvoidingRandomStrollGoal::with_probability(0.8, 1.0000001E-5)));
        goal_selector.add_goal(12, Goal::LookAtPlayer(LookAtPlayerGoal::new(10.0)));
        let target_selector = &mut ai.target_selector;
        target_selector.add_goal(1, Goal::NonTameRandomTarget(NonTameRandomTargetGoal::new(TargetType::Rabbit, false, None, follow_distance)));
        target_selector.add_goal(1, Goal::NonTameRandomTarget(NonTameRandomTargetGoal::new(TargetType::Turtle, false, Some(zombie::baby_on_land_selector), follow_distance)));
    }

    pub fn get_sound_set(mob: &Mob) -> &'static CatSoundSet {
        let sound_variant = cat_sound_variant::sound_variant(get_sound_variant(&mob.entity));
        if mob.is_baby() { &sound_variant.baby_sounds } else { &sound_variant.adult_sounds }
    }

    pub fn get_hurt_sound(mob: &Mob) -> &'static str {
        &Cat::get_sound_set(mob).hurt_sound
    }

    pub fn get_death_sound(mob: &Mob) -> &'static str {
        &Cat::get_sound_set(mob).death_sound
    }

    pub fn add_additional_save_data(mob: &Mob, output: &mut Vec<(String, Nbt)>) {
        TamableAnimal::add_additional_save_data(mob, output);
        output.push(("variant".into(), Nbt::String(variant_utils::variant_name(VARIANT_REGISTRY, get_variant(&mob.entity)).into())));
        output.push(("sound_variant".into(), Nbt::String(variant_utils::variant_name(cat_sound_variant::REGISTRY, get_sound_variant(&mob.entity)).into())));
        output.push(("CollarColor".into(), Nbt::Byte(get_collar_color(&mob.entity).id as i8)));
    }

    pub fn read_additional_save_data(mob: &mut Mob, input: &Nbt) {
        TamableAnimal::read_additional_save_data(mob, input);
        let read = |key: &str, registry: &str| variant_utils::read_variant_field(input, key, registry);
        if let Some(variant) = read("variant", VARIANT_REGISTRY) {
            set_variant(&mut mob.entity, variant);
        }
        if let Some(sound_variant) = read("sound_variant", cat_sound_variant::REGISTRY) {
            set_sound_variant(&mut mob.entity, sound_variant);
        }
        let collar_color = tag_value_input::read_byte(input, "CollarColor").map_or(DEFAULT_COLLAR_COLOR, |id| DyeColor::by_id(id as i32));
        set_collar_color(&mut mob.entity, collar_color);
    }
}

pub struct CatAvoidEntityGoal {
    pub avoid_entity_goal: AvoidEntityGoal,
}

impl CatAvoidEntityGoal {
    pub fn new(avoid_class: &'static str, max_dist: f32, walk_speed_modifier: f64, sprint_speed_modifier: f64) -> CatAvoidEntityGoal {
        CatAvoidEntityGoal { avoid_entity_goal: AvoidEntityGoal::new(avoid_class, max_dist, walk_speed_modifier, sprint_speed_modifier) }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        !level.is_tame(mob) && self.avoid_entity_goal.can_use(level, mob)
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        !level.is_tame(mob) && self.avoid_entity_goal.can_continue_to_use(level, mob)
    }
}

#[derive(Default)]
pub struct CatRelaxOnOwnerGoal {
    owner_player: Option<EntityId>,
    goal_pos: Option<BlockPos>,
    on_bed_ticks: i32,
}

impl CatRelaxOnOwnerGoal {
    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        if !level.is_tame(mob) {
            return false;
        }
        if level.is_ordered_to_sit(mob) {
            return false;
        }
        let Some((_, owner)) = level.get_owner_in_any_dimension(mob).filter(|&(_, owner)| level.is_player(owner)) else { return false };
        self.owner_player = Some(owner);
        if !level.is_sleeping(owner) {
            return false;
        }
        if level.entity(mob).distance_to_sqr(level.entity(owner)) > 100.0 {
            return false;
        }
        let owner_pos = level.entity(owner).block_position();
        let owner_pos_state = level.block_state(owner_pos);
        if owner_pos_state.is(&block_tags::CATS_CAN_LIE_ON) {
            self.goal_pos = Some(owner_pos_state.try_get(HORIZONTAL_FACING).map_or(owner_pos, |bed_dir| owner_pos.relative(bed_dir.opposite())));
            return !self.space_is_occupied(level, mob);
        }
        false
    }

    fn space_is_occupied(&self, level: &ServerLevel, mob: EntityId) -> bool {
        let area = AABB::of_block(self.goal_pos.expect("a goal position")).inflate(2.0, 2.0, 2.0);
        level.get_entities(None, &area).into_iter().any(|other_cat| other_cat != mob && level.is_cat(other_cat) && (is_lying(level.entity(other_cat)) || is_relax_state_one(level.entity(other_cat))))
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        level.is_tame(mob)
            && !level.is_ordered_to_sit(mob)
            && self.owner_player.is_some_and(|owner| level.is_player(owner) && level.is_sleeping(owner))
            && self.goal_pos.is_some()
            && !self.space_is_occupied(level, mob)
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        if let Some(goal_pos) = self.goal_pos {
            level.set_in_sitting_pose(mob, false);
            level.get_navigation(mob, |navigation, level| navigation.move_to(level, mob, goal_pos.x as f64, goal_pos.y as f64, goal_pos.z as f64, 1.1f32 as f64));
        }
    }

    pub fn stop(&mut self, level: &mut ServerLevel, mob: EntityId) {
        level.set_lying(mob, false);
        let sleep_timer = self.owner_player.and_then(|owner| level.find_player(owner)).map_or(0, |owner| owner.sleep_counter);
        if sleep_timer >= 100 {
            let roll = level.random.next_float();
            let position = level.entity(mob).position();
            if roll < level.environment_attributes().get_value(environment_attributes::CAT_WAKING_UP_GIFT_CHANCE, position) {
                self.give_morning_gift(level, mob);
            }
        }
        self.on_bed_ticks = 0;
        level.set_relax_state_one(mob, false);
        level.get_navigation(mob, |navigation, _| navigation.stop());
    }

    fn give_morning_gift(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let cat_pos = level.entity(mob).block_position();
        let random = &mut level.entity_mut(mob).random;
        let x = cat_pos.x + random.next_int_bounded(11) - 5;
        let y = cat_pos.y + random.next_int_bounded(5) - 2;
        let z = cat_pos.z + random.next_int_bounded(11) - 5;
        level.random_teleport(mob, x as f64, y as f64, z as f64, false, &|state| state.is(&block_tags::CAT_DOES_NOT_TELEPORT_TO));
        let cat_pos = level.entity(mob).block_position();
        let y_body_rot = level.living(mob).y_body_rot;
        let angle = (y_body_rot * (std::f64::consts::PI / 180.0) as f32) as f64;
        let (x, z) = (cat_pos.x as f64 - mth::sin(angle) as f64, cat_pos.z as f64 + mth::cos(angle) as f64);
        level.drop_from_gift_loot_table(mob, CAT_MORNING_GIFT, |level, item_stack| {
            level.add_fresh_entity(LevelEntity::Item(ItemEntity::new(Vec3::new(x, cat_pos.y as f64, z), item_stack)));
        });
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId, on_bed_tick_delay: i32) {
        let (Some(owner), Some(goal_pos)) = (self.owner_player, self.goal_pos) else { return };
        if !level.players.contains_key(&owner) {
            return;
        }
        level.set_in_sitting_pose(mob, false);
        level.get_navigation(mob, |navigation, level| navigation.move_to(level, mob, goal_pos.x as f64, goal_pos.y as f64, goal_pos.z as f64, 1.1f32 as f64));
        if level.entity(mob).distance_to_sqr(level.entity(owner)) < 2.5 {
            self.on_bed_ticks += 1;
            if self.on_bed_ticks > on_bed_tick_delay {
                level.set_lying(mob, true);
                level.set_relax_state_one(mob, false);
            } else {
                level.look_at(mob, owner, 45.0, 45.0);
                level.set_relax_state_one(mob, true);
            }
        } else {
            level.set_lying(mob, false);
        }
    }
}

pub struct CatTemptGoal {
    pub tempt_goal: TemptGoal,
    selected_player: Option<EntityId>,
}

impl CatTemptGoal {
    pub fn new(speed_modifier: f64, items: fn(&ItemStack) -> bool, can_scare: bool) -> CatTemptGoal {
        CatTemptGoal { tempt_goal: TemptGoal::new(speed_modifier, items, can_scare), selected_player: None }
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId, select_delay: i32, unselect_delay: i32) {
        self.tempt_goal.tick(level, mob);
        let random = &mut level.entity_mut(mob).random;
        if self.selected_player.is_none() && random.next_int_bounded(select_delay) == 0 {
            self.selected_player = self.tempt_goal.player;
        } else if random.next_int_bounded(unselect_delay) == 0 {
            self.selected_player = None;
        }
    }

    fn can_scare(&self) -> bool {
        if self.selected_player.is_some() && self.selected_player == self.tempt_goal.player { false } else { self.tempt_goal.can_scare() }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        self.tempt_goal.can_use(level, mob) && !level.is_tame(mob)
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        let can_scare = self.can_scare();
        self.tempt_goal.is_not_scared(level, mob, can_scare) && self.can_use(level, mob)
    }
}

impl ServerLevel {
    pub fn is_cat(&self, id: EntityId) -> bool {
        self.mob(id).is_some_and(|mob| matches!(mob.kind, MobKind::Cat(_)))
    }

    pub fn set_lying(&mut self, id: EntityId, value: bool) {
        self.entity_mut(id).data.set(IS_LYING, DataValue::Boolean(value));
    }

    pub fn is_lying(&self, id: EntityId) -> bool {
        is_lying(self.entity(id))
    }

    fn set_relax_state_one(&mut self, id: EntityId, value: bool) {
        self.entity_mut(id).data.set(RELAX_STATE_ONE, DataValue::Boolean(value));
    }

    pub fn cat_custom_server_ai_step(&mut self, id: EntityId) {
        let move_control = &self.mob(id).expect("a cat").ai.move_control;
        let (pose, sprinting) = if move_control.has_wanted() {
            let speed = move_control.speed_modifier;
            if speed == TEMPT_SPEED_MOD {
                (Pose::Crouching, false)
            } else if speed == SPRINT_SPEED_MOD {
                (Pose::Standing, true)
            } else {
                (Pose::Standing, false)
            }
        } else {
            (Pose::Standing, false)
        };
        self.entity_mut(id).set_pose(pose);
        self.set_sprinting(id, sprinting);
    }

    pub fn cat_get_ambient_sound(&mut self, id: EntityId) -> &'static str {
        let sound_set = Cat::get_sound_set(self.mob(id).expect("a cat"));
        if self.is_tame(id) {
            if self.is_in_love(id) {
                &sound_set.purr_sound
            } else if self.entity_mut(id).random.next_int_bounded(4) == 0 {
                &sound_set.purreow_sound
            } else {
                &sound_set.ambient_sound
            }
        } else {
            &sound_set.stray_ambient_sound
        }
    }

    pub fn cat_hiss(&mut self, id: EntityId) {
        let sound = sound_event(&Cat::get_sound_set(self.mob(id).expect("a cat")).hiss_sound);
        self.make_sound(id, sound);
    }

    pub fn cat_play_eating_sound(&mut self, id: EntityId) {
        let sound = sound_event(&Cat::get_sound_set(self.mob(id).expect("a cat")).eat_sound);
        self.entity_play_sound(id, sound, 1.0, 1.0);
    }

    pub fn cat_tick(&mut self, id: EntityId) {
        self.mob_tick(id);
        let Some(mob) = self.mob(id) else { return };
        let tempting = mob.ai.goal_selector.get_available_goals().iter().any(|goal| matches!(goal.get_goal(), Goal::CatTempt(goal) if goal.tempt_goal.is_running()));
        if tempting && !self.is_tame(id) && self.entity(id).tick_count % 100 == 0 {
            let sound = sound_event(&Cat::get_sound_set(mob).beg_for_food_sound);
            self.entity_play_sound(id, sound, 1.0, 1.0);
        }
        self.handle_lie_down(id);
    }

    fn handle_lie_down(&mut self, id: EntityId) {
        let e = self.entity(id);
        let (lying, relax_state_one) = (is_lying(e), is_relax_state_one(e));
        if (lying || relax_state_one) && e.tick_count % 5 == 0 {
            let sound = sound_event(&Cat::get_sound_set(self.mob(id).expect("a cat")).purr_sound);
            let random = &mut self.entity_mut(id).random;
            let volume = 0.6 + 0.4 * (random.next_float() - random.next_float());
            self.entity_play_sound(id, sound, volume, 1.0);
        }
        let cat_pos = self.entity(id).block_position();
        let lying_on_top_of_sleeping_player = lying && {
            let area = AABB::of_block(cat_pos).inflate(2.0, 2.0, 2.0);
            self.get_entities(None, &area).into_iter().any(|player| self.is_player(player) && self.is_sleeping(player))
        };
        let cat = cat_mut(self.mob_mut(id).expect("a cat"));
        cat.update_lie_down_amount(lying);
        cat.update_relax_state_one_amount(relax_state_one);
        cat.is_lying_on_top_of_sleeping_player = lying_on_top_of_sleeping_player;
    }

    pub fn cat_get_breed_offspring(&mut self, id: EntityId, partner: EntityId) -> Option<Box<Mob>> {
        let mut baby = Box::new(Cat::create());
        if self.is_cat(partner) {
            let variant = if self.entity_mut(id).random.next_boolean() { get_variant(self.entity(id)) } else { get_variant(self.entity(partner)) };
            set_variant(&mut baby.entity, variant);
            if self.is_tame(id) {
                tamable_animal::set_owner_reference(&mut baby.entity, self.get_owner_reference(id));
                tamable_animal::set_tame(&mut baby, true, true);
                let parent1_collar_color = get_collar_color(self.entity(id));
                let parent2_collar_color = get_collar_color(self.entity(partner));
                set_collar_color(&mut baby.entity, DyeColor::get_mixed_color(self, parent1_collar_color, parent2_collar_color));
            }
        }
        Some(baby)
    }

    pub fn cat_can_mate(&self, id: EntityId, partner: EntityId) -> bool {
        if !self.is_tame(id) {
            false
        } else if !self.is_cat(partner) {
            false
        } else {
            self.is_tame(partner) && self.animal_can_mate(id, partner)
        }
    }

    pub fn cat_mob_interact(&mut self, id: EntityId, player: EntityId, hand: InteractionHand) -> InteractionResult {
        let mut item_stack = self.player(player).item_in_hand(hand).unwrap_or_else(ItemStack::empty);
        if self.is_tame(id) {
            if self.is_owned_by(id, player) {
                if item_stack.get_item().is(&item_tags::CAT_COLLAR_DYES) {
                    let color = item_stack.get(dc::DYE).and_then(Nbt::as_str).and_then(DyeColor::by_name);
                    if let Some(color) = color.filter(|&color| color != get_collar_color(self.entity(id))) {
                        set_collar_color(self.entity_mut(id), color);
                        item_stack.consume(1, self, Some(player));
                        self.player_mut(player).update_item_in_hand(hand, &item_stack);
                        self.mob_mut(id).expect("a cat").persistence_required = true;
                        return InteractionResult::SUCCESS;
                    }
                } else if is_food(&item_stack) && self.health(id) < self.max_health(id) {
                    self.tamable_animal_feed(id, player, hand, &mut item_stack, 1.0, 1.0);
                    return InteractionResult::SUCCESS;
                }
                let parent_interaction = self.animal_mob_interact(id, player, hand);
                if !parent_interaction.consumes_action() {
                    let ordered_to_sit = !self.is_ordered_to_sit(id);
                    self.set_ordered_to_sit(id, ordered_to_sit);
                    return InteractionResult::SUCCESS;
                }
                return parent_interaction;
            }
        } else if is_food(&item_stack) {
            self.use_player_item(player, hand, &mut item_stack);
            self.try_to_tame_cat(id, player);
            self.mob_mut(id).expect("a cat").persistence_required = true;
            self.cat_play_eating_sound(id);
            return InteractionResult::SUCCESS;
        }
        let interact = self.animal_mob_interact(id, player, hand);
        if interact.consumes_action() {
            self.mob_mut(id).expect("a cat").persistence_required = true;
        }
        interact
    }

    fn try_to_tame_cat(&mut self, id: EntityId, player: EntityId) {
        if self.entity_mut(id).random.next_int_bounded(3) == 0 {
            self.tame(id, player);
            self.set_ordered_to_sit(id, true);
            self.broadcast_entity_event(id, entity_event::TAMING_SUCCEEDED);
        } else {
            self.broadcast_entity_event(id, entity_event::TAMING_FAILED);
        }
    }

    pub fn cat_is_stepping_carefully(&self, id: EntityId) -> bool {
        self.entity(id).is_crouching() || self.entity_is_stepping_carefully(id)
    }
}

impl Cat {
    fn update_lie_down_amount(&mut self, lying: bool) {
        self.lie_down_amount_o = self.lie_down_amount;
        self.lie_down_amount_o_tail = self.lie_down_amount_tail;
        if lying {
            self.lie_down_amount = (self.lie_down_amount + 0.15).min(1.0);
            self.lie_down_amount_tail = (self.lie_down_amount_tail + 0.08).min(1.0);
        } else {
            self.lie_down_amount = (self.lie_down_amount - 0.22).max(0.0);
            self.lie_down_amount_tail = (self.lie_down_amount_tail - 0.13).max(0.0);
        }
    }

    fn update_relax_state_one_amount(&mut self, relax_state_one: bool) {
        self.relax_state_one_amount_o = self.relax_state_one_amount;
        if relax_state_one {
            self.relax_state_one_amount = (self.relax_state_one_amount + 0.1).min(1.0);
        } else {
            self.relax_state_one_amount = (self.relax_state_one_amount - 0.13).max(0.0);
        }
    }

    pub fn is_lying_on_top_of_sleeping_player(&self) -> bool {
        self.is_lying_on_top_of_sleeping_player
    }

    pub fn get_lie_down_amount(&self, a: f32) -> f32 {
        mth::lerp_f(a, self.lie_down_amount_o, self.lie_down_amount)
    }

    pub fn get_lie_down_amount_tail(&self, a: f32) -> f32 {
        mth::lerp_f(a, self.lie_down_amount_o_tail, self.lie_down_amount_tail)
    }

    pub fn get_relax_state_one_amount(&self, a: f32) -> f32 {
        mth::lerp_f(a, self.relax_state_one_amount_o, self.relax_state_one_amount)
    }
}
