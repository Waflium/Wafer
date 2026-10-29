use wafer_core::Vec3;
use wafer_core::random::RandomSource;
use wafer_data::{item_tags, items, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use super::abstract_chested_horse::{self, AbstractChestedHorse, abstract_chested_horse_defaults};
use super::abstract_horse::{self, AbstractHorse};
use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::entity::ageable_mob::AgeableMobGroupData;
use crate::world::entity::ai::MobAi;
use crate::world::entity::ai::goal::breed_goal::BreedGoal;
use crate::world::entity::ai::goal::float_goal::FloatGoal;
use crate::world::entity::ai::goal::follow_parent_goal::FollowParentGoal;
use crate::world::entity::ai::goal::goal::Goal;
use crate::world::entity::ai::goal::llama_follow_caravan_goal::LlamaFollowCaravanGoal;
use crate::world::entity::ai::goal::look_at_player_goal::LookAtPlayerGoal;
use crate::world::entity::ai::goal::panic_goal::PanicGoal;
use crate::world::entity::ai::goal::random_look_around_goal::RandomLookAroundGoal;
use crate::world::entity::ai::goal::ranged_attack_goal::RangedAttackGoal;
use crate::world::entity::ai::goal::run_around_like_crazy_goal::RunAroundLikeCrazyGoal;
use crate::world::entity::ai::goal::target::hurt_by_target_goal::HurtByTargetGoal;
use crate::world::entity::ai::goal::target::nearest_attackable_target_goal::{NearestAttackableTargetGoal, TargetType};
use crate::world::entity::ai::goal::tempt_goal::TemptGoal;
use crate::world::entity::ai::goal::water_avoiding_random_stroll_goal::WaterAvoidingRandomStrollGoal;
use crate::world::entity::attributes::CHESTED_HORSE_ATTRIBUTES;
use crate::world::entity::data::DataValue;
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::projectile::llama_spit::LlamaSpit;
use crate::world::entity::spawn_group_data::SpawnGroupData;
use crate::world::entity::{Entity, EntityDimensions, EntityId, sound_event};
use crate::world::item::ItemStack;
use crate::world::level::server_level_accessor::ServerLevelAccessor;
use crate::world::level::storage::tag_value_input;

const MAX_STRENGTH: i32 = 5;
pub const DATA_STRENGTH_ID: usize = 20;
pub const DATA_VARIANT_ID: usize = 21;
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.9, 1.87).with_eye_height(1.7765).with_passenger_attachments(&[Vec3::new(0.0, 1.37, -0.3)]);
const BABY_PASSENGER_ATTACHMENTS: &[Vec3] = &[Vec3::new(0.0, (1.87f32 - 0.25f32) as f64, -0.3f32 as f64)];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Variant {
    Creamy,
    White,
    Brown,
    Gray,
}

impl Variant {
    pub const VALUES: [Variant; 4] = [Variant::Creamy, Variant::White, Variant::Brown, Variant::Gray];
    pub const DEFAULT: Variant = Variant::Creamy;

    pub fn id(self) -> i32 {
        self as i32
    }

    pub fn by_id(id: i32) -> Variant {
        Variant::VALUES[id.clamp(0, Variant::VALUES.len() as i32 - 1) as usize]
    }

    pub fn by_name(name: &str) -> Option<Variant> {
        Variant::VALUES.into_iter().find(|variant| variant.serialized_name() == name)
    }

