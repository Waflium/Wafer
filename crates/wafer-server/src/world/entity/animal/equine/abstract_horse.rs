use wafer_core::random::RandomSource;
use wafer_core::{AABB, BlockPos, Vec3, mth};
use wafer_data::{EquipmentSlot, blocks, item_tags, items};
use wafer_protocol::nbt::Nbt;

use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::entity::ageable_mob::AgeableMobGroupData;
use crate::world::entity::ai::MobAi;
use crate::world::entity::ai::goal::breed_goal::BreedGoal;
use crate::world::entity::ai::goal::float_goal::FloatGoal;
use crate::world::entity::ai::goal::follow_parent_goal::FollowParentGoal;
use crate::world::entity::ai::goal::goal::Goal;
use crate::world::entity::ai::goal::look_at_player_goal::LookAtPlayerGoal;
use crate::world::entity::ai::goal::panic_goal::PanicGoal;
use crate::world::entity::ai::goal::random_look_around_goal::RandomLookAroundGoal;
use crate::world::entity::ai::goal::random_stand_goal::RandomStandGoal;
use crate::world::entity::ai::goal::run_around_like_crazy_goal::RunAroundLikeCrazyGoal;
use crate::world::entity::ai::goal::tempt_goal::TemptGoal;
use crate::world::entity::ai::goal::water_avoiding_random_stroll_goal::WaterAvoidingRandomStrollGoal;
use crate::world::entity::ai::targeting::targeting_conditions::TargetingConditions;
use crate::world::entity::animal::Animal;
use crate::world::entity::attributes::Attribute;
use crate::world::entity::data::{DataValue, ageable_mob_defaults};
use crate::world::entity::entity_reference::EntityReference;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::spawn_group_data::SpawnGroupData;
use crate::world::entity::vehicle::dismount_helper;
use crate::world::entity::{Entity, EntityId, get_collision_horizontal_escape_vector, sound_event};
use crate::world::item::ItemStack;
use crate::world::level::BlockGetter;
use crate::world::level::server_level_accessor::ServerLevelAccessor;
use crate::world::simple_container::{SimpleContainer, SimpleContainerKind};
use crate::world::{InteractionHand, InteractionResult};

pub const CHEST_SLOT_OFFSET: i32 = 499;
pub const INVENTORY_SLOT_OFFSET: i32 = 500;
const BACKWARDS_MOVE_SPEED_FACTOR: f32 = 0.25;
const SIDEWAYS_MOVE_SPEED_FACTOR: f32 = 0.5;
pub const DATA_ID_FLAGS: usize = 18;
const FLAG_TAME: i8 = 2;
const FLAG_BRED: i8 = 8;
const FLAG_EATING: i8 = 16;
const FLAG_STANDING: i8 = 32;
const FLAG_OPEN_MOUTH: i8 = 64;
pub const BABY_SCALE: f32 = 0.7;
const DEFAULT_TEMPER: i32 = 0;

fn parent_horse_selector(target: EntityId, level: &ServerLevel) -> bool {
    level.is_abstract_horse(target) && is_bred(level.entity(target))
}

pub struct AbstractHorse {
    pub animal: Animal,
    eating_counter: i32,
    mouth_counter: i32,
    stand_counter: i32,
    pub tail_counter: i32,
    pub sprint_counter: i32,
    pub inventory: SimpleContainer,
    pub inventory_version: u32,
    pub temper: i32,
    pub player_jump_pending_scale: f32,
    pub allow_stand_sliding: bool,
    eat_anim: f32,
    eat_anim_o: f32,
    stand_anim: f32,
    pub stand_anim_o: f32,
    mouth_anim: f32,
    mouth_anim_o: f32,
    pub can_gallop: bool,
    pub gallop_sound_counter: i32,
    pub owner: Option<EntityReference>,
}

impl Default for AbstractHorse {
    fn default() -> AbstractHorse {
        AbstractHorse {
            animal: Animal::default(),
            eating_counter: 0,
            mouth_counter: 0,
            stand_counter: 0,
            tail_counter: 0,
            sprint_counter: 0,
            inventory: SimpleContainer::new(SimpleContainerKind::Plain, 0),
            inventory_version: 0,
            temper: DEFAULT_TEMPER,
            player_jump_pending_scale: 0.0,
            allow_stand_sliding: false,
            eat_anim: 0.0,
            eat_anim_o: 0.0,
            stand_anim: 0.0,
            stand_anim_o: 0.0,
            mouth_anim: 0.0,
            mouth_anim_o: 0.0,
            can_gallop: true,
            gallop_sound_counter: 0,
            owner: None,
        }
    }
}

pub fn abstract_horse_defaults() -> Vec<DataValue> {
    let mut data = ageable_mob_defaults();
    data.push(DataValue::Byte(0));
    data
}

pub fn abstract_horse(mob: &Mob) -> &AbstractHorse {
    mob.abstract_horse().expect("an abstract horse")
}

pub fn abstract_horse_mut(mob: &mut Mob) -> &mut AbstractHorse {
    mob.abstract_horse_mut().expect("an abstract horse")
}

fn flags(entity: &Entity) -> i8 {
    match entity.data.get(DATA_ID_FLAGS) {
        DataValue::Byte(flags) => *flags,
        _ => 0,
    }
}

fn get_flag(entity: &Entity, flag: i8) -> bool {
    flags(entity) & flag != 0
}

fn set_flag(entity: &mut Entity, flag: i8, value: bool) {
    let current = flags(entity);
    entity.data.set(DATA_ID_FLAGS, DataValue::Byte(if value { current | flag } else { current & !flag }));
}

pub fn is_tamed(entity: &Entity) -> bool {
    get_flag(entity, FLAG_TAME)
}

pub fn set_tamed(entity: &mut Entity, flag: bool) {
    set_flag(entity, FLAG_TAME, flag);
}

pub fn is_eating(entity: &Entity) -> bool {
    get_flag(entity, FLAG_EATING)
}

pub fn is_standing(entity: &Entity) -> bool {
    get_flag(entity, FLAG_STANDING)
}

pub fn is_bred(entity: &Entity) -> bool {
    get_flag(entity, FLAG_BRED)
}

pub fn set_bred(entity: &mut Entity, flag: bool) {
    set_flag(entity, FLAG_BRED, flag);
}

