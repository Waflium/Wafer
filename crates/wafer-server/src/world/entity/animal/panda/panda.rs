use wafer_core::random::RandomSource;
use wafer_core::{Vec3, mth};
use wafer_data::{EquipmentSlot, blocks, damage_type_tags, game_rule, item_tags, items, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use crate::server::level::ServerLevel;
use crate::world::entity::ageable_mob::AgeableMobGroupData;
use crate::world::entity::ai::MobAi;
use crate::world::entity::ai::control::move_control::MoveControlKind;
use crate::world::entity::ai::goal::avoid_entity_goal::{AvoidClass, AvoidEntityGoal};
use crate::world::entity::ai::goal::breed_goal::BreedGoal;
use crate::world::entity::ai::goal::float_goal::FloatGoal;
use crate::world::entity::ai::goal::follow_parent_goal::FollowParentGoal;
use crate::world::entity::ai::goal::goal::{Goal, reduced_tick_delay};
use crate::world::entity::ai::goal::look_at_player_goal::LookAtPlayerGoal;
use crate::world::entity::ai::goal::melee_attack_goal::MeleeAttackGoal;
use crate::world::entity::ai::goal::panic_goal::PanicGoal;
use crate::world::entity::ai::goal::random_look_around_goal::RandomLookAroundGoal;
use crate::world::entity::ai::goal::target::hurt_by_target_goal::HurtByTargetGoal;
use crate::world::entity::ai::goal::tempt_goal::TemptGoal;
use crate::world::entity::ai::goal::water_avoiding_random_stroll_goal::WaterAvoidingRandomStrollGoal;
use crate::world::entity::ai::targeting::targeting_conditions::TargetingConditions;
use crate::world::entity::animal::Animal;
use crate::world::entity::attributes::{Attribute, PANDA_ATTRIBUTES};
use crate::world::entity::data::{DataValue, ageable_mob_defaults};
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::spawn_group_data::SpawnGroupData;
use crate::world::entity::{Entity, EntityDimensions, EntityId, LevelEntity, entity_selector, sound_event};
use crate::world::item::ItemStack;
use crate::world::level::storage::tag_value_input;
use crate::world::level::BlockGetter;
use crate::world::level::server_level_accessor::ServerLevelAccessor;
use crate::world::{InteractionHand, InteractionResult};

const UNHAPPY_COUNTER: usize = 18;
const SNEEZE_COUNTER: usize = 19;
const EAT_COUNTER: usize = 20;
const MAIN_GENE_ID: usize = 21;
const HIDDEN_GENE_ID: usize = 22;
const DATA_ID_FLAGS: usize = 23;
const BREED_TARGETING: TargetingConditions<'static> = TargetingConditions::for_non_combat().range(8.0);
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(1.3, 1.25);
pub const BABY_DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.65, 0.625).with_eye_height(0.28125).with_passenger_attachments(&[Vec3::new(0.0, 0.5625, 0.0)]);
const FLAG_SNEEZE: i8 = 2;
const FLAG_ROLL: i8 = 4;
const FLAG_SIT: i8 = 8;
const FLAG_ON_BACK: i8 = 16;
const EAT_TICK_INTERVAL: i32 = 5;
pub const TOTAL_ROLL_STEPS: i32 = 32;
const TOTAL_UNHAPPY_TIME: i32 = 32;
const PANDA_SNEEZE: &str = "minecraft:gameplay/panda_sneeze";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Gene {
    Normal,
    Lazy,
    Worried,
    Playful,
    Brown,
    Weak,
    Aggressive,
}

const GENES: [(&str, Gene); 7] = [
    ("normal", Gene::Normal),
    ("lazy", Gene::Lazy),
    ("worried", Gene::Worried),
    ("playful", Gene::Playful),
    ("brown", Gene::Brown),
    ("weak", Gene::Weak),
    ("aggressive", Gene::Aggressive),
];

impl Gene {
    pub fn get_id(self) -> i32 {
        self as i32
    }

    fn get_serialized_name(self) -> &'static str {
        GENES[self as usize].0
    }

    fn is_recessive(self) -> bool {
        matches!(self, Gene::Brown | Gene::Weak)
    }

    fn get_variant_from_genes(main_gene: Gene, hidden_gene: Gene) -> Gene {
        if main_gene.is_recessive() {
            if main_gene == hidden_gene { main_gene } else { Gene::Normal }
        } else {
            main_gene
        }
    }

    pub fn by_id(id: i32) -> Gene {
        GENES.get(id as usize).map_or(Gene::Normal, |(_, gene)| *gene)
    }

    pub fn get_random(random: &mut (impl RandomSource + ?Sized)) -> Gene {
        let next_int = random.next_int_bounded(16);
        if next_int == 0 {
            Gene::Lazy
        } else if next_int == 1 {
            Gene::Worried
        } else if next_int == 2 {
            Gene::Playful
        } else if next_int == 4 {
            Gene::Aggressive
        } else if next_int < 9 {
            Gene::Weak
        } else if next_int < 11 {
            Gene::Brown
        } else {
            Gene::Normal
        }
    }
}

