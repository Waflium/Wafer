use wafer_core::random::RandomSource;
use wafer_data::{damage_type_tags, entity_type_tags, item_tags, items, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use super::armadillo_ai;
use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::entity::ai::brain::{self, Brain, Provider};
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::sensing::sensor_type::SensorType;
use crate::world::entity::animal::Animal;
use crate::world::entity::attributes::ARMADILLO_ATTRIBUTES;
use crate::world::entity::data::{DataValue, ageable_mob_defaults};
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::{Entity, EntityDimensions, EntityId, sound_event};
use crate::world::item::ItemStack;
use crate::world::level::block::entity::int_value;
use crate::world::{InteractionHand, InteractionResult};

pub const BABY_SCALE: f32 = 0.6;
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.7, 0.65).with_eye_height(0.26);
pub const DATA_ARMADILLO_STATE: usize = 18;
const ARMADILLO_SHED: &str = "minecraft:gameplay/armadillo_shed";
const ARMADILLO_BRUSH: &str = "minecraft:brush/armadillo";

pub static BRAIN_PROVIDER: Provider = brain::provider(
    &[SensorType::NearestLivingEntities, SensorType::HurtBy, SensorType::FoodTemptations, SensorType::NearestAdult, SensorType::ArmadilloScareDetected],
    |_| armadillo_ai::get_activities(),
);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ArmadilloState {
    Idle,
    Rolling,
    Scared,
    Unrolling,
}

impl ArmadilloState {
    pub const VALUES: [ArmadilloState; 4] = [ArmadilloState::Idle, ArmadilloState::Rolling, ArmadilloState::Scared, ArmadilloState::Unrolling];

    pub fn should_hide_in_shell(self, ticks_in_state: i64) -> bool {
        match self {
            ArmadilloState::Idle => false,
            ArmadilloState::Rolling => ticks_in_state > 5,
            ArmadilloState::Scared => true,
            ArmadilloState::Unrolling => ticks_in_state < 26,
        }
    }

    pub fn get_serialized_name(self) -> &'static str {
        match self {
            ArmadilloState::Idle => "idle",
            ArmadilloState::Rolling => "rolling",
            ArmadilloState::Scared => "scared",
            ArmadilloState::Unrolling => "unrolling",
        }
    }

    pub fn by_name(name: &str) -> Option<ArmadilloState> {
        ArmadilloState::VALUES.into_iter().find(|state| state.get_serialized_name() == name)
    }

    fn id(self) -> i32 {
        self as i32
    }

    fn by_id(id: i32) -> ArmadilloState {
        ArmadilloState::VALUES.get(id as usize).copied().unwrap_or(ArmadilloState::Idle)
    }

    pub fn is_threatened(self) -> bool {
        self != ArmadilloState::Idle
    }

    pub fn animation_duration(self) -> i32 {
        match self {
            ArmadilloState::Idle => 0,
            ArmadilloState::Rolling => 10,
            ArmadilloState::Scared => 50,
            ArmadilloState::Unrolling => 30,
        }
    }
}

pub struct Armadillo {
    pub animal: Animal,
    pub(super) in_state_ticks: i64,
    pub(super) scute_time: i32,
}

pub fn get_state(entity: &Entity) -> ArmadilloState {
    match entity.data.get(DATA_ARMADILLO_STATE) {
        DataValue::ArmadilloState(id) => ArmadilloState::by_id(*id),
        _ => ArmadilloState::Idle,
    }
}

pub fn is_scared(entity: &Entity) -> bool {
    get_state(entity) != ArmadilloState::Idle
}

pub fn is_food(item_stack: &ItemStack) -> bool {
    item_stack.get_item().is(&item_tags::ARMADILLO_FOOD)
}

pub fn get_default_dimensions(baby: bool) -> EntityDimensions {
    if baby { DIMENSIONS.scale(BABY_SCALE).with_eye_height(0.21875) } else { DIMENSIONS }
}

pub fn get_ambient_sound(entity: &Entity) -> Option<&'static str> {
    if is_scared(entity) { None } else { Some("minecraft:entity.armadillo.ambient") }
}

pub fn get_death_sound() -> &'static str {
    "minecraft:entity.armadillo.death"
}

pub fn get_hurt_sound(entity: &Entity) -> &'static str {
    if is_scared(entity) { "minecraft:entity.armadillo.hurt_reduced" } else { "minecraft:entity.armadillo.hurt" }
}

pub fn get_step_sound() -> &'static str {
    "minecraft:entity.armadillo.step"
}

pub fn get_max_head_y_rot(entity: &Entity) -> i32 {
    if is_scared(entity) { 0 } else { 32 }
}