pub fn set_eating(entity: &mut Entity, flag: bool) {
    set_flag(entity, FLAG_EATING, flag);
}

pub fn is_food(item_stack: &ItemStack) -> bool {
    item_stack.get_item().is(&item_tags::HORSE_FOOD)
}

pub fn generate_max_health(mut integer_by_bound_provider: impl FnMut(i32) -> i32) -> f32 {
    15.0 + integer_by_bound_provider(8) as f32 + integer_by_bound_provider(9) as f32
}

pub fn generate_jump_strength(mut probability_provider: impl FnMut() -> f64) -> f64 {
    0.4f32 as f64 + probability_provider() * 0.2 + probability_provider() * 0.2 + probability_provider() * 0.2
}

pub fn generate_speed(mut probability_provider: impl FnMut() -> f64) -> f64 {
    (0.45f32 as f64 + probability_provider() * 0.3 + probability_provider() * 0.3 + probability_provider() * 0.3) * 0.25
}

fn min_movement_speed() -> f32 {
    generate_speed(|| 0.0) as f32
}

fn max_movement_speed() -> f32 {
    generate_speed(|| 1.0) as f32
}

fn min_jump_strength() -> f32 {
    generate_jump_strength(|| 0.0) as f32
}

fn max_jump_strength() -> f32 {
    generate_jump_strength(|| 1.0) as f32
}

fn min_health() -> f32 {
    generate_max_health(|_| 0)
}

fn max_health() -> f32 {
    generate_max_health(|i| i - 1)
}

pub fn create_offspring_attribute(parent_a_value: f64, parent_b_value: f64, attribute_range_min: f64, attribute_range_max: f64, random: &mut impl RandomSource) -> f64 {
    if attribute_range_max <= attribute_range_min {
        panic!("Incorrect range for an attribute");
    }
    let parent_a_value = mth::clamp(parent_a_value, attribute_range_min, attribute_range_max);
    let parent_b_value = mth::clamp(parent_b_value, attribute_range_min, attribute_range_max);
    let margin = 0.15 * (attribute_range_max - attribute_range_min);
    let range = (parent_a_value - parent_b_value).abs() + margin * 2.0;
    let average = (parent_a_value + parent_b_value) / 2.0;
    let baby_quality = (random.next_double() + random.next_double() + random.next_double()) / 3.0 - 0.5;
    let new_value = average + range * baby_quality;
    if new_value > attribute_range_max {
        let difference = new_value - attribute_range_max;
        attribute_range_max - difference
    } else if new_value < attribute_range_min {
        let difference = attribute_range_min - new_value;
        attribute_range_min + difference
    } else {
        new_value
    }
}

pub fn get_inventory_size(mob: &Mob) -> usize {
    crate::world::inventory::abstract_mount_inventory_menu::get_inventory_size(get_inventory_columns(mob) as usize)
}

pub fn create_inventory(mob: &mut Mob) {
    let size = get_inventory_size(mob);
    let horse = abstract_horse_mut(mob);
    let mut inventory = SimpleContainer::new(SimpleContainerKind::Plain, size);
    let max = horse.inventory.items.len().min(size);
    for slot in 0..max {
        let item_stack = &horse.inventory.items[slot];
        if !item_stack.is_empty() {
            inventory.items[slot] = item_stack.copy();
        }
    }
    horse.inventory = inventory;
    horse.inventory_version += 1;
}

pub fn get_inventory_columns(mob: &Mob) -> i32 {
    match mob.kind {
        MobKind::Donkey(_) | MobKind::Mule(_) => super::abstract_chested_horse::get_inventory_columns(mob),
        MobKind::Llama(_) | MobKind::TraderLlama(_) => super::llama::get_inventory_columns(mob),
        _ => 0,
    }
}

pub fn randomize_attributes(mob: &mut Mob, random: &mut (impl RandomSource + ?Sized)) {
    match mob.kind {
        MobKind::Horse(_) => super::horse::randomize_attributes(mob, random),
        MobKind::Donkey(_) | MobKind::Mule(_) | MobKind::Llama(_) | MobKind::TraderLlama(_) => super::abstract_chested_horse::randomize_attributes(mob, random),
        MobKind::SkeletonHorse(_) => super::skeleton_horse::randomize_attributes(mob, random),
        MobKind::ZombieHorse(_) => super::zombie_horse::randomize_attributes(mob, random),
        _ => {}
    }
}

pub fn finalize_spawn(level: &mut (impl ServerLevelAccessor + ?Sized), mob: &mut Mob, group_data: Option<SpawnGroupData>) -> Option<SpawnGroupData> {
    let group_data = group_data.or(Some(SpawnGroupData::AgeableMob(AgeableMobGroupData::with_chance(true, 0.2))));
    randomize_attributes(mob, level.get_random());
    group_data
}

impl AbstractHorse {
    pub fn init(mob: &mut Mob) {
        Animal::init(mob);
        create_inventory(mob);
    }

    pub fn register_goals(ai: &mut MobAi, add_behaviour_goals: fn(&mut MobAi)) {
        let goal_selector = &mut ai.goal_selector;
        goal_selector.add_goal(1, Goal::RunAroundLikeCrazy(RunAroundLikeCrazyGoal::new(1.2)));
        goal_selector.add_goal(2, Goal::Breed(BreedGoal::with_partner_class(1.0, ServerLevel::is_abstract_horse)));
        goal_selector.add_goal(4, Goal::FollowParent(FollowParentGoal::new(1.0)));
        goal_selector.add_goal(6, Goal::WaterAvoidingRandomStroll(WaterAvoidingRandomStrollGoal::new(0.7)));
        goal_selector.add_goal(7, Goal::LookAtPlayer(LookAtPlayerGoal::new(6.0)));
        goal_selector.add_goal(8, Goal::RandomLookAround(RandomLookAroundGoal::default()));
        goal_selector.add_goal(9, Goal::RandomStand(RandomStandGoal::new(get_ambient_stand_interval())));
        add_behaviour_goals(ai);
    }