#[derive(Default)]
pub struct Panda {
    pub animal: Animal,
    got_bamboo: bool,
    did_bite: bool,
    pub roll_counter: i32,
    roll_delta: Vec3,
    look_at_player_target: Option<Option<EntityId>>,
}

fn panda(mob: &Mob) -> &Panda {
    match &mob.kind {
        MobKind::Panda(panda) => panda,
        _ => unreachable!("a panda"),
    }
}

fn panda_mut(mob: &mut Mob) -> &mut Panda {
    match &mut mob.kind {
        MobKind::Panda(panda) => panda,
        _ => unreachable!("a panda"),
    }
}

fn int(entity: &Entity, id: usize) -> i32 {
    match entity.data.get(id) {
        DataValue::Int(v) => *v,
        _ => 0,
    }
}

fn byte(entity: &Entity, id: usize) -> i8 {
    match entity.data.get(id) {
        DataValue::Byte(v) => *v,
        _ => 0,
    }
}

fn get_flag(entity: &Entity, flag: i8) -> bool {
    (byte(entity, DATA_ID_FLAGS) & flag) != 0
}

fn set_flag(entity: &mut Entity, flag: i8, value: bool) {
    let current = byte(entity, DATA_ID_FLAGS);
    entity.data.set(DATA_ID_FLAGS, DataValue::Byte(if value { current | flag } else { current & !flag }));
}

pub fn get_unhappy_counter(entity: &Entity) -> i32 {
    int(entity, UNHAPPY_COUNTER)
}

fn set_unhappy_counter(entity: &mut Entity, value: i32) {
    entity.data.set(UNHAPPY_COUNTER, DataValue::Int(value));
}

pub fn is_sneezing(entity: &Entity) -> bool {
    get_flag(entity, FLAG_SNEEZE)
}

pub fn is_sitting(entity: &Entity) -> bool {
    get_flag(entity, FLAG_SIT)
}

fn sit(entity: &mut Entity, value: bool) {
    set_flag(entity, FLAG_SIT, value);
}

pub fn is_on_back(entity: &Entity) -> bool {
    get_flag(entity, FLAG_ON_BACK)
}

fn set_on_back(entity: &mut Entity, value: bool) {
    set_flag(entity, FLAG_ON_BACK, value);
}

pub fn is_eating(entity: &Entity) -> bool {
    int(entity, EAT_COUNTER) > 0
}

fn eat(entity: &mut Entity, value: bool) {
    entity.data.set(EAT_COUNTER, DataValue::Int(if value { 1 } else { 0 }));
}

fn sneeze(entity: &mut Entity, value: bool) {
    set_flag(entity, FLAG_SNEEZE, value);
    if !value {
        entity.data.set(SNEEZE_COUNTER, DataValue::Int(0));
    }
}

pub fn get_main_gene(entity: &Entity) -> Gene {
    Gene::by_id(byte(entity, MAIN_GENE_ID) as i32)
}

pub fn get_hidden_gene(entity: &Entity) -> Gene {
    Gene::by_id(byte(entity, HIDDEN_GENE_ID) as i32)
}

fn set_main_gene(entity: &mut Entity, gene: Gene) {
    entity.data.set(MAIN_GENE_ID, DataValue::Byte(gene.get_id() as i8));
}

fn set_hidden_gene(entity: &mut Entity, gene: Gene) {
    entity.data.set(HIDDEN_GENE_ID, DataValue::Byte(gene.get_id() as i8));
}

pub fn is_rolling(entity: &Entity) -> bool {
    get_flag(entity, FLAG_ROLL)
}

fn roll(entity: &mut Entity, value: bool) {
    set_flag(entity, FLAG_ROLL, value);
}

pub fn get_variant(entity: &Entity) -> Gene {
    Gene::get_variant_from_genes(get_main_gene(entity), get_hidden_gene(entity))
}

pub fn is_lazy(entity: &Entity) -> bool {
    get_variant(entity) == Gene::Lazy
}

pub fn is_worried(entity: &Entity) -> bool {
    get_variant(entity) == Gene::Worried
}

pub fn is_playful(entity: &Entity) -> bool {
    get_variant(entity) == Gene::Playful
}

pub fn is_weak(entity: &Entity) -> bool {
    get_variant(entity) == Gene::Weak
}

pub fn is_aggressive(entity: &Entity) -> bool {
    get_variant(entity) == Gene::Aggressive
}

pub fn is_food(item_stack: &ItemStack) -> bool {
    item_stack.get_item().is(&item_tags::PANDA_FOOD)
}

fn decode_gene(tag: &Nbt) -> Result<Gene, String> {
    crate::util::codec::string_enum(tag, &GENES)
}

pub fn get_hurt_sound() -> &'static str {
    "minecraft:entity.panda.hurt"
}

pub fn get_death_sound() -> &'static str {
    "minecraft:entity.panda.death"
}

pub fn get_step_sound() -> &'static str {
    "minecraft:entity.panda.step"
}