    pub fn serialized_name(self) -> &'static str {
        match self {
            Variant::Creamy => "creamy",
            Variant::White => "white",
            Variant::Brown => "brown",
            Variant::Gray => "gray",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct LlamaGroupData {
    pub ageable_mob_group_data: AgeableMobGroupData,
    pub variant: Variant,
}

#[derive(Default)]
pub struct Llama {
    pub abstract_chested_horse: AbstractChestedHorse,
    pub did_spit: bool,
    pub caravan_head: Option<EntityId>,
    pub caravan_tail: Option<EntityId>,
    pub despawn_delay: i32,
}

pub fn llama(mob: &Mob) -> &Llama {
    match &mob.kind {
        MobKind::Llama(llama) | MobKind::TraderLlama(llama) => llama,
        _ => unreachable!("a llama"),
    }
}

pub fn llama_mut(mob: &mut Mob) -> &mut Llama {
    match &mut mob.kind {
        MobKind::Llama(llama) | MobKind::TraderLlama(llama) => llama,
        _ => unreachable!("a llama"),
    }
}

pub fn baby_dimensions() -> EntityDimensions {
    abstract_chested_horse::baby_dimensions(DIMENSIONS, BABY_PASSENGER_ATTACHMENTS)
}

pub fn get_strength(entity: &Entity) -> i32 {
    match entity.data.get(DATA_STRENGTH_ID) {
        DataValue::Int(strength) => *strength,
        _ => 0,
    }
}

fn set_strength(entity: &mut Entity, strength: i32) {
    entity.data.set(DATA_STRENGTH_ID, DataValue::Int(strength.min(MAX_STRENGTH).max(1)));
}

fn set_random_strength(entity: &mut Entity, random: &mut (impl RandomSource + ?Sized)) {
    let max_strength = if random.next_float() < 0.04 { 5 } else { 3 };
    set_strength(entity, 1 + random.next_int_bounded(max_strength));
}

pub fn get_variant(entity: &Entity) -> Variant {
    match entity.data.get(DATA_VARIANT_ID) {
        DataValue::Int(variant) => Variant::by_id(*variant),
        _ => Variant::DEFAULT,
    }
}

pub fn set_variant(entity: &mut Entity, variant: Variant) {
    entity.data.set(DATA_VARIANT_ID, DataValue::Int(variant.id()));
}

pub fn is_food(item_stack: &ItemStack) -> bool {
    item_stack.get_item().is(&item_tags::LLAMA_FOOD)
}

pub fn get_inventory_columns(mob: &Mob) -> i32 {
    if abstract_chested_horse::has_chest(&mob.entity) { get_strength(&mob.entity) } else { 0 }
}

pub fn get_max_temper() -> i32 {
    30
}

pub fn get_angry_sound() -> &'static str {
    "minecraft:entity.llama.angry"
}

pub fn get_ambient_sound() -> &'static str {
    "minecraft:entity.llama.ambient"
}

pub fn get_hurt_sound() -> &'static str {
    "minecraft:entity.llama.hurt"
}

pub fn get_death_sound() -> &'static str {
    "minecraft:entity.llama.death"
}

pub fn get_eating_sound() -> &'static str {
    "minecraft:entity.llama.eat"
}

pub fn get_step_sound() -> &'static str {
    "minecraft:entity.llama.step"
}

pub fn follow_leash_speed() -> f64 {
    2.0
}

pub fn finalize_spawn(level: &mut (impl ServerLevelAccessor + ?Sized), mob: &mut Mob, group_data: Option<SpawnGroupData>) -> Option<SpawnGroupData> {
    set_random_strength(&mut mob.entity, level.get_random());
    let (variant, group_data) = match group_data {
        Some(SpawnGroupData::Llama(llama_group_data)) => (llama_group_data.variant, group_data),
        _ => {
            let variant = Variant::VALUES[level.get_random().next_int_bounded(Variant::VALUES.len() as i32) as usize];
            (variant, Some(SpawnGroupData::Llama(LlamaGroupData { ageable_mob_group_data: AgeableMobGroupData::new(true), variant })))
        }
    };
    set_variant(&mut mob.entity, variant);
    abstract_horse::finalize_spawn(level, mob, group_data)
}

fn not_tame(_mob: EntityId, target: EntityId, level: &ServerLevel) -> bool {
    !level.is_tame(target)
}

impl Llama {
    pub fn create() -> Mob {
        Llama::create_kind("minecraft:llama", MobKind::Llama(Llama::default()))
    }