    pub fn add_behaviour_goals(ai: &mut MobAi) {
        let float_goal = FloatGoal::new(&mut ai.navigation);
        let goal_selector = &mut ai.goal_selector;
        goal_selector.add_goal(0, Goal::Float(float_goal));
        goal_selector.add_goal(1, Goal::MountPanic(MountPanicGoal::new(1.2)));
        goal_selector.add_goal(3, Goal::Tempt(TemptGoal::new(1.25, |i| i.get_item().is(&item_tags::HORSE_TEMPT_ITEMS), false)));
    }

    pub fn add_additional_save_data(mob: &Mob, output: &mut Vec<(String, Nbt)>) {
        output.push(("EatingHaystack".into(), Nbt::Byte(is_eating(&mob.entity) as i8)));
        output.push(("Bred".into(), Nbt::Byte(is_bred(&mob.entity) as i8)));
        output.push(("Temper".into(), Nbt::Int(abstract_horse(mob).temper)));
        let tamed = if matches!(mob.kind, MobKind::Camel(_) | MobKind::CamelHusk(_)) { crate::world::entity::animal::camel::camel::is_tamed() } else { is_tamed(&mob.entity) };
        output.push(("Tame".into(), Nbt::Byte(tamed as i8)));
        EntityReference::store(abstract_horse(mob).owner, output, "Owner");
    }

    pub fn read_additional_save_data(mob: &mut Mob, input: &Nbt) {
        let get_boolean = |key: &str| input.get(key).and_then(Nbt::as_i64).is_some_and(|v| v != 0);
        set_eating(&mut mob.entity, get_boolean("EatingHaystack"));
        set_bred(&mut mob.entity, get_boolean("Bred"));
        let temper = input.get("Temper").and_then(crate::world::level::block::entity::int_value).unwrap_or(DEFAULT_TEMPER);
        abstract_horse_mut(mob).temper = temper;
        set_tamed(&mut mob.entity, get_boolean("Tame"));
        abstract_horse_mut(mob).owner = EntityReference::read(input, "Owner");
    }
}

pub fn get_max_spawn_cluster_size() -> i32 {
    6
}

pub fn get_max_temper() -> i32 {
    100
}

pub fn get_sound_volume() -> f32 {
    0.8
}

pub fn get_ambient_sound_interval() -> i32 {
    400
}

pub fn get_ambient_stand_interval() -> i32 {
    get_ambient_sound_interval()
}

pub fn can_perform_rearing() -> bool {
    true
}

pub fn can_eat_grass() -> bool {
    true
}

pub fn is_mob_controlled() -> bool {
    false
}

impl ServerLevel {
    pub fn get_max_temper(&self, id: EntityId) -> i32 {
        if self.is_llama(id) { super::llama::get_max_temper() } else { get_max_temper() }
    }

    pub fn can_perform_rearing(&self, id: EntityId) -> bool {
        !self.is_llama(id) && !self.is_camel(id) && can_perform_rearing()
    }

    pub fn can_eat_grass(&self, id: EntityId) -> bool {
        !self.is_llama(id) && can_eat_grass()
    }

    pub fn is_mob_controlled(&self, id: EntityId) -> bool {
        if self.is_zombie_horse(id) {
            self.zombie_horse_is_mob_controlled(id)
        } else if self.is_camel_husk(id) {
            self.camel_husk_is_mob_controlled(id)
        } else {
            is_mob_controlled()
        }
    }
}

pub fn on_climbable() -> bool {
    false
}

fn is_wood_sound_type(sound_type: &wafer_data::SoundType) -> bool {
    let step = |name: &str| sound_event(name);
    [
        "minecraft:block.wood.step",
        "minecraft:block.nether_wood.step",
        "minecraft:block.stem.step",
        "minecraft:block.cherry_wood.step",
        "minecraft:block.bamboo_wood.step",
    ]
    .into_iter()
    .any(|name| sound_type.step_sound == step(name) && sound_type.break_sound == step(&name.replace(".step", ".break")))
}

pub struct MountPanicGoal {
    pub panic_goal: PanicGoal,
}

impl MountPanicGoal {
    pub fn new(speed_modifier: f64) -> MountPanicGoal {
        MountPanicGoal { panic_goal: PanicGoal::new(speed_modifier) }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        !level.is_mob_controlled(mob) && self.panic_goal.can_use(level, mob)
    }
}

impl ServerLevel {
    pub fn is_abstract_horse(&self, id: EntityId) -> bool {
        self.mob(id).is_some_and(|mob| mob.abstract_horse().is_some())
    }

    pub fn is_tamed(&self, id: EntityId) -> bool {
        if self.is_camel(id) { crate::world::entity::animal::camel::camel::is_tamed() } else { is_tamed(self.entity(id)) }
    }

    pub fn is_eating(&self, id: EntityId) -> bool {
        is_eating(self.entity(id))
    }

    pub fn is_standing(&self, id: EntityId) -> bool {
        is_standing(self.entity(id))
    }

    pub fn set_eating(&mut self, id: EntityId, flag: bool) {
        set_eating(self.entity_mut(id), flag);
    }

    pub fn abstract_horse_get_owner_reference(&self, id: EntityId) -> Option<EntityReference> {
        abstract_horse(self.mob(id).expect("an abstract horse")).owner
    }

    pub fn set_horse_owner(&mut self, id: EntityId, owner: Option<EntityId>) {
        let owner = owner.map(|owner| EntityReference::of(self.entity(owner).uuid));
        abstract_horse_mut(self.mob_mut(id).expect("an abstract horse")).owner = owner;
    }

    pub fn abstract_horse_can_use_slot(&self, id: EntityId, slot: EquipmentSlot) -> bool {
        if slot != EquipmentSlot::Saddle { true } else { self.is_alive(id) && !self.is_baby(id) && self.is_tamed(id) }
    }

    pub fn equip_body_armor(&mut self, id: EntityId, player: EntityId, item_stack: &mut ItemStack) {
        if self.is_equippable_in_slot(id, item_stack, EquipmentSlot::Body) {
            let armor = item_stack.consume_and_return(1, self, Some(player));
            self.set_item_slot_and_drop_when_killed(id, EquipmentSlot::Body, armor);
        }
    }

    pub fn get_temper(&self, id: EntityId) -> i32 {
        abstract_horse(self.mob(id).expect("an abstract horse")).temper
    }

    pub fn modify_temper(&mut self, id: EntityId, amount: i32) -> i32 {
        let temper = mth::clamp_i(self.get_temper(id) + amount, 0, self.get_max_temper(id));
        abstract_horse_mut(self.mob_mut(id).expect("an abstract horse")).temper = temper;
        temper
    }