pub fn get_ambient_sound(entity: &Entity) -> &'static str {
    if is_aggressive(entity) {
        "minecraft:entity.panda.aggressive_ambient"
    } else if is_worried(entity) {
        "minecraft:entity.panda.worried_ambient"
    } else {
        "minecraft:entity.panda.ambient"
    }
}

fn set_attributes(mob: &mut Mob) {
    if is_weak(&mob.entity) {
        mob.living.attributes.instance(Attribute::MaxHealth).expect("pandas have a max health").set_base_value(10.0);
    }
    if is_lazy(&mob.entity) {
        mob.living.attributes.instance(Attribute::MovementSpeed).expect("pandas have a movement speed").set_base_value(0.07f32 as f64);
    }
}

pub fn finalize_spawn(level: &mut (impl ServerLevelAccessor + ?Sized), mob: &mut Mob, group_data: Option<SpawnGroupData>) -> Option<SpawnGroupData> {
    let random = level.get_random();
    let main_gene = Gene::get_random(random);
    set_main_gene(&mut mob.entity, main_gene);
    let hidden_gene = Gene::get_random(random);
    set_hidden_gene(&mut mob.entity, hidden_gene);
    set_attributes(mob);
    Some(group_data.unwrap_or(SpawnGroupData::AgeableMob(AgeableMobGroupData::with_chance(true, 0.2))))
}

fn can_pick_up_and_eat(level: &ServerLevel, entity: EntityId) -> bool {
    match level.find_entity(entity) {
        Some(LevelEntity::Item(item)) => item.item.get_item().is(&item_tags::PANDA_EATS_FROM_GROUND) && level.is_alive(entity) && !item.has_pickup_delay(),
        _ => false,
    }
}

pub fn can_perform_action(entity: &Entity, level: &ServerLevel) -> bool {
    !is_on_back(entity) && !is_scared(entity, level) && !is_eating(entity) && !is_rolling(entity) && !is_sitting(entity)
}

pub fn is_scared(entity: &Entity, level: &ServerLevel) -> bool {
    is_worried(entity) && level.is_thundering()
}

impl Panda {
    pub fn create() -> Mob {
        let mut data = ageable_mob_defaults();
        data.extend([DataValue::Int(0), DataValue::Int(0), DataValue::Int(0), DataValue::Byte(0), DataValue::Byte(0), DataValue::Byte(0)]);
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:panda"), DIMENSIONS, data);
        let mut mob = Mob::new(entity, LivingEntity::new(&PANDA_ATTRIBUTES), MobKind::Panda(Panda::default()));
        Animal::init(&mut mob);
        mob.ai.move_control.kind = MoveControlKind::Panda;
        if !mob.is_baby() {
            mob.can_pick_up_loot = true;
        }
        mob
    }

    pub fn register_goals(ai: &mut MobAi) {
        let float_goal = FloatGoal::new(&mut ai.navigation);
        let goal_selector = &mut ai.goal_selector;
        goal_selector.add_goal(0, Goal::Float(float_goal));
        goal_selector.add_goal(2, Goal::PandaPanic(PandaPanicGoal { panic_goal: PanicGoal::with_damage_types(2.0, |_, _| &damage_type_tags::PANIC_ENVIRONMENTAL_CAUSES) }));
        goal_selector.add_goal(2, Goal::PandaBreed(PandaBreedGoal { breed_goal: BreedGoal::new(1.0), unhappy_cooldown: 0 }));
        goal_selector.add_goal(3, Goal::PandaAttack(PandaAttackGoal { melee_attack_goal: MeleeAttackGoal::new(1.2f32 as f64, true) }));
        goal_selector.add_goal(4, Goal::Tempt(TemptGoal::new(1.0, is_food, false)));
        goal_selector.add_goal(6, Goal::PandaAvoid(PandaAvoidGoal { avoid_entity_goal: AvoidEntityGoal::of_class_with_predicate(AvoidClass::EntityType("minecraft:player"), 8.0, 2.0, 2.0, no_spectators) }));
        goal_selector.add_goal(6, Goal::PandaAvoid(PandaAvoidGoal { avoid_entity_goal: AvoidEntityGoal::of_class_with_predicate(AvoidClass::Monster, 4.0, 2.0, 2.0, no_spectators) }));
        goal_selector.add_goal(7, Goal::PandaSit(PandaSitGoal { cooldown: 0 }));
        goal_selector.add_goal(8, Goal::PandaLieOnBack(PandaLieOnBackGoal { cooldown: 0 }));
        goal_selector.add_goal(8, Goal::PandaSneeze(PandaSneezeGoal));
        goal_selector.add_goal(9, Goal::PandaLookAtPlayer(PandaLookAtPlayerGoal { look_at_player_goal: LookAtPlayerGoal::new(6.0) }));
        goal_selector.add_goal(10, Goal::RandomLookAround(RandomLookAroundGoal::default()));
        goal_selector.add_goal(12, Goal::PandaRoll(PandaRollGoal));
        goal_selector.add_goal(13, Goal::FollowParent(FollowParentGoal::new(1.25)));
        goal_selector.add_goal(14, Goal::WaterAvoidingRandomStroll(WaterAvoidingRandomStrollGoal::new(1.0)));
        ai.target_selector.add_goal(1, Goal::PandaHurtByTarget(PandaHurtByTargetGoal { hurt_by_target_goal: HurtByTargetGoal::new(&[]).set_alert_others(&[]) }));
    }