    pub fn create_kind(entity_type: &str, kind: MobKind) -> Mob {
        let mut data = abstract_chested_horse_defaults();
        data.push(DataValue::Int(0));
        data.push(DataValue::Int(0));
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, entity_type), DIMENSIONS, data);
        let mut mob = Mob::new(entity, LivingEntity::new(&CHESTED_HORSE_ATTRIBUTES), kind);
        AbstractHorse::init(&mut mob);
        let follow_range = mob.living.attributes.value(crate::world::entity::attributes::Attribute::FollowRange);
        mob.ai.navigation.set_required_path_length_detached(follow_range, 40.0);
        mob
    }

    pub fn register_goals(ai: &mut MobAi) {
        let float_goal = FloatGoal::new(&mut ai.navigation);
        let goal_selector = &mut ai.goal_selector;
        goal_selector.add_goal(0, Goal::Float(float_goal));
        goal_selector.add_goal(1, Goal::RunAroundLikeCrazy(RunAroundLikeCrazyGoal::new(1.2)));
        goal_selector.add_goal(2, Goal::LlamaFollowCaravan(LlamaFollowCaravanGoal::new(2.1f32 as f64)));
        goal_selector.add_goal(3, Goal::RangedAttack(RangedAttackGoal::new(1.25, 40, 20.0)));
        goal_selector.add_goal(3, Goal::Panic(PanicGoal::new(1.2)));
        goal_selector.add_goal(4, Goal::Breed(BreedGoal::new(1.0)));
        goal_selector.add_goal(5, Goal::Tempt(TemptGoal::new(1.25, |i| i.get_item().is(&item_tags::LLAMA_TEMPT_ITEMS), false)));
        goal_selector.add_goal(6, Goal::FollowParent(FollowParentGoal::new(1.0)));
        goal_selector.add_goal(7, Goal::WaterAvoidingRandomStroll(WaterAvoidingRandomStrollGoal::new(0.7)));
        goal_selector.add_goal(8, Goal::LookAtPlayer(LookAtPlayerGoal::new(6.0)));
        goal_selector.add_goal(9, Goal::RandomLookAround(RandomLookAroundGoal::default()));
        let target_selector = &mut ai.target_selector;
        target_selector.add_goal(1, Goal::LlamaHurtByTarget(HurtByTargetGoal::new(&[])));
        let mut attack_wolf = NearestAttackableTargetGoal::with(TargetType::Wolf, 16, false, true, Some(not_tame));
        attack_wolf.target_goal.follow_distance_scale = 0.25;
        target_selector.add_goal(2, Goal::NearestAttackableTarget(attack_wolf));
    }

    pub fn add_additional_save_data(mob: &Mob, out: &mut Vec<(String, Nbt)>) {
        AbstractChestedHorse::add_additional_save_data(mob, out);
        out.push(("Variant".into(), Nbt::Int(get_variant(&mob.entity).id())));
        out.push(("Strength".into(), Nbt::Int(get_strength(&mob.entity))));
    }

    pub fn read_additional_save_data(mob: &mut Mob, input: &Nbt) {
        set_strength(&mut mob.entity, tag_value_input::get_i64(input, "Strength").map_or(0, |v| v as i32));
        AbstractChestedHorse::read_additional_save_data(mob, input);
        let variant = tag_value_input::read_or_error(input, "Variant", crate::util::codec::int).map_or(Variant::DEFAULT, Variant::by_id);
        set_variant(&mut mob.entity, variant);
    }

    pub fn in_caravan(mob: &Mob) -> bool {
        llama(mob).caravan_head.is_some()
    }

    pub fn has_caravan_tail(mob: &Mob) -> bool {
        llama(mob).caravan_tail.is_some()
    }
}

impl ServerLevel {
    pub fn is_llama(&self, id: EntityId) -> bool {
        matches!(self.mob(id).map(|mob| &mob.kind), Some(MobKind::Llama(_) | MobKind::TraderLlama(_)))
    }

    pub fn in_caravan(&self, id: EntityId) -> bool {
        Llama::in_caravan(self.mob(id).expect("a llama"))
    }

    pub fn has_caravan_tail(&self, id: EntityId) -> bool {
        Llama::has_caravan_tail(self.mob(id).expect("a llama"))
    }

    pub fn get_caravan_head(&self, id: EntityId) -> Option<EntityId> {
        llama(self.mob(id).expect("a llama")).caravan_head
    }

    pub fn leave_caravan(&mut self, id: EntityId) {
        if let Some(head) = self.get_caravan_head(id)
            && let Some(head_mob) = self.mob_mut(head)
            && matches!(head_mob.kind, MobKind::Llama(_) | MobKind::TraderLlama(_))
        {
            llama_mut(head_mob).caravan_tail = None;
        }
        llama_mut(self.mob_mut(id).expect("a llama")).caravan_head = None;
    }

    pub fn join_caravan(&mut self, id: EntityId, tail: EntityId) {
        llama_mut(self.mob_mut(id).expect("a llama")).caravan_head = Some(tail);
        llama_mut(self.mob_mut(tail).expect("a llama")).caravan_tail = Some(id);
    }

    pub fn llama_can_mate(&self, id: EntityId, partner: EntityId) -> bool {
        partner != id && self.is_llama(partner) && self.can_parent(id) && self.can_parent(partner)
    }

    pub fn llama_get_breed_offspring(&mut self, id: EntityId, partner: EntityId) -> Option<Box<Mob>> {
        let mut baby = Box::new(self.make_new_llama(id));
        self.set_offspring_attributes(id, partner, &mut baby);
        let strength = get_strength(self.entity(id)).max(get_strength(self.entity(partner)));
        let random = &mut self.entity_mut(id).random;
        let mut baby_strength = random.next_int_bounded(strength) + 1;
        if random.next_float() < 0.03 {
            baby_strength += 1;
        }
        set_strength(&mut baby.entity, baby_strength);
        let variant = if self.entity_mut(id).random.next_boolean() { get_variant(self.entity(id)) } else { get_variant(self.entity(partner)) };
        set_variant(&mut baby.entity, variant);
        Some(baby)
    }