    pub fn abstract_horse_is_pushable(&self, id: EntityId) -> bool {
        !self.is_vehicle(id)
    }

    fn eating(&mut self, id: EntityId) {
        self.open_mouth(id);
        if !self.entity(id).is_silent()
            && let Some(eating_sound) = self.get_eating_sound(id)
        {
            let source = self.sound_source(id);
            let e = self.entity_mut(id);
            let (x, y, z) = (e.x(), e.y(), e.z());
            let pitch = 1.0 + (e.random.next_float() - e.random.next_float()) * 0.2;
            self.play_sound_at(None, x, y, z, sound_event(eating_sound), source, 1.0, pitch);
        }
    }

    pub fn abstract_horse_cause_fall_damage(&mut self, id: EntityId, fall_distance: f64, damage_modifier: f32, damage_source: DamageSource) -> bool {
        if fall_distance > 1.0 {
            let sound = if self.is_baby(id) { "minecraft:entity.baby_horse.land" } else { "minecraft:entity.horse.land" };
            self.entity_play_sound(id, sound_event(sound), 0.4, 1.0);
        }
        let dmg = self.calculate_fall_damage(id, fall_distance, damage_modifier);
        if dmg <= 0 {
            return false;
        }
        self.hurt(id, damage_source.clone(), dmg as f32);
        if self.entity_ref(id).is_some() {
            self.propagate_fall_to_passengers(id, fall_distance, damage_modifier, damage_source);
            self.play_block_fall_sound(id);
        }
        true
    }

    pub fn create_inventory(&mut self, id: EntityId) {
        create_inventory(self.mob_mut(id).expect("an abstract horse"));
    }

    pub fn get_equip_sound_for_saddle(&self, id: EntityId) -> Option<i32> {
        let mob = self.mob(id)?;
        match mob.kind {
            MobKind::Camel(_) | MobKind::CamelHusk(_) => Some(sound_event(crate::world::entity::animal::camel::camel::get_saddle_sound(mob))),
            _ => self.is_abstract_horse(id).then(|| sound_event("minecraft:entity.horse.saddle")),
        }
    }

    pub fn abstract_horse_hurt_server(&mut self, id: EntityId, source: DamageSource, damage: f32) -> bool {
        let was_hurt = self.living_entity_hurt_server(id, source, damage);
        if was_hurt && self.mob(id).is_some() && self.entity_mut(id).random.next_int_bounded(3) == 0 {
            self.stand_if_possible(id);
        }
        was_hurt
    }