    pub fn add_additional_save_data(mob: &Mob, out: &mut Vec<(String, Nbt)>) {
        out.push(("MainGene".into(), Nbt::String(get_main_gene(&mob.entity).get_serialized_name().into())));
        out.push(("HiddenGene".into(), Nbt::String(get_hidden_gene(&mob.entity).get_serialized_name().into())));
    }

    pub fn read_additional_save_data(mob: &mut Mob, nbt: &Nbt) {
        let main_gene = tag_value_input::read_or_error(nbt, "MainGene", decode_gene).unwrap_or(Gene::Normal);
        set_main_gene(&mut mob.entity, main_gene);
        let hidden_gene = tag_value_input::read_or_error(nbt, "HiddenGene", decode_gene).unwrap_or(Gene::Normal);
        set_hidden_gene(&mut mob.entity, hidden_gene);
    }

    pub fn can_dispenser_equip_into_slot(mob: &Mob, slot: EquipmentSlot) -> bool {
        slot == EquipmentSlot::Mainhand && mob.can_pick_up_loot
    }
}

fn no_spectators(_mob: EntityId, target: EntityId, level: &ServerLevel) -> bool {
    entity_selector::no_spectators(level, target)
}

impl ServerLevel {
    fn panda_can_perform_action(&self, id: EntityId) -> bool {
        can_perform_action(self.entity(id), self)
    }

    pub fn panda_get_breed_offspring(&mut self, id: EntityId, partner: EntityId) -> Option<Box<Mob>> {
        let mut baby = Box::new(Panda::create());
        let partner = matches!(self.mob(partner).map(|mob| &mob.kind), Some(MobKind::Panda(_))).then_some(partner);
        if let Some(partner) = partner {
            self.set_gene_from_parents(&mut baby, id, Some(partner));
        }
        set_attributes(&mut baby);
        Some(baby)
    }

    fn set_gene_from_parents(&mut self, baby: &mut Mob, parent1: EntityId, parent2: Option<EntityId>) {
        match parent2 {
            None => {
                if baby.entity.random.next_boolean() {
                    let gene = self.get_one_of_genes_randomly(parent1);
                    set_main_gene(&mut baby.entity, gene);
                    let gene = Gene::get_random(&mut baby.entity.random);
                    set_hidden_gene(&mut baby.entity, gene);
                } else {
                    let gene = Gene::get_random(&mut baby.entity.random);
                    set_main_gene(&mut baby.entity, gene);
                    let gene = self.get_one_of_genes_randomly(parent1);
                    set_hidden_gene(&mut baby.entity, gene);
                }
            }
            Some(parent2) => {
                if baby.entity.random.next_boolean() {
                    let gene = self.get_one_of_genes_randomly(parent1);
                    set_main_gene(&mut baby.entity, gene);
                    let gene = self.get_one_of_genes_randomly(parent2);
                    set_hidden_gene(&mut baby.entity, gene);
                } else {
                    let gene = self.get_one_of_genes_randomly(parent2);
                    set_main_gene(&mut baby.entity, gene);
                    let gene = self.get_one_of_genes_randomly(parent1);
                    set_hidden_gene(&mut baby.entity, gene);
                }
            }
        }
        if baby.entity.random.next_int_bounded(32) == 0 {
            let gene = Gene::get_random(&mut baby.entity.random);
            set_main_gene(&mut baby.entity, gene);
        }
        if baby.entity.random.next_int_bounded(32) == 0 {
            let gene = Gene::get_random(&mut baby.entity.random);
            set_hidden_gene(&mut baby.entity, gene);
        }
    }

    fn get_one_of_genes_randomly(&mut self, id: EntityId) -> Gene {
        let e = self.entity_mut(id);
        if e.random.next_boolean() { get_main_gene(e) } else { get_hidden_gene(e) }
    }

    pub fn panda_do_hurt_target(&mut self, id: EntityId, target: EntityId) -> bool {
        if !is_aggressive(self.entity(id)) {
            panda_mut(self.mob_mut(id).expect("a panda")).did_bite = true;
        }
        self.mob_do_hurt_target(id, target)
    }

    pub fn panda_play_attack_sound(&mut self, id: EntityId) {
        self.entity_play_sound(id, sound_event("minecraft:entity.panda.bite"), 1.0, 1.0);
    }