fn armadillo(mob: &Mob) -> &Armadillo {
    match &mob.kind {
        MobKind::Armadillo(armadillo) => armadillo,
        _ => unreachable!("an armadillo"),
    }
}

fn armadillo_mut(mob: &mut Mob) -> &mut Armadillo {
    match &mut mob.kind {
        MobKind::Armadillo(armadillo) => armadillo,
        _ => unreachable!("an armadillo"),
    }
}

fn pick_next_scute_drop_time(random: &mut impl RandomSource) -> i32 {
    random.next_int_bounded(20 * 60 * 5) + 20 * 60 * 5
}

impl Armadillo {
    pub fn create() -> Mob {
        let mut data = ageable_mob_defaults();
        data.push(DataValue::ArmadilloState(ArmadilloState::Idle.id()));
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:armadillo"), DIMENSIONS, data);
        let armadillo = Armadillo { animal: Animal::default(), in_state_ticks: 0, scute_time: 0 };
        let mut mob = Mob::new(entity, LivingEntity::new(&ARMADILLO_ATTRIBUTES), MobKind::Armadillo(armadillo));
        Animal::init(&mut mob);
        mob.ai.navigation.set_can_float(true);
        let scute_time = pick_next_scute_drop_time(&mut mob.entity.random);
        armadillo_mut(&mut mob).scute_time = scute_time;
        mob
    }

    pub fn make_brain(mob: &mut Mob, packed_brain: brain::Packed) -> Brain {
        BRAIN_PROVIDER.make_brain(mob, packed_brain)
    }

    pub fn should_hide_in_shell(mob: &Mob) -> bool {
        get_state(&mob.entity).should_hide_in_shell(armadillo(mob).in_state_ticks)
    }

    pub fn should_switch_to_scared_state(mob: &Mob) -> bool {
        get_state(&mob.entity) == ArmadilloState::Rolling && armadillo(mob).in_state_ticks > ArmadilloState::Rolling.animation_duration() as i64
    }

    pub fn switch_to_state(mob: &mut Mob, state: ArmadilloState) {
        if get_state(&mob.entity) != state {
            mob.entity.data.set(DATA_ARMADILLO_STATE, DataValue::ArmadilloState(state.id()));
            armadillo_mut(mob).in_state_ticks = 0;
        }
    }

    pub fn add_additional_save_data(mob: &Mob, out: &mut Vec<(String, Nbt)>) {
        out.push(("state".into(), Nbt::String(get_state(&mob.entity).get_serialized_name().into())));
        out.push(("scute_time".into(), Nbt::Int(armadillo(mob).scute_time)));
    }

    pub fn read_additional_save_data(mob: &mut Mob, nbt: &Nbt) {
        let state = nbt.get("state").and_then(Nbt::as_str).and_then(ArmadilloState::by_name).unwrap_or(ArmadilloState::Idle);
        Armadillo::switch_to_state(mob, state);
        if let Some(time) = nbt.get("scute_time").and_then(int_value) {
            armadillo_mut(mob).scute_time = time;
        }
    }
}

pub fn is_scared_by(level: &ServerLevel, id: EntityId, living_entity: EntityId) -> bool {
    let Some(other) = level.entity_ref(living_entity) else { return false };
    if !level.entity(id).bounding_box().inflate(7.0, 2.0, 7.0).intersects(&other.bounding_box()) {
        return false;
    }
    if entity_type_tags::UNDEAD.contains(other.entity_type as usize) {
        return true;
    }
    if level.get_last_hurt_by_mob(id) == Some(living_entity) {
        return true;
    }
    if level.is_player(living_entity) {
        return if level.is_spectator(living_entity) { false } else { other.is_sprinting() || level.is_passenger(living_entity) };
    }
    false
}

pub fn can_stay_rolled_up(level: &ServerLevel, id: EntityId) -> bool {
    !level.is_panicking(id) && !level.entity(id).is_in_liquid() && !level.is_passenger(id) && !level.is_vehicle(id)
}

impl ServerLevel {
    pub fn armadillo_get_breed_offspring(&mut self, _id: EntityId, _partner: EntityId) -> Option<Box<Mob>> {
        Some(Box::new(Armadillo::create()))
    }