    fn make_new_llama(&self, id: EntityId) -> Mob {
        if matches!(self.mob(id).map(|mob| &mob.kind), Some(MobKind::TraderLlama(_))) {
            let mut baby = super::trader_llama::TraderLlama::create();
            baby.persistence_required = true;
            baby
        } else {
            Llama::create()
        }
    }

    pub fn llama_handle_eating(&mut self, id: EntityId, player: EntityId, item_stack: &ItemStack) -> bool {
        let mut age_up = 0;
        let mut temper = 0;
        let mut heal = 0.0f32;
        let mut item_used = false;
        if item_stack.is(items::WHEAT) {
            age_up = 10;
            temper = 3;
            heal = 2.0;
        } else if item_stack.is(items::HAY_BLOCK) {
            age_up = 90;
            temper = 6;
            heal = 10.0;
            if self.is_tamed(id) && self.mob(id).and_then(Mob::ageable).is_some_and(|ageable| ageable.age == 0) && self.can_fall_in_love(id) {
                item_used = true;
                self.set_in_love(id, Some(player));
            }
        }
        if self.health(id) < self.max_health(id) && heal > 0.0 {
            self.heal(id, heal);
            item_used = true;
        }
        if self.is_baby(id) && age_up > 0 && !self.mob(id).expect("a llama").is_age_locked() {
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
        if item_used && !self.entity(id).is_silent() {
            let source = self.sound_source(id);
            let e = self.entity_mut(id);
            let (x, y, z) = (e.x(), e.y(), e.z());
            let pitch = 1.0 + (e.random.next_float() - e.random.next_float()) * 0.2;
            self.play_sound_at(None, x, y, z, sound_event(get_eating_sound()), source, 1.0, pitch);
        }
        item_used
    }

    pub fn llama_is_immobile(&self, id: EntityId) -> bool {
        self.is_dead_or_dying(id) || self.is_eating(id)
    }

    pub fn llama_play_chest_equips_sound(&mut self, id: EntityId) {
        let random = &mut self.entity_mut(id).random;
        let pitch = (random.next_float() - random.next_float()) * 0.2 + 1.0;
        self.entity_play_sound(id, sound_event("minecraft:entity.llama.chest"), 1.0, pitch);
    }

    pub fn llama_cause_fall_damage(&mut self, id: EntityId, fall_distance: f64, damage_modifier: f32, damage_source: DamageSource) -> bool {
        let dmg = self.calculate_fall_damage(id, fall_distance, damage_modifier);
        if dmg <= 0 {
            return false;
        }
        if fall_distance >= 6.0 {
            self.hurt(id, damage_source.clone(), dmg as f32);
            if self.entity_ref(id).is_some() {
                self.propagate_fall_to_passengers(id, fall_distance, damage_modifier, damage_source);
            }
        }
        if self.entity_ref(id).is_some() {
            self.play_block_fall_sound(id);
        }
        true
    }

    pub fn llama_perform_ranged_attack(&mut self, id: EntityId, target: EntityId) {
        self.spit(id, target);
    }

    fn spit(&mut self, id: EntityId, target: EntityId) {
        let spit = LlamaSpit::from_owner(self, id);
        let (t, e) = (self.entity(target), self.entity(id));
        let xd = t.x() - e.x();
        let yd = t.get_y(0.3333333333333333) - spit.entity.y();
        let zd = t.z() - e.z();
        let yo = (xd * xd + zd * zd).sqrt() * 0.2f32 as f64;
        let uncertainty = self.llama_ranged_attack_uncertainty();
        self.spawn_projectile_using_shoot(spit, &ItemStack::empty(), xd, yd + yo, zd, 1.5, uncertainty);
        if !self.entity(id).is_silent() {
            let source = self.sound_source(id);
            let e = self.entity_mut(id);
            let (x, y, z) = (e.x(), e.y(), e.z());
            let pitch = 1.0 + (e.random.next_float() - e.random.next_float()) * 0.2;
            self.play_sound_at(None, x, y, z, sound_event("minecraft:entity.llama.spit"), source, 1.0, pitch);
        }
        llama_mut(self.mob_mut(id).expect("a llama")).did_spit = true;
    }

    pub fn llama_ranged_attack_uncertainty(&self) -> f32 {
        10.0
    }

    pub fn llama_hurt_by_target_can_continue_to_use(&mut self, goal: &mut HurtByTargetGoal, mob: EntityId) -> bool {
        let llama = llama_mut(self.mob_mut(mob).expect("a llama"));
        if llama.did_spit {
            llama.did_spit = false;
            return false;
        }
        goal.target_goal.can_continue_to_use(self, mob)
    }

    pub fn llama_follow_mommy_allowed(&self, id: EntityId) -> bool {
        !self.in_caravan(id) && self.is_baby(id)
    }
}