    pub fn panda_tick(&mut self, id: EntityId) {
        self.mob_tick(id);
        if self.mob(id).is_none_or(|mob| !matches!(mob.kind, MobKind::Panda(_))) {
            return;
        }
        if is_worried(self.entity(id)) {
            if self.is_thundering() && !self.entity(id).is_in_water() {
                let e = self.entity_mut(id);
                sit(e, true);
                eat(e, false);
            } else if !is_eating(self.entity(id)) {
                sit(self.entity_mut(id), false);
            }
        }
        let target = self.get_target(id);
        if target.is_none() {
            let panda = panda_mut(self.mob_mut(id).expect("a panda"));
            panda.got_bamboo = false;
            panda.did_bite = false;
        }
        if get_unhappy_counter(self.entity(id)) > 0 {
            if let Some(target) = target {
                self.look_at(id, target, 90.0, 90.0);
            }
            let unhappy_counter = get_unhappy_counter(self.entity(id));
            if unhappy_counter == 29 || unhappy_counter == 14 {
                self.entity_play_sound(id, sound_event("minecraft:entity.panda.cant_breed"), 1.0, 1.0);
            }
            set_unhappy_counter(self.entity_mut(id), unhappy_counter - 1);
        }
        if is_sneezing(self.entity(id)) {
            let sneeze_counter = int(self.entity(id), SNEEZE_COUNTER) + 1;
            self.entity_mut(id).data.set(SNEEZE_COUNTER, DataValue::Int(sneeze_counter));
            if sneeze_counter > 20 {
                sneeze(self.entity_mut(id), false);
                self.panda_after_sneeze(id);
            } else if sneeze_counter == 1 {
                self.entity_play_sound(id, sound_event("minecraft:entity.panda.pre_sneeze"), 1.0, 1.0);
            }
        }
        if is_rolling(self.entity(id)) {
            self.panda_handle_roll(id);
        } else {
            panda_mut(self.mob_mut(id).expect("a panda")).roll_counter = 0;
        }
        if is_sitting(self.entity(id)) {
            self.entity_mut(id).set_x_rot(0.0);
        }
        self.panda_handle_eating(id);
    }

    fn panda_handle_eating(&mut self, id: EntityId) {
        let main_hand_empty = self.item_by_slot(id, EquipmentSlot::Mainhand).is_none();
        let e = self.entity(id);
        let should_start = !is_eating(e) && is_sitting(e) && !is_scared(e, self) && !main_hand_empty;
        if should_start && self.entity_mut(id).random.next_int_bounded(80) == 1 {
            eat(self.entity_mut(id), true);
        } else if main_hand_empty || !is_sitting(self.entity(id)) {
            eat(self.entity_mut(id), false);
        }
        if is_eating(self.entity(id)) {
            self.panda_add_eating_particles(id);
            let eat_counter = int(self.entity(id), EAT_COUNTER);
            if eat_counter > 80 && self.entity_mut(id).random.next_int_bounded(20) == 1 {
                if eat_counter > 100 && self.item_by_slot(id, EquipmentSlot::Mainhand).is_some_and(|item| item.get_item().is(&item_tags::PANDA_EATS_FROM_GROUND)) {
                    self.set_item_slot(id, EquipmentSlot::Mainhand, ItemStack::empty());
                    self.entity_game_event(id, crate::world::level::gameevent::game_event::EAT);
                    sit(self.entity_mut(id), false);
                }
                eat(self.entity_mut(id), false);
                return;
            }
            self.entity_mut(id).data.set(EAT_COUNTER, DataValue::Int(eat_counter + 1));
        }
    }

    fn panda_add_eating_particles(&mut self, id: EntityId) {
        if int(self.entity(id), EAT_COUNTER) % EAT_TICK_INTERVAL == 0 {
            let e = self.entity_mut(id);
            let volume = 0.5 + 0.5 * e.random.next_int_bounded(2) as f32;
            let pitch = (e.random.next_float() - e.random.next_float()) * 0.2 + 1.0;
            self.entity_play_sound(id, sound_event("minecraft:entity.panda.eat"), volume, pitch);
            if self.item_by_slot(id, EquipmentSlot::Mainhand).is_some() {
                let e = self.entity_mut(id);
                for _ in 0..6 {
                    for _ in 0..6 {
                        e.random.next_float();
                    }
                }
            }
        }
    }

    fn panda_handle_roll(&mut self, id: EntityId) {
        let baby = self.is_baby(id);
        let mob = self.mob_mut(id).expect("a panda");
        panda_mut(mob).roll_counter += 1;
        if panda_mut(mob).roll_counter > TOTAL_ROLL_STEPS {
            roll(&mut mob.entity, false);
            return;
        }
        let roll_counter = panda_mut(mob).roll_counter;
        let movement = mob.entity.delta_movement;
        if roll_counter == 1 {
            let angle = mob.entity.y_rot * (std::f64::consts::PI / 180.0) as f32;
            let multiplier = if baby { 0.1f32 } else { 0.2 };
            let roll_delta = Vec3::new(movement.x + (-mth::sin(angle as f64) * multiplier) as f64, 0.0, movement.z + (mth::cos(angle as f64) * multiplier) as f64);
            panda_mut(mob).roll_delta = roll_delta;
            mob.entity.set_delta_movement(roll_delta.add(0.0, 0.27, 0.0));
        } else if roll_counter as f32 != 7.0 && roll_counter as f32 != 15.0 && roll_counter as f32 != 23.0 {
            let roll_delta = panda(mob).roll_delta;
            mob.entity.set_delta_movement(Vec3::new(roll_delta.x, movement.y, roll_delta.z));
        } else {
            let y = if mob.entity.on_ground { 0.27 } else { movement.y };
            mob.entity.set_delta_movement(Vec3::new(0.0, y, 0.0));
        }
    }