    pub fn armadillo_custom_server_ai_step(&mut self, id: EntityId) {
        Brain::tick(self, id);
        if self.mob(id).is_none() {
            return;
        }
        armadillo_ai::update_activity(self.get_brain_mut(id));
        if self.is_alive(id)
            && {
                let armadillo = armadillo_mut(self.mob_mut(id).expect("an armadillo"));
                armadillo.scute_time -= 1;
                armadillo.scute_time <= 0
            }
            && self.should_drop_loot(id)
        {
            if self.drop_from_gift_loot_table(id, ARMADILLO_SHED, |level, item_stack| {
                level.spawn_at_location(id, item_stack, 0.0);
            }) {
                let random = &mut self.entity_mut(id).random;
                let pitch = (random.next_float() - random.next_float()) * 0.2 + 1.0;
                self.entity_play_sound(id, sound_event("minecraft:entity.armadillo.scute_drop"), 1.0, pitch);
            }
            let mob = self.mob_mut(id).expect("an armadillo");
            let scute_time = pick_next_scute_drop_time(&mut mob.entity.random);
            armadillo_mut(mob).scute_time = scute_time;
        }
        self.animal_custom_server_ai_step(id);
    }

    pub fn armadillo_tick(&mut self, id: EntityId) {
        self.mob_tick(id);
        if self.mob(id).is_none() {
            return;
        }
        if is_scared(self.entity(id)) {
            self.clamp_head_rotation_to_body(id);
        }
        armadillo_mut(self.mob_mut(id).expect("an armadillo")).in_state_ticks += 1;
    }

    pub fn armadillo_body_rotation_control_client_tick(&mut self, id: EntityId) {
        if !is_scared(self.entity(id)) {
            self.body_rotation_control_tick(id);
        }
    }

    pub fn armadillo_hurt_server(&mut self, id: EntityId, source: DamageSource, damage: f32) -> bool {
        let damage = if is_scared(self.entity(id)) { (damage - 1.0) / 2.0 } else { damage };
        self.living_entity_hurt_server(id, source, damage)
    }

    pub fn armadillo_actually_hurt(&mut self, id: EntityId, source: &DamageSource, dmg: f32) {
        self.animal_actually_hurt(id, source, dmg);
        if !self.mob(id).expect("an armadillo").is_no_ai() && !self.is_dead_or_dying(id) {
            if source.causing_entity.is_some_and(|entity| self.is_living(entity)) {
                self.get_brain_mut(id).set_memory_with_expiry(memory_module_type::DANGER_DETECTED_RECENTLY, true, 80);
                if can_stay_rolled_up(self, id) {
                    self.armadillo_roll_up(id);
                }
            } else if source.is(&damage_type_tags::PANIC_ENVIRONMENTAL_CAUSES) {
                self.armadillo_roll_out(id);
            }
        }
    }

    pub fn armadillo_roll_up(&mut self, id: EntityId) {
        if !is_scared(self.entity(id)) {
            self.stop_in_place(id);
            self.reset_love(id);
            self.make_sound(id, sound_event("minecraft:entity.armadillo.roll"));
            Armadillo::switch_to_state(self.mob_mut(id).expect("an armadillo"), ArmadilloState::Rolling);
        }
    }

    pub fn armadillo_roll_out(&mut self, id: EntityId) {
        if is_scared(self.entity(id)) {
            self.make_sound(id, sound_event("minecraft:entity.armadillo.unroll_finish"));
            Armadillo::switch_to_state(self.mob_mut(id).expect("an armadillo"), ArmadilloState::Idle);
        }
    }

    pub fn armadillo_mob_interact(&mut self, id: EntityId, player: EntityId, hand: InteractionHand) -> InteractionResult {
        let item_stack = self.player(player).item_in_hand(hand).unwrap_or_else(ItemStack::empty);
        if item_stack.is(items::BRUSH) && self.armadillo_brush_off_scute(id, Some(player), &item_stack) {
            self.hurt_and_break(player, hand.as_equipment_slot(), 16);
            return InteractionResult::SUCCESS;
        }
        if is_scared(self.entity(id)) { InteractionResult::Fail } else { self.animal_mob_interact(id, player, hand) }
    }

    pub fn armadillo_brush_off_scute(&mut self, id: EntityId, interacting_entity: Option<EntityId>, tool: &ItemStack) -> bool {
        if self.is_baby(id) {
            return false;
        }
        self.drop_from_entity_interact_loot_table(id, ARMADILLO_BRUSH, interacting_entity, tool, |level, item_stack| {
            level.spawn_at_location(id, item_stack, 0.0);
        });
        self.entity_play_sound(id, sound_event("minecraft:entity.armadillo.brush"), 1.0, 1.0);
        true
    }

    pub fn armadillo_can_fall_in_love(&self, id: EntityId) -> bool {
        self.animal_can_fall_in_love(id) && !is_scared(self.entity(id))
    }

    pub fn armadillo_play_eating_sound(&mut self, id: EntityId) {
        self.make_sound(id, sound_event("minecraft:entity.armadillo.eat"));
    }
}