    pub fn get_eating_sound(&self, id: EntityId) -> Option<&'static str> {
        let mob = self.mob(id).expect("an abstract horse");
        match mob.kind {
            MobKind::Horse(_) => Some(super::horse::get_eating_sound(mob)),
            MobKind::Donkey(_) => Some(super::donkey::get_eating_sound()),
            MobKind::Mule(_) => Some(super::mule::get_eating_sound()),
            MobKind::ZombieHorse(_) => Some(super::zombie_horse::get_eating_sound()),
            MobKind::Llama(_) | MobKind::TraderLlama(_) => Some(super::llama::get_eating_sound()),
            MobKind::Camel(_) | MobKind::CamelHusk(_) => Some(crate::world::entity::animal::camel::camel::get_eating_sound(mob)),
            _ => None,
        }
    }

    pub fn get_angry_sound(&self, id: EntityId) -> Option<&'static str> {
        let mob = self.mob(id).expect("an abstract horse");
        match mob.kind {
            MobKind::Horse(_) => Some(super::horse::get_angry_sound(mob)),
            MobKind::Donkey(_) => Some(super::donkey::get_angry_sound()),
            MobKind::Mule(_) => Some(super::mule::get_angry_sound()),
            MobKind::ZombieHorse(_) => Some(super::zombie_horse::get_angry_sound()),
            MobKind::Llama(_) | MobKind::TraderLlama(_) => Some(super::llama::get_angry_sound()),
            _ => None,
        }
    }

    pub fn abstract_horse_play_step_sound(&mut self, id: EntityId, pos: BlockPos, block_state: wafer_data::BlockState) {
        if block_state.liquid() {
            return;
        }
        let above_state = self.block_state(pos.above());
        let mut sound_type = block_state.sound_type();
        if above_state.is_block(blocks::SNOW) {
            sound_type = above_state.sound_type();
        }
        let horse = abstract_horse(self.mob(id).expect("an abstract horse"));
        if self.is_vehicle(id) && horse.can_gallop {
            let horse = abstract_horse_mut(self.mob_mut(id).expect("an abstract horse"));
            horse.gallop_sound_counter += 1;
            let gallop_sound_counter = horse.gallop_sound_counter;
            if gallop_sound_counter > 5 && gallop_sound_counter % 3 == 0 {
                self.play_gallop_sound(id, sound_type);
            } else if gallop_sound_counter <= 5 {
                self.entity_play_sound(id, sound_event("minecraft:entity.horse.step_wood"), sound_type.volume * 0.15, sound_type.pitch);
            }
        } else if is_wood_sound_type(sound_type) {
            self.entity_play_sound(id, sound_event("minecraft:entity.horse.step_wood"), sound_type.volume * 0.15, sound_type.pitch);
        } else {
            let sound = if self.is_baby(id) { "minecraft:entity.baby_horse.step" } else { "minecraft:entity.horse.step" };
            self.entity_play_sound(id, sound_event(sound), sound_type.volume * 0.15, sound_type.pitch);
        }
    }

    fn play_gallop_sound(&mut self, id: EntityId, sound_type: &wafer_data::SoundType) {
        self.entity_play_sound(id, sound_event("minecraft:entity.horse.gallop"), sound_type.volume * 0.15, sound_type.pitch);
        if self.mob(id).is_some_and(|mob| matches!(mob.kind, MobKind::Horse(_))) {
            self.horse_play_gallop_sound(id, sound_type);
        }
    }

    pub fn abstract_horse_open_custom_inventory_screen(&mut self, id: EntityId, player: EntityId) {
        if self.is_camel(id) {
            self.camel_open_custom_inventory_screen(id, player);
            return;
        }
        if (!self.is_vehicle(id) || self.has_passenger(id, player)) && self.is_tamed(id) {
            self.open_horse_inventory(player, id);
        }
    }

    pub fn fed_food(&mut self, id: EntityId, player: EntityId, hand: InteractionHand, item_stack: &mut ItemStack) -> InteractionResult {
        let ate_food = if self.is_llama(id) {
            self.llama_handle_eating(id, player, item_stack)
        } else if self.is_camel(id) {
            self.camel_handle_eating(id, player, item_stack)
        } else {
            self.handle_eating(id, player, item_stack)
        };
        if ate_food {
            item_stack.consume(1, self, Some(player));
            self.player_mut(player).update_item_in_hand(hand, item_stack);
        }
        if !ate_food { InteractionResult::Pass } else { InteractionResult::SUCCESS_SERVER }
    }

    pub fn handle_eating(&mut self, id: EntityId, player: EntityId, item_stack: &ItemStack) -> bool {
        let mut item_used = false;
        let mut heal = 0.0f32;
        let mut age_up = 0;
        let mut temper = 0;
        let tamed = self.is_tamed(id);
        if item_stack.is(items::WHEAT) {
            heal = 2.0;
            age_up = 20;
            temper = 3;
        } else if item_stack.is(items::SUGAR) {
            heal = 1.0;
            age_up = 30;
            temper = 3;
        } else if item_stack.is(items::HAY_BLOCK) {
            heal = 20.0;
            age_up = 180;
        } else if item_stack.is(items::APPLE) {
            heal = 3.0;
            age_up = 60;
            temper = 3;
        } else if item_stack.is(items::RED_MUSHROOM) {
            heal = 3.0;
            age_up = 0;
            temper = 3;
        } else if item_stack.is(items::CARROT) {
            heal = 3.0;
            age_up = 60;
            temper = 3;
        } else if item_stack.is(items::GOLDEN_CARROT) {
            heal = 4.0;
            age_up = 60;
            temper = 5;
            if tamed && self.ageable_age(id) == 0 && !self.is_in_love(id) {
                item_used = true;
                self.set_in_love(id, Some(player));
            }
        } else if item_stack.is(items::GOLDEN_APPLE) || item_stack.is(items::ENCHANTED_GOLDEN_APPLE) {
            heal = 10.0;
            age_up = 240;
            temper = 10;
            if tamed && self.ageable_age(id) == 0 && !self.is_in_love(id) {
                item_used = true;
                self.set_in_love(id, Some(player));
            }
        }
        if self.health(id) < self.max_health(id) && heal > 0.0 {
            self.heal(id, heal);
            item_used = true;
        }
        if self.is_baby(id) && age_up > 0 && !self.mob(id).expect("an abstract horse").is_age_locked() {
            let e = self.entity_mut(id);
            e.random_x(1.0);
            e.random_y();
            e.random_z(1.0);
            self.age_up(id, age_up, false);
            item_used = true;
        }
        if temper > 0 && (item_used || !self.is_tamed(id)) && self.get_temper(id) < get_max_temper() {
            self.modify_temper(id, temper);
            item_used = true;
        }
        if item_used {
            self.eating(id);
            self.entity_game_event(id, crate::world::level::gameevent::game_event::EAT);
        }
        item_used
    }

    pub fn ageable_age(&self, id: EntityId) -> i32 {
        self.mob(id).and_then(Mob::ageable).map_or(0, |ageable| ageable.age)
    }

    pub fn do_player_ride(&mut self, id: EntityId, player: EntityId) {
        self.set_eating(id, false);
        self.clear_standing(id);
        let (y_rot, x_rot) = (self.entity(id).y_rot, self.entity(id).x_rot);
        let p = self.entity_mut(player);
        p.set_y_rot(y_rot);
        p.set_x_rot(x_rot);
        self.start_riding(player, id, false, true);
    }

    pub fn abstract_horse_is_immobile(&self, id: EntityId) -> bool {
        self.is_dead_or_dying(id) && self.is_vehicle(id) && self.is_saddled(id) || self.is_eating(id) || self.is_standing(id)
    }

    fn move_tail(&mut self, id: EntityId) {
        abstract_horse_mut(self.mob_mut(id).expect("an abstract horse")).tail_counter = 1;
    }

    pub fn abstract_horse_drop_equipment(&mut self, id: EntityId) {
        let items = abstract_horse(self.mob(id).expect("an abstract horse")).inventory.items.clone();
        for item_stack in items {
            if !item_stack.is_empty() && !crate::world::item::enchantment::enchantment_helper::has_prevent_equipment_drop(&item_stack) {
                self.spawn_at_location(id, item_stack, 0.0);
            }
        }
    }

    pub fn abstract_horse_ai_step(&mut self, id: EntityId) {
        if self.entity_mut(id).random.next_int_bounded(200) == 0 {
            self.move_tail(id);
        }
        self.animal_ai_step(id);
        if self.mob(id).is_none() || !self.is_alive(id) {
            return;
        }
        if self.entity_mut(id).random.next_int_bounded(900) == 0 && self.living(id).death_time == 0 {
            self.heal(id, 1.0);
        }
        if self.can_eat_grass(id) {
            if !self.is_eating(id) && !self.is_vehicle(id) && self.entity_mut(id).random.next_int_bounded(300) == 0 && {
                let below = self.entity(id).block_position().below();
                self.block_state(below).is_block(blocks::GRASS_BLOCK)
            } {
                self.set_eating(id, true);
            }
            if self.is_eating(id) {
                let horse = abstract_horse_mut(self.mob_mut(id).expect("an abstract horse"));
                horse.eating_counter += 1;
                if horse.eating_counter > 50 {
                    horse.eating_counter = 0;
                    self.set_eating(id, false);
                }
            }
        }
        self.follow_mommy(id);
    }

    fn follow_mommy(&mut self, id: EntityId) {
        if self.is_llama(id) && !self.llama_follow_mommy_allowed(id) {
            return;
        }
        if !(is_bred(self.entity(id)) && self.is_baby(id) && !self.is_eating(id)) {
            return;
        }
        let e = self.entity(id);
        let (x, y, z) = (e.x(), e.y(), e.z());
        let mut horses = self.get_entities(None, &e.bounding_box().inflate_all(16.0));
        horses.retain(|&horse| self.is_abstract_horse(horse));
        let mommy_targeting = TargetingConditions::for_non_combat().range(16.0).ignore_line_of_sight().selector(&parent_horse_selector);
        let mommy = self.get_nearest_entity(&horses, &mommy_targeting, Some(id), x, y, z);
        if let Some(mommy) = mommy
            && self.entity(id).distance_to_sqr(self.entity(mommy)) > 4.0
        {
            self.get_navigation(id, |navigation, level| navigation.create_path_to_entity(level, id, mommy, 0));
        }
    }

    pub fn abstract_horse_tick(&mut self, id: EntityId) {
        self.mob_tick(id);
        let Some(mob) = self.mob_mut(id) else { return };
        let horse = abstract_horse_mut(mob);
        let mut close_mouth = false;
        if horse.mouth_counter > 0 {
            horse.mouth_counter += 1;
            if horse.mouth_counter > 30 {
                horse.mouth_counter = 0;
                close_mouth = true;
            }
        }
        if close_mouth {
            set_flag(&mut mob.entity, FLAG_OPEN_MOUTH, false);
        }
        let horse = abstract_horse_mut(mob);
        if horse.stand_counter > 0 {
            horse.stand_counter -= 1;
            if horse.stand_counter <= 0 {
                self.clear_standing(id);
            }
        }
        let mob = self.mob_mut(id).expect("an abstract horse");
        let eating = is_eating(&mob.entity);
        let standing = is_standing(&mob.entity);
        let open_mouth = get_flag(&mob.entity, FLAG_OPEN_MOUTH);
        let horse = abstract_horse_mut(mob);
        if horse.tail_counter > 0 {
            horse.tail_counter += 1;
            if horse.tail_counter > 8 {
                horse.tail_counter = 0;
            }
        }
        if horse.sprint_counter > 0 {
            horse.sprint_counter += 1;
            if horse.sprint_counter > 300 {
                horse.sprint_counter = 0;
            }
        }
        horse.eat_anim_o = horse.eat_anim;
        if eating {
            horse.eat_anim += (1.0 - horse.eat_anim) * 0.4 + 0.05;
            if horse.eat_anim > 1.0 {
                horse.eat_anim = 1.0;
            }
        } else {
            horse.eat_anim += (0.0 - horse.eat_anim) * 0.4 - 0.05;
            if horse.eat_anim < 0.0 {
                horse.eat_anim = 0.0;
            }
        }
        horse.stand_anim_o = horse.stand_anim;
        if standing {
            horse.eat_anim = 0.0;
            horse.eat_anim_o = horse.eat_anim;
            horse.stand_anim += (1.0 - horse.stand_anim) * 0.4 + 0.05;
            if horse.stand_anim > 1.0 {
                horse.stand_anim = 1.0;
            }
        } else {
            horse.allow_stand_sliding = false;
            horse.stand_anim += (0.8 * horse.stand_anim * horse.stand_anim * horse.stand_anim - horse.stand_anim) * 0.6 - 0.05;
            if horse.stand_anim < 0.0 {
                horse.stand_anim = 0.0;
            }
        }
        horse.mouth_anim_o = horse.mouth_anim;
        if open_mouth {
            horse.mouth_anim += (1.0 - horse.mouth_anim) * 0.7 + 0.05;
            if horse.mouth_anim > 1.0 {
                horse.mouth_anim = 1.0;
            }
        } else {
            horse.mouth_anim += (0.0 - horse.mouth_anim) * 0.7 - 0.05;
            if horse.mouth_anim < 0.0 {
                horse.mouth_anim = 0.0;
            }
        }
    }

    pub fn abstract_horse_mob_interact(&mut self, id: EntityId, player: EntityId, hand: InteractionHand) -> InteractionResult {
        if self.is_vehicle(id) || self.is_baby(id) {
            return self.animal_mob_interact(id, player, hand);
        }
        if self.is_tamed(id) && self.player(player).is_secondary_use_active() {
            self.open_custom_inventory_screen(id, player);
            return InteractionResult::SUCCESS;
        }
        let mut item_stack = self.player(player).item_in_hand(hand).unwrap_or_else(ItemStack::empty);
        if !item_stack.is_empty() {
            let interaction_result = crate::world::item::interact_living_entity(self, &mut item_stack, player, id, hand);
            self.player_mut(player).update_item_in_hand(hand, &item_stack);
            if interaction_result.consumes_action() {
                return interaction_result;
            }
            if self.is_equippable_in_slot(id, &item_stack, EquipmentSlot::Body) && !self.is_wearing_body_armor(id) {
                self.equip_body_armor(id, player, &mut item_stack);
                self.player_mut(player).update_item_in_hand(hand, &item_stack);
                return InteractionResult::SUCCESS;
            }
        }
        self.do_player_ride(id, player);
        InteractionResult::SUCCESS
    }

    fn open_mouth(&mut self, id: EntityId) {
        let mob = self.mob_mut(id).expect("an abstract horse");
        abstract_horse_mut(mob).mouth_counter = 1;
        set_flag(&mut mob.entity, FLAG_OPEN_MOUTH, true);
    }

    pub fn set_standing(&mut self, id: EntityId, ticks: i32) {
        self.set_eating(id, false);
        let mob = self.mob_mut(id).expect("an abstract horse");
        set_flag(&mut mob.entity, FLAG_STANDING, true);
        abstract_horse_mut(mob).stand_counter = ticks;
    }

    pub fn clear_standing(&mut self, id: EntityId) {
        let mob = self.mob_mut(id).expect("an abstract horse");
        set_flag(&mut mob.entity, FLAG_STANDING, false);
        abstract_horse_mut(mob).stand_counter = 0;
    }

    pub fn get_ambient_stand_sound(&mut self, id: EntityId) -> Option<&'static str> {
        self.mob(id).expect("an abstract horse").get_ambient_sound()
    }

    pub fn stand_if_possible(&mut self, id: EntityId) {
        if self.can_perform_rearing(id) {
            self.set_standing(id, 20);
        }
    }

    pub fn make_mad(&mut self, id: EntityId) {
        if !self.is_standing(id) {
            self.stand_if_possible(id);
            if let Some(angry_sound) = self.get_angry_sound(id) {
                self.make_sound(id, sound_event(angry_sound));
            }
        }
    }

    pub fn tame_with_name(&mut self, id: EntityId, player: EntityId) -> bool {
        self.set_horse_owner(id, Some(player));
        set_tamed(self.entity_mut(id), true);
        if self.is_player(player) {
            crate::advancements::triggers::criteria_triggers::TAME_ANIMAL.trigger(self, player, id);
        }
        self.broadcast_entity_event(id, crate::world::entity::entity_event::TAMING_SUCCEEDED);
        true
    }

    pub fn abstract_horse_tick_ridden(&mut self, id: EntityId, controller: EntityId, ridden_input: Vec3) {
        self.living_entity_tick_ridden(id, controller, ridden_input);
        let (rotation_x, rotation_y) = self.get_ridden_rotation(id, controller);
        let e = self.entity_mut(id);
        e.set_rot(rotation_y, rotation_x);
        let y_rot = e.y_rot;
        e.y_rot_o = y_rot;
        e.y_head_rot = y_rot;
        self.living_mut(id).y_body_rot = y_rot;
        if self.is_local_instance_authoritative(id) {
            if ridden_input.z <= 0.0 {
                abstract_horse_mut(self.mob_mut(id).expect("an abstract horse")).gallop_sound_counter = 0;
            }
            if self.entity(id).on_ground {
                let pending = abstract_horse(self.mob(id).expect("an abstract horse")).player_jump_pending_scale;
                if pending > 0.0 && !self.living(id).jumping {
                    self.execute_riders_jump(id, pending, ridden_input);
                }
                abstract_horse_mut(self.mob_mut(id).expect("an abstract horse")).player_jump_pending_scale = 0.0;
            }
        }
    }

    fn get_ridden_rotation(&self, id: EntityId, controller: EntityId) -> (f32, f32) {
        if self.is_camel(id) {
            return self.camel_get_ridden_rotation(id, controller);
        }
        let c = self.entity(controller);
        (c.x_rot * 0.5, c.y_rot)
    }

    pub fn abstract_horse_add_passenger(&mut self, id: EntityId, passenger: EntityId) {
        self.entity_add_passenger(id, passenger);
        let view_y_rot = self.living(id).y_head_rot_o;
        let view_x_rot = self.entity(id).x_rot_o;
        self.entity_mut(passenger).abs_snap_rotation_to(view_y_rot, view_x_rot);
    }

    pub fn abstract_horse_get_ridden_input(&self, id: EntityId, controller: EntityId, _self_input: Vec3) -> Vec3 {
        let horse = abstract_horse(self.mob(id).expect("an abstract horse"));
        if self.entity(id).on_ground && horse.player_jump_pending_scale == 0.0 && self.is_standing(id) && !horse.allow_stand_sliding {
            return Vec3::ZERO;
        }
        let c = self.living(controller);
        let sideways = c.xxa * SIDEWAYS_MOVE_SPEED_FACTOR;
        let mut forward = c.zza;
        if forward <= 0.0 {
            forward *= BACKWARDS_MOVE_SPEED_FACTOR;
        }
        Vec3::new(sideways as f64, 0.0, forward as f64)
    }

    pub fn abstract_horse_get_ridden_speed(&self, id: EntityId, _controller: EntityId) -> f32 {
        self.attribute_value(id, Attribute::MovementSpeed) as f32
    }

    fn execute_riders_jump(&mut self, id: EntityId, amount: f32, input: Vec3) {
        if self.is_camel(id) {
            self.camel_execute_riders_jump(id, amount);
            return;
        }
        let impulse = self.get_jump_power_with_multiplier(id, amount) as f64;
        let e = self.entity_mut(id);
        let movement = e.delta_movement;
        e.set_delta_movement(Vec3::new(movement.x, impulse, movement.z));
        e.needs_sync = true;
        if input.z > 0.0 {
            let sin = mth::sin((e.y_rot * mth::DEG_TO_RAD) as f64);
            let cos = mth::cos((e.y_rot * mth::DEG_TO_RAD) as f64);
            let delta = e.delta_movement.add((-0.4 * sin * amount) as f64, 0.0, (0.4 * cos * amount) as f64);
            e.set_delta_movement(delta);
        }
    }

    pub fn play_jump_sound(&mut self, id: EntityId) {
        if self.mob(id).is_some_and(|mob| matches!(mob.kind, MobKind::SkeletonHorse(_))) && self.skeleton_horse_play_jump_sound(id) {
            return;
        }
        let sound = match self.mob(id).expect("an abstract horse").kind {
            MobKind::Donkey(_) => "minecraft:entity.donkey.jump",
            MobKind::Mule(_) => "minecraft:entity.mule.jump",
            _ => "minecraft:entity.horse.jump",
        };
        self.entity_play_sound(id, sound_event(sound), 0.4, 1.0);
    }

    pub fn can_parent(&self, id: EntityId) -> bool {
        !self.is_vehicle(id) && !self.is_passenger(id) && self.is_tamed(id) && !self.is_baby(id) && self.health(id) >= self.max_health(id) && self.is_in_love(id)
    }

    pub fn set_offspring_attributes(&mut self, id: EntityId, partner: EntityId, baby: &mut Mob) {
        self.set_offspring_attribute(id, partner, baby, Attribute::MaxHealth, min_health() as f64, max_health() as f64);
        self.set_offspring_attribute(id, partner, baby, Attribute::JumpStrength, min_jump_strength() as f64, max_jump_strength() as f64);
        self.set_offspring_attribute(id, partner, baby, Attribute::MovementSpeed, min_movement_speed() as f64, max_movement_speed() as f64);
    }

    fn set_offspring_attribute(&mut self, id: EntityId, partner: EntityId, baby: &mut Mob, attribute: Attribute, attribute_range_min: f64, attribute_range_max: f64) {
        let parent_a_value = self.mob(id).expect("an abstract horse").living.attributes.base_value(attribute);
        let parent_b_value = self.mob(partner).expect("an ageable mob").living.attributes.base_value(attribute);
        let new_value = create_offspring_attribute(parent_a_value, parent_b_value, attribute_range_min, attribute_range_max, &mut self.entity_mut(id).random);
        baby.living.attributes.instance(attribute).expect("horses have the attribute").set_base_value(new_value);
    }

    pub fn on_player_jump(&mut self, id: EntityId, jump_amount: i32) {
        if !self.is_saddled(id) {
            return;
        }
        let jump_amount = if jump_amount < 0 {
            0
        } else {
            abstract_horse_mut(self.mob_mut(id).expect("an abstract horse")).allow_stand_sliding = true;
            self.stand_if_possible(id);
            jump_amount
        };
        abstract_horse_mut(self.mob_mut(id).expect("an abstract horse")).player_jump_pending_scale = get_player_jump_pending_scale(jump_amount);
    }

    pub fn abstract_horse_can_jump(&self, id: EntityId) -> bool {
        self.is_saddled(id)
    }

    pub fn abstract_horse_handle_start_jump(&mut self, id: EntityId, _jump_scale: i32) {
        abstract_horse_mut(self.mob_mut(id).expect("an abstract horse")).allow_stand_sliding = true;
        self.stand_if_possible(id);
        self.play_jump_sound(id);
    }

    pub fn abstract_horse_handle_stop_jump(&mut self, _id: EntityId) {}

    pub fn abstract_horse_position_rider(&mut self, id: EntityId, passenger: EntityId) {
        if self.is_living(passenger) {
            let y_body_rot = self.living(id).y_body_rot;
            self.living_mut(passenger).y_body_rot = y_body_rot;
        }
    }

    pub fn abstract_horse_get_slot(&self, id: EntityId, slot: i32) -> Option<crate::world::entity::slot_access::LevelSlot> {
        let inventory_slot = slot - INVENTORY_SLOT_OFFSET;
        let size = abstract_horse(self.mob(id).expect("an abstract horse")).inventory.items.len() as i32;
        if inventory_slot >= 0 && inventory_slot < size {
            return Some(crate::world::entity::slot_access::LevelSlot::Container(crate::world::container::ContainerRef::MountInventory(id), inventory_slot as usize));
        }
        None
    }

    pub fn abstract_horse_get_controlling_passenger(&self, id: EntityId) -> Option<EntityId> {
        if self.is_saddled(id)
            && let Some(passenger) = self.first_passenger(id).filter(|&passenger| self.is_player(passenger))
        {
            return Some(passenger);
        }
        self.mob_get_controlling_passenger(id)
    }

    fn get_dismount_location_in_direction(&mut self, id: EntityId, direction: Vec3, passenger: EntityId) -> Option<Vec3> {
        let e = self.entity(id);
        let bounding_box = e.bounding_box();
        let target_x = e.x() + direction.x;
        let target_y = bounding_box.min_y;
        let target_z = e.z() + direction.z;
        for &dismount_pose in self.dismount_poses(passenger) {
            let mut target_block_pos = BlockPos::containing(target_x, target_y, target_z);
            let dismount_jump_limit = bounding_box.max_y + 0.75;
            loop {
                let block_floor_height = dismount_helper::get_block_floor_height(self, target_block_pos);
                if target_block_pos.y as f64 + block_floor_height > dismount_jump_limit {
                    break;
                }
                if dismount_helper::is_block_floor_valid(block_floor_height) {
                    let pose_collision_box: AABB = self.local_bounds_for_pose(passenger, dismount_pose);
                    let location = Vec3::new(target_x, target_block_pos.y as f64 + block_floor_height, target_z);
                    if dismount_helper::can_dismount_to(self, passenger, &pose_collision_box.move_by(location.x, location.y, location.z)) {
                        self.set_entity_pose(passenger, dismount_pose);
                        return Some(location);
                    }
                }
                target_block_pos = target_block_pos.above();
                if !((target_block_pos.y as f64) < dismount_jump_limit) {
                    break;
                }
            }
        }
        None
    }

    pub fn abstract_horse_dismount_location_for_passenger(&mut self, id: EntityId, passenger: EntityId) -> Vec3 {
        let width = self.entity(id).dimensions.width as f64;
        let passenger_width = self.entity(passenger).dimensions.width as f64;
        let y_rot = self.entity(id).y_rot;
        let main_arm_right = self.is_main_arm_right(passenger);
        let main_hand_direction = get_collision_horizontal_escape_vector(width, passenger_width, y_rot + if main_arm_right { 90.0 } else { -90.0 });
        if let Some(main_hand_location) = self.get_dismount_location_in_direction(id, main_hand_direction, passenger) {
            return main_hand_location;
        }
        let off_hand_direction = get_collision_horizontal_escape_vector(width, passenger_width, y_rot + if !main_arm_right { 90.0 } else { -90.0 });
        self.get_dismount_location_in_direction(id, off_hand_direction, passenger).unwrap_or_else(|| self.entity(id).position())
    }

    fn is_main_arm_right(&self, id: EntityId) -> bool {
        match self.mob(id) {
            Some(mob) => !mob.is_left_handed(),
            None => matches!(self.entity(id).data.get(crate::world::entity::data::ids::PLAYER_MAIN_HAND), DataValue::HumanoidArm(1)),
        }
    }

    pub fn has_inventory_changed(&self, id: EntityId, inventory_version: u32) -> bool {
        if let Some(nautilus) = self.mob(id).and_then(Mob::abstract_nautilus) {
            return nautilus.inventory_version != inventory_version;
        }
        self.mob(id).and_then(Mob::abstract_horse).is_none_or(|horse| horse.inventory_version != inventory_version)
    }

    pub fn abstract_horse_passenger_attachment_point(&self, id: EntityId, passenger: EntityId) -> Vec3 {
        let scale = self.attribute_value(id, Attribute::Scale) as f32 * if self.is_baby(id) { 0.5 } else { 1.0 };
        let stand_anim_o = abstract_horse(self.mob(id).expect("an abstract horse")).stand_anim_o;
        let y_rot = self.entity(id).y_rot;
        let offset = Vec3::new(0.0, (0.15 * stand_anim_o * scale) as f64, (-0.7 * stand_anim_o * scale) as f64).y_rot(-y_rot * mth::DEG_TO_RAD);
        self.default_passenger_attachment_point(id, passenger).add_vec(offset)
    }
}

pub fn get_player_jump_pending_scale(jump_amount: i32) -> f32 {
    if jump_amount >= 90 { 1.0 } else { 0.4 + 0.4 * jump_amount as f32 / 90.0 }
}