    fn panda_after_sneeze(&mut self, id: EntityId) {
        self.entity_play_sound(id, sound_event("minecraft:entity.panda.sneeze"), 1.0, 1.0);
        let area = self.entity(id).bounding_box().inflate_all(10.0);
        for panda in self.get_entities(None, &area) {
            if !matches!(self.mob(panda).map(|mob| &mob.kind), Some(MobKind::Panda(_))) {
                continue;
            }
            let e = self.entity(panda);
            if !self.is_baby(panda) && e.on_ground && !e.is_in_water() && can_perform_action(e, self) {
                self.jump_from_ground(panda);
            }
        }
        if self.game_rules.get_bool(game_rule::MOB_DROPS) {
            self.drop_from_gift_loot_table(id, PANDA_SNEEZE, |level, item_stack| {
                level.spawn_at_location(id, item_stack, 0.0);
            });
        }
    }

    pub fn panda_pick_up_item(&mut self, id: EntityId, entity: EntityId) {
        if self.item_by_slot(id, EquipmentSlot::Mainhand).is_none() && can_pick_up_and_eat(self, entity) {
            self.on_item_pickup(id, entity);
            let Some(LevelEntity::Item(item)) = self.find_entity(entity) else { return };
            let item_stack = item.item.clone();
            let count = item_stack.count;
            self.set_item_slot(id, EquipmentSlot::Mainhand, item_stack);
            self.set_guaranteed_drop(id, EquipmentSlot::Mainhand);
            self.take(id, entity, count);
            self.discard(entity);
        }
    }

    pub fn panda_hurt_server(&mut self, id: EntityId, source: crate::world::damagesource::DamageSource, damage: f32) -> bool {
        sit(self.entity_mut(id), false);
        self.living_entity_hurt_server(id, source, damage)
    }

    fn panda_try_to_sit(&mut self, id: EntityId) {
        if !self.entity(id).is_in_water() {
            let mob = self.mob_mut(id).expect("a panda");
            mob.living.zza = 0.0;
            mob.ai.navigation.stop();
            sit(&mut mob.entity, true);
        }
    }

    pub fn panda_mob_interact(&mut self, id: EntityId, player: EntityId, hand: InteractionHand) -> InteractionResult {
        let mut interaction_item_stack = self.player(player).item_in_hand(hand).unwrap_or_else(ItemStack::empty);
        if is_scared(self.entity(id), self) {
            return InteractionResult::Pass;
        }
        if is_on_back(self.entity(id)) {
            set_on_back(self.entity_mut(id), false);
            return InteractionResult::SUCCESS;
        }
        if is_food(&interaction_item_stack) {
            if self.get_target(id).is_some() {
                panda_mut(self.mob_mut(id).expect("a panda")).got_bamboo = true;
            }
            if self.can_age_up(id) {
                self.use_player_item(player, hand, &mut interaction_item_stack);
                let age = self.mob(id).and_then(|mob| mob.ageable()).map_or(0, |ageable| ageable.age);
                self.age_up(id, ((-age / 20) as f32 * 0.1) as i32, true);
            } else {
                if self.is_baby(id) {
                    return InteractionResult::Pass;
                }
                let age = self.mob(id).and_then(|mob| mob.ageable()).map_or(0, |ageable| ageable.age);
                if age == 0 && self.can_fall_in_love(id) {
                    self.use_player_item(player, hand, &mut interaction_item_stack);
                    self.set_in_love(id, Some(player));
                } else {
                    if is_sitting(self.entity(id)) || self.entity(id).is_in_water() {
                        return InteractionResult::Pass;
                    }
                    self.panda_try_to_sit(id);
                    eat(self.entity_mut(id), true);
                    if let Some(pandas_current_item) = self.item_by_slot(id, EquipmentSlot::Mainhand)
                        && !self.has_infinite_materials(player)
                    {
                        self.spawn_at_location(id, pandas_current_item, 0.0);
                    }
                    self.set_item_slot(id, EquipmentSlot::Mainhand, ItemStack::new(interaction_item_stack.get_item(), 1));
                    self.use_player_item(player, hand, &mut interaction_item_stack);
                }
            }
            InteractionResult::SUCCESS_SERVER
        } else if self.is_baby(id) && self.is_holding(player, items::GOLDEN_DANDELION) {
            self.animal_mob_interact(id, player, hand)
        } else {
            InteractionResult::Pass
        }
    }

    pub fn panda_move_control_tick(&mut self, id: EntityId) {
        if self.panda_can_perform_action(id) {
            self.move_control_tick(id);
        }
    }

    fn panda_can_find_bamboo(&mut self, id: EntityId) -> bool {
        let panda_pos = self.entity(id).block_position();
        for y_off in 0..3 {
            for r in 0..8 {
                let mut x = 0;
                while x <= r {
                    let mut z = if x < r && x > -r { r } else { 0 };
                    while z <= r {
                        if self.block_state(panda_pos.offset(x, y_off, z)).is_block(blocks::BAMBOO) {
                            return true;
                        }
                        z = if z > 0 { -z } else { 1 - z };
                    }
                    x = if x > 0 { -x } else { 1 - x };
                }
            }
        }
        false
    }

    fn panda_items_in(&self, id: EntityId, inflate: f64) -> Vec<EntityId> {
        let area = self.entity(id).bounding_box().inflate_all(inflate);
        self.items_in(&area, None).into_iter().filter(|&item| can_pick_up_and_eat(self, item)).collect()
    }
}

pub struct PandaAttackGoal {
    pub melee_attack_goal: MeleeAttackGoal,
}

impl PandaAttackGoal {
    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        level.panda_can_perform_action(mob) && self.melee_attack_goal.can_use(level, mob)
    }
}

pub struct PandaAvoidGoal {
    pub avoid_entity_goal: AvoidEntityGoal,
}

impl PandaAvoidGoal {
    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        is_worried(level.entity(mob)) && level.panda_can_perform_action(mob) && self.avoid_entity_goal.can_use(level, mob)
    }
}

pub struct PandaBreedGoal {
    pub breed_goal: BreedGoal,
    unhappy_cooldown: i32,
}

impl PandaBreedGoal {
    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        if !self.breed_goal.can_use(level, mob) || get_unhappy_counter(level.entity(mob)) != 0 {
            return false;
        }
        if !level.panda_can_find_bamboo(mob) {
            let tick_count = level.entity(mob).tick_count;
            if self.unhappy_cooldown <= tick_count {
                set_unhappy_counter(level.entity_mut(mob), TOTAL_UNHAPPY_TIME);
                self.unhappy_cooldown = tick_count + 600;
                if level.is_effective_ai(mob) {
                    let e = level.entity(mob);
                    let (x, y, z) = (e.x(), e.y(), e.z());
                    let player = level.get_nearest_player(&BREED_TARGETING, Some(mob), x, y, z);
                    level.panda_set_look_at_player_goal_target(mob, player);
                }
            }
            false
        } else {
            true
        }
    }
}

impl ServerLevel {
    fn panda_set_look_at_player_goal_target(&mut self, id: EntityId, target: Option<EntityId>) {
        panda_mut(self.mob_mut(id).expect("a panda")).look_at_player_target = Some(target);
    }
}

pub struct PandaHurtByTargetGoal {
    pub hurt_by_target_goal: HurtByTargetGoal,
}

impl PandaHurtByTargetGoal {
    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        let panda = panda(level.mob(mob).expect("a panda"));
        if !panda.got_bamboo && !panda.did_bite {
            return self.hurt_by_target_goal.target_goal.can_continue_to_use(level, mob);
        }
        level.set_target(mob, None);
        false
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.hurt_by_target_goal.start_with(level, mob, PandaHurtByTargetGoal::alert_other);
    }

    fn alert_other(_goal: &HurtByTargetGoal, level: &mut ServerLevel, _mob: EntityId, other: EntityId, hurt_by_mob: EntityId) {
        if matches!(level.mob(other).map(|mob| &mob.kind), Some(MobKind::Panda(_))) && is_aggressive(level.entity(other)) {
            level.set_target(other, Some(hurt_by_mob));
        }
    }
}

pub struct PandaLieOnBackGoal {
    cooldown: i32,
}

impl PandaLieOnBackGoal {
    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        let e = level.entity(mob);
        let ready = self.cooldown < e.tick_count && is_lazy(e) && can_perform_action(e, level);
        ready && level.entity_mut(mob).random.next_int_bounded(reduced_tick_delay(400)) == 1
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        if !level.entity(mob).is_in_water() && (is_lazy(level.entity(mob)) || level.entity_mut(mob).random.next_int_bounded(reduced_tick_delay(600)) != 1) {
            level.entity_mut(mob).random.next_int_bounded(reduced_tick_delay(2000)) != 1
        } else {
            false
        }
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        set_on_back(level.entity_mut(mob), true);
        self.cooldown = 0;
    }

    pub fn stop(&mut self, level: &mut ServerLevel, mob: EntityId) {
        set_on_back(level.entity_mut(mob), false);
        self.cooldown = level.entity(mob).tick_count + 200;
    }
}

pub struct PandaLookAtPlayerGoal {
    pub look_at_player_goal: LookAtPlayerGoal,
}

impl PandaLookAtPlayerGoal {
    fn take_set_target(&mut self, level: &mut ServerLevel, mob: EntityId) {
        if let Some(target) = panda_mut(level.mob_mut(mob).expect("a panda")).look_at_player_target.take() {
            self.look_at_player_goal.look_at = target;
        }
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        self.take_set_target(level, mob);
        self.look_at_player_goal.look_at.is_some() && self.look_at_player_goal.can_continue_to_use(level, mob)
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        self.take_set_target(level, mob);
        if level.entity_mut(mob).random.next_float() >= self.look_at_player_goal.probability {
            return false;
        }
        if self.look_at_player_goal.look_at.is_none() {
            let e = level.entity(mob);
            let (x, y, z) = (e.x(), e.eye_y(), e.z());
            self.look_at_player_goal.look_at = level.get_nearest_player(&self.look_at_player_goal.look_at_context(), Some(mob), x, y, z);
        }
        level.panda_can_perform_action(mob) && self.look_at_player_goal.look_at.is_some()
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.take_set_target(level, mob);
        if self.look_at_player_goal.look_at.is_some() {
            self.look_at_player_goal.tick(level, mob);
        }
    }
}

pub struct PandaPanicGoal {
    pub panic_goal: PanicGoal,
}

impl PandaPanicGoal {
    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        if is_sitting(level.entity(mob)) {
            level.mob_mut(mob).expect("a panda").ai.navigation.stop();
            false
        } else {
            self.panic_goal.can_continue_to_use(level, mob)
        }
    }
}

pub struct PandaRollGoal;

impl PandaRollGoal {
    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        let e = level.entity(mob);
        if !((level.is_baby(mob) || is_playful(e)) && e.on_ground) {
            return false;
        }
        if !can_perform_action(e, level) {
            return false;
        }
        let angle = e.y_rot * (std::f64::consts::PI / 180.0) as f32;
        let x_dir = -mth::sin(angle as f64);
        let z_dir = mth::cos(angle as f64);
        let x_step = if x_dir.abs() as f64 > 0.5 { mth::sign(x_dir as f64) } else { 0 };
        let z_step = if z_dir.abs() as f64 > 0.5 { mth::sign(z_dir as f64) } else { 0 };
        let (below, playful) = (e.block_position().offset(x_step, -1, z_step), is_playful(e));
        if level.block_state(below).is_air() {
            true
        } else if playful && level.entity_mut(mob).random.next_int_bounded(reduced_tick_delay(60)) == 1 {
            true
        } else {
            level.entity_mut(mob).random.next_int_bounded(reduced_tick_delay(500)) == 1
        }
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        roll(level.entity_mut(mob), true);
    }
}

pub struct PandaSitGoal {
    cooldown: i32,
}

impl PandaSitGoal {
    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        let e = level.entity(mob);
        if self.cooldown > e.tick_count || level.is_baby(mob) || e.is_in_water() || !can_perform_action(e, level) || get_unhappy_counter(e) > 0 {
            false
        } else if level.item_by_slot(mob, EquipmentSlot::Mainhand).is_some() {
            true
        } else {
            !level.panda_items_in(mob, 6.0).is_empty()
        }
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        if !level.entity(mob).is_in_water() && (is_lazy(level.entity(mob)) || level.entity_mut(mob).random.next_int_bounded(reduced_tick_delay(600)) != 1) {
            level.entity_mut(mob).random.next_int_bounded(reduced_tick_delay(2000)) != 1
        } else {
            false
        }
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        if !is_sitting(level.entity(mob)) && level.item_by_slot(mob, EquipmentSlot::Mainhand).is_some() {
            level.panda_try_to_sit(mob);
        }
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        if level.item_by_slot(mob, EquipmentSlot::Mainhand).is_none() {
            let items = level.panda_items_in(mob, 8.0);
            if let Some(&first) = items.first() {
                level.get_navigation(mob, |navigation, level| navigation.move_to_entity(level, mob, first, 1.2f32 as f64));
            }
        } else {
            level.panda_try_to_sit(mob);
        }
        self.cooldown = 0;
    }

    pub fn stop(&mut self, level: &mut ServerLevel, mob: EntityId) {
        if let Some(item_stack) = level.item_by_slot(mob, EquipmentSlot::Mainhand) {
            level.spawn_at_location(mob, item_stack, 0.0);
            level.set_item_slot(mob, EquipmentSlot::Mainhand, ItemStack::empty());
            let lazy = is_lazy(level.entity(mob));
            let random = &mut level.entity_mut(mob).random;
            let wait_seconds = if lazy { random.next_int_bounded(50) + 10 } else { random.next_int_bounded(150) + 10 };
            self.cooldown = level.entity(mob).tick_count + wait_seconds * 20;
        }
        sit(level.entity_mut(mob), false);
    }
}

pub struct PandaSneezeGoal;

impl PandaSneezeGoal {
    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        if level.is_baby(mob) && level.panda_can_perform_action(mob) {
            if is_weak(level.entity(mob)) && level.entity_mut(mob).random.next_int_bounded(reduced_tick_delay(500)) == 1 {
                true
            } else {
                level.entity_mut(mob).random.next_int_bounded(reduced_tick_delay(6000)) == 1
            }
        } else {
            false
        }
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        sneeze(level.entity_mut(mob), true);
    }
}
