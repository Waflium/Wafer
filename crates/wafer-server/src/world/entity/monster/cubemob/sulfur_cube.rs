use wafer_core::random::RandomSource;
use wafer_core::{BlockPos, Vec2, Vec3, mth};
use wafer_data::{EquipmentSlot, damage_type_tags, fluid_tags, game_rule, item_tags, items};
use wafer_protocol::nbt::Nbt;

use super::abstract_cube_mob::{self, AbstractCubeMob};
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::damagesource::DamageSource;
use crate::world::difficulty_instance::DifficultyInstance;
use crate::world::entity::ai::MobAi;
use crate::world::entity::ai::goal::goal::Goal;
use crate::world::entity::ai::goal::tempt_goal::TemptGoal;
use crate::world::entity::attributes::{Attribute, SULFUR_CUBE_ATTRIBUTES};
use crate::world::entity::data::DataValue;
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::spawn_group_data::SpawnGroupData;
use crate::world::entity::sulfur_cube_archetype::{self, ContactDamage, ExplosionData, KnockbackModifiers, SoundSettings, SulfurCubeArchetype};
use crate::world::entity::{EntityDimensions, EntityId, LevelEntity, RemovalReason, sound_event};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::level::gameevent::game_event;
use crate::world::level::server_explosion::{ExplosionDamageCalculator, ExplosionInteraction};
use crate::world::level::server_level_accessor::ServerLevelAccessor;
use crate::world::{InteractionHand, InteractionResult};
use crate::world::level::storage::tag_value_input;

pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.49, 0.49).with_eye_height(0.175);
pub const DATA_MAX_FUSE: usize = 19;
pub const DATA_FROM_BUCKET: usize = 20;
pub const SPLIT_COUNT: i32 = 2;
pub const PICKUP_TIMER_DURATION: i32 = 100;
const PUSH_DISTANCE_THRESHOLD: f64 = 1.3f32 as f64;
const MAX_PLAYER_PUSH_SPEED: f64 = 0.5;
const PLAYER_PUSH_SPEED_SCALE_MULTIPLIER: f32 = 0.3;
const VEHICLE_PUSH_SPEED_SCALE_MULTIPLIER: f32 = 0.16;
const VERTICAL_PUSH_MULTIPLIER: f32 = 0.3;
const HORIZONTAL_HIT_ANGLE_SCALE: f32 = 1.6;
const VERTICAL_HIT_ANGLE_SCALE: f32 = 0.5;
const VERTICAL_POSITION_ANGLE_SCALE: f32 = 0.8;

pub struct SulfurCube {
    pub abstract_cube_mob: AbstractCubeMob,
    pub pickup_timer: i32,
    pub push_sound_cooldown: i32,
    pub floats_in_liquids: bool,
    pub explosion_data: Option<ExplosionData>,
    pub knockback_modifier: KnockbackModifiers,
    pub sound_settings: SoundSettings,
    pub fuse: i32,
    pub contact_damages: Vec<ContactDamage>,
}

impl Default for SulfurCube {
    fn default() -> SulfurCube {
        SulfurCube {
            abstract_cube_mob: AbstractCubeMob::default(),
            pickup_timer: 0,
            push_sound_cooldown: 0,
            floats_in_liquids: false,
            explosion_data: None,
            knockback_modifier: sulfur_cube_archetype::DEFAULT_KNOCKBACK_MODIFIERS,
            sound_settings: sulfur_cube_archetype::DEFAULT_SOUND_SETTINGS,
            fuse: -1,
            contact_damages: Vec::new(),
        }
    }
}

pub fn is_swallowable_item(item_stack: &ItemStack) -> bool {
    item_stack.get_item().is(&item_tags::SULFUR_CUBE_SWALLOWABLE)
}

fn is_food(item_stack: &ItemStack) -> bool {
    item_stack.get_item().is(&item_tags::SULFUR_CUBE_FOOD)
}

fn tempt_items(level: &ServerLevel, mob: EntityId, item_stack: &ItemStack) -> bool {
    if level.is_baby(mob) { is_food(item_stack) } else { is_swallowable_item(item_stack) }
}

fn matching_archetypes(stack: &ItemStack) -> impl Iterator<Item = &'static SulfurCubeArchetype> + '_ {
    SulfurCubeArchetype::registered().iter().filter(move |archetype| archetype.matches(stack))
}

impl SulfurCube {
    pub fn create() -> Mob {
        let mut data = abstract_cube_mob::abstract_cube_mob_defaults();
        data.push(DataValue::Int(-1));
        data.push(DataValue::Boolean(false));
        let mut mob = AbstractCubeMob::create_with_data("minecraft:sulfur_cube", DIMENSIONS, &SULFUR_CUBE_ATTRIBUTES, MobKind::SulfurCube(SulfurCube::default()), data);
        mob.ai.look_control.kind = crate::world::entity::ai::control::look_control::LookControlKind::SulfurCube;
        mob
    }

    pub fn register_goals(ai: &mut MobAi) {
        AbstractCubeMob::register_goals(ai);
        SulfurCube::add_behaviour_goals(ai);
    }

    pub fn add_behaviour_goals(ai: &mut MobAi) {
        ai.goal_selector.add_goal(2, Goal::SulfurCubeTempt(TemptGoal::for_sulfur_cube(1.0, tempt_items, false, 1.0)));
        ai.goal_selector.add_goal(3, Goal::SulfurCubeSearchForItems(SulfurCubeSearchForItemsGoal::default()));
    }

    pub fn get_hurt_sound(mob: &Mob) -> &'static str {
        if mob.is_tiny() { "minecraft:entity.small_sulfur_cube.hurt" } else { "minecraft:entity.sulfur_cube.hurt" }
    }

    pub fn get_death_sound(mob: &Mob) -> &'static str {
        if mob.is_tiny() { "minecraft:entity.small_sulfur_cube.death" } else { "minecraft:entity.sulfur_cube.death" }
    }

    pub fn get_squish_sound(mob: &Mob) -> &'static str {
        if mob.is_tiny() {
            "minecraft:entity.small_sulfur_cube.squish"
        } else if mob.has_body_item() {
            "minecraft:entity.sulfur_cube.bounce"
        } else {
            "minecraft:entity.sulfur_cube.squish"
        }
    }

    pub fn get_jump_sound(mob: &Mob) -> &'static str {
        if mob.is_tiny() { "minecraft:entity.small_sulfur_cube.jump" } else { "minecraft:entity.sulfur_cube.jump" }
    }

    pub fn get_particle_type() -> Option<&'static str> {
        Some("minecraft:sulfur_cube_goo")
    }
}

impl Mob {
    pub fn sulfur_cube(&self) -> Option<&SulfurCube> {
        match &self.kind {
            MobKind::SulfurCube(sulfur_cube) => Some(sulfur_cube),
            _ => None,
        }
    }

    pub fn sulfur_cube_mut(&mut self) -> Option<&mut SulfurCube> {
        match &mut self.kind {
            MobKind::SulfurCube(sulfur_cube) => Some(sulfur_cube),
            _ => None,
        }
    }

    pub fn has_body_item(&self) -> bool {
        self.equipment[EquipmentSlot::Body as usize].as_ref().is_some_and(|stack| !stack.is_empty())
    }

    pub fn sulfur_cube_from_bucket(&self) -> bool {
        matches!(self.entity.data.get(DATA_FROM_BUCKET), DataValue::Boolean(true))
    }

    pub fn sulfur_cube_set_from_bucket(&mut self, from_bucket: bool) {
        self.entity.data.set(DATA_FROM_BUCKET, DataValue::Boolean(from_bucket));
    }

    pub fn sulfur_cube_get_fuse(&self) -> i32 {
        self.sulfur_cube().expect("a sulfur cube").fuse
    }

    pub fn sulfur_cube_is_primed(&self) -> bool {
        self.sulfur_cube_get_fuse() >= 0
    }

    fn sulfur_cube_set_fuse(&mut self, fuse: i32) {
        self.sulfur_cube_mut().expect("a sulfur cube").fuse = fuse;
    }

    fn sulfur_cube_set_max_fuse(&mut self, fuse: i32) {
        self.entity.data.set(DATA_MAX_FUSE, DataValue::Int(fuse));
        self.sulfur_cube_set_fuse(fuse);
    }

    pub fn sulfur_cube_set_size(&mut self, size: i32, update_health: bool) -> bool {
        let changed = self.abstract_cube_mob_set_size(size, update_health);
        if update_health && size == 1 && !self.is_baby() {
            self.set_baby(true);
        }
        changed
    }

    pub fn sulfur_cube_set_cube_mob_health(&mut self, actual_size: i32) {
        self.living.attributes.instance(Attribute::MaxHealth).expect("sulfur cubes have a max health").set_base_value((4 * actual_size) as f64);
    }

    pub fn sulfur_cube_requires_custom_persistence(&self) -> bool {
        self.has_body_item() || self.sulfur_cube_from_bucket()
    }

    pub fn sulfur_cube_max_up_step(&self) -> Option<f32> {
        self.has_body_item().then_some(0.0)
    }

    pub fn sulfur_cube_add_additional_save_data(&self, out: &mut Vec<(String, Nbt)>) {
        let sulfur_cube = self.sulfur_cube().expect("a sulfur cube");
        out.push(("pickup_timer".into(), Nbt::Int(sulfur_cube.pickup_timer)));
        out.push(("from_bucket".into(), Nbt::Byte(self.sulfur_cube_from_bucket() as i8)));
        out.push(("fuse".into(), Nbt::Int(sulfur_cube.fuse)));
    }

    pub fn sulfur_cube_read_additional_save_data(&mut self, input: &Nbt) {
        let int = |key: &str, default: i32| tag_value_input::get_numeric(input, key).and_then(crate::world::level::block::entity::int_value).unwrap_or(default);
        self.sulfur_cube_mut().expect("a sulfur cube").pickup_timer = int("pickup_timer", 0);
        let from_bucket = tag_value_input::get_i64(input, "from_bucket").is_some_and(|v| v != 0);
        self.sulfur_cube_set_from_bucket(from_bucket);
        self.sulfur_cube_set_fuse(int("fuse", -1));
        let fuse = self.sulfur_cube_get_fuse();
        self.sulfur_cube_set_max_fuse(fuse);
    }

    pub fn sulfur_cube_save_to_bucket_tag(&self, bucket: &mut ItemStack) {
        crate::world::entity::bucketable::save_default_data_to_bucket_tag(self, bucket);
        if let Some(body) = self.equipment[EquipmentSlot::Body as usize].as_ref().filter(|stack| !stack.is_empty()) {
            bucket.set(dc::SULFUR_CUBE_CONTENT, Some(crate::world::item::component::bundle_contents::template_of(body)));
        }
        let mut tag = match bucket.get(dc::BUCKET_ENTITY_DATA) {
            Some(Nbt::Compound(tag)) => tag.copy(),
            _ => Vec::new().into(),
        };
        tag.put("age", Nbt::Int(self.ageable().map_or(0, |ageable| ageable.age)));
        tag.put("age_locked", Nbt::Byte(self.is_age_locked() as i8));
        bucket.set(dc::BUCKET_ENTITY_DATA, Some(Nbt::Compound(tag)));
    }

    pub fn sulfur_cube_load_from_bucket_tag(&mut self, tag: &Nbt) {
        crate::world::entity::bucketable::load_default_data_from_bucket_tag(self, tag);
        let age = tag_value_input::get_numeric(tag, "age").and_then(crate::world::level::block::entity::int_value).unwrap_or(0);
        self.set_age(age);
        let age_locked = tag_value_input::get_i64(tag, "age_locked").is_some_and(|v| v != 0);
        self.entity.data.set(crate::world::entity::data::ids::AGE_LOCKED, DataValue::Boolean(age_locked));
    }

    pub fn sulfur_cube_apply_content(&mut self, value: &Nbt) {
        let Some(stack) = crate::world::item::component::values::ItemStackTemplate::from_nbt(value).create() else { return };
        self.equipment[EquipmentSlot::Body as usize] = Some(stack).filter(|stack| !stack.is_empty());
        self.drop_chances[EquipmentSlot::Body as usize] = 2.0;
    }

    pub fn sulfur_cube_can_use_slot(&self, slot: EquipmentSlot) -> Option<bool> {
        (slot == EquipmentSlot::Body).then(|| !self.living.dead && self.health > 0.0 && !self.is_baby())
    }

    pub fn sulfur_cube_can_hold_item(&self, item_stack: &ItemStack) -> bool {
        !self.has_body_item() && is_swallowable_item(item_stack) && !self.is_baby()
    }

}

pub fn finalize_spawn(
    level: &mut (impl ServerLevelAccessor + ?Sized),
    mob: &mut Mob,
    difficulty: &DifficultyInstance,
    spawn_reason: EntitySpawnReason,
    group_data: Option<SpawnGroupData>,
) -> Option<SpawnGroupData> {
    let group_data = crate::world::entity::ageable_mob::finalize_spawn(level, mob, group_data);
    let data = crate::world::entity::mob::mob_finalize_spawn(level, mob, difficulty, spawn_reason, group_data);
    if mob.is_baby() {
        mob.set_size(1, true);
    } else {
        mob.set_size(2, true);
    }
    data
}

impl ServerLevel {
    fn sulfur_cube_mut(&mut self, id: EntityId) -> &mut SulfurCube {
        self.mob_mut(id).and_then(Mob::sulfur_cube_mut).expect("a sulfur cube")
    }

    fn sulfur_cube_ref(&self, id: EntityId) -> &SulfurCube {
        self.mob(id).and_then(Mob::sulfur_cube).expect("a sulfur cube")
    }

    pub fn sulfur_cube_has_body_item(&self, id: EntityId) -> bool {
        self.mob(id).is_some_and(Mob::has_body_item)
    }

    pub fn sulfur_cube_can_explode(&self, id: EntityId) -> bool {
        let mob = self.mob(id).expect("a sulfur cube");
        mob.sulfur_cube().expect("a sulfur cube").explosion_data.is_some() && self.is_alive(id) && !mob.sulfur_cube_is_primed()
    }

    pub fn sulfur_cube_hurt_server(&mut self, id: EntityId, source: DamageSource, damage: f32) -> bool {
        if self.sulfur_cube_has_body_item(id) {
            if self.sulfur_cube_can_explode(id) && !self.mob(id).expect("a sulfur cube").sulfur_cube_is_primed() {
                let burning_arrow = source.direct_entity.is_some_and(|direct| self.as_projectile(direct).is_some_and(|p| p.abstract_arrow().is_some()) && self.is_on_fire(direct));
                if source.is(&damage_type_tags::IS_FIRE) || burning_arrow {
                    self.sulfur_cube_prime_time(id, false);
                } else if source.is(&damage_type_tags::IS_EXPLOSION) {
                    self.sulfur_cube_prime_time(id, true);
                }
            }
            if source.is(&damage_type_tags::SULFUR_CUBE_WITH_BLOCK_IMMUNE_TO) {
                if !source.is(&damage_type_tags::NO_KNOCKBACK) {
                    self.deal_default_knockback(id, &source, damage, true);
                }
                return true;
            }
        }
        self.living_entity_hurt_server(id, source, damage)
    }

    pub fn sulfur_cube_tick(&mut self, id: EntityId) {
        self.sulfur_cube_tick_fuse(id);
        self.sulfur_cube_prime_when_on_powered_position(id);
        self.abstract_cube_mob_tick(id);
    }

    fn sulfur_cube_tick_fuse(&mut self, id: EntityId) {
        let sulfur_cube = self.sulfur_cube_mut(id);
        if sulfur_cube.fuse > 0 {
            sulfur_cube.fuse -= 1;
        }
        let Some(explosion_data) = sulfur_cube.explosion_data else { return };
        if sulfur_cube.fuse != 0 {
            return;
        }
        self.drop_leash(id);
        self.living_mut(id).dead = true;
        if self.game_rules.get_bool(game_rule::TNT_EXPLODES) {
            let interaction = if self.game_rules.get_bool(game_rule::MOB_GRIEFING) { ExplosionInteraction::Tnt } else { ExplosionInteraction::None };
            let e = self.entity(id);
            let center = Vec3::new(e.x(), e.y() + e.dimensions.height as f64 * 0.0625, e.z());
            let damage_calculator = if e.portal_cooldown > 0 { ExplosionDamageCalculator::UsedPortal } else { ExplosionDamageCalculator::Default };
            self.explode_with_calculator(Some(id), Some(id), None, damage_calculator, center, explosion_data.power as f32, explosion_data.causes_fire, interaction);
        }
        self.trigger_on_death_mob_effects(id, RemovalReason::Killed);
        self.discard(id);
    }

    fn sulfur_cube_prime_when_on_powered_position(&mut self, id: EntityId) {
        if self.sulfur_cube_can_explode(id) {
            let here = BlockPos::containing_vec(self.entity(id).position());
            if self.get_best_own_or_neighbour_signal(here) != 0 {
                self.sulfur_cube_prime_time(id, false);
            }
        }
    }

    pub fn sulfur_cube_prime_time(&mut self, id: EntityId, imminent: bool) -> bool {
        let mob = self.mob(id).expect("a sulfur cube");
        let Some(explosion_data) = mob.sulfur_cube().expect("a sulfur cube").explosion_data else { return false };
        if !self.is_alive(id) || !self.game_rules.get_bool(game_rule::TNT_EXPLODES) || mob.sulfur_cube_is_primed() {
            return false;
        }
        let fuse = explosion_data.fuse;
        let fuse_time = if imminent { crate::world::entity::primed_tnt::random_short_fuse(fuse, &mut self.entity_mut(id).random) } else { fuse };
        self.entity_mut(id).invulnerable = true;
        self.mob_mut(id).expect("a sulfur cube").sulfur_cube_set_max_fuse(fuse_time);
        self.make_sound(id, sound_event("minecraft:entity.tnt.primed"));
        self.entity_game_event(id, game_event::PRIME_FUSE);
        true
    }

    pub fn sulfur_cube_custom_server_ai_step(&mut self, id: EntityId) {
        let sulfur_cube = self.sulfur_cube_mut(id);
        if sulfur_cube.pickup_timer > 0 {
            sulfur_cube.pickup_timer -= 1;
        }
        if sulfur_cube.push_sound_cooldown > 0 {
            sulfur_cube.push_sound_cooldown -= 1;
        }
    }

    pub fn sulfur_cube_collect_equipment_changes(&mut self, id: EntityId, previous: &ItemStack) {
        let current = self.item_by_slot(id, EquipmentSlot::Body).unwrap_or_else(ItemStack::empty);
        if previous == &current {
            return;
        }
        if !current.is_empty() {
            self.with_goal_selector(id, |goal_selector, level| {
                goal_selector.remove_all_goals(level, id, |_| true);
            });
            self.mob_mut(id).expect("a sulfur cube").set_speed(0.0);
        } else {
            self.mob_mut(id).expect("a sulfur cube").register_goals();
        }
        for archetype in matching_archetypes(previous) {
            for (attribute, modifier) in &archetype.attribute_modifiers {
                if let Some(instance) = self.living_mut(id).attributes.instance(*attribute) {
                    instance.remove_modifier(&modifier.id);
                }
            }
        }
        let sulfur_cube = self.sulfur_cube_mut(id);
        sulfur_cube.floats_in_liquids = false;
        sulfur_cube.explosion_data = None;
        sulfur_cube.contact_damages.clear();
        sulfur_cube.knockback_modifier = sulfur_cube_archetype::DEFAULT_KNOCKBACK_MODIFIERS;
        sulfur_cube.sound_settings = sulfur_cube_archetype::DEFAULT_SOUND_SETTINGS;
        for archetype in matching_archetypes(&current) {
            let sulfur_cube = self.sulfur_cube_mut(id);
            if archetype.buoyant {
                sulfur_cube.floats_in_liquids = true;
            }
            if archetype.explosion.is_some() {
                sulfur_cube.explosion_data = archetype.explosion;
            }
            if let Some(contact_damage) = &archetype.contact_damage {
                sulfur_cube.contact_damages.push(contact_damage.clone());
            }
            sulfur_cube.knockback_modifier = archetype.knockback_modifiers;
            sulfur_cube.sound_settings = archetype.sound_settings;
            for (attribute, modifier) in &archetype.attribute_modifiers {
                if let Some(instance) = self.living_mut(id).attributes.instance(*attribute) {
                    instance.add_or_update_transient_modifier(modifier.clone());
                }
            }
        }
    }

    pub fn sulfur_cube_mob_interact(&mut self, id: EntityId, player: EntityId, hand: InteractionHand) -> InteractionResult {
        let mut held_item = self.player(player).item_in_hand(hand).unwrap_or_else(ItemStack::empty);
        if self.is_baby(id) {
            if is_food(&held_item) && self.can_age_up(id) {
                let age = self.mob(id).and_then(Mob::ageable).map_or(0, |ageable| ageable.age);
                self.use_player_item(player, hand, &mut held_item);
                self.age_up(id, crate::world::entity::ageable_mob::get_speed_up_seconds_when_feeding(-age), true);
                self.make_sound(id, sound_event("minecraft:entity.small_sulfur_cube.eat"));
                return InteractionResult::SUCCESS;
            }
            return self.ageable_mob_mob_interact(id, player, hand);
        }
        if self.mob(id).expect("a sulfur cube").sulfur_cube_is_primed() {
            return InteractionResult::Pass;
        }
        if !self.sulfur_cube_can_explode(id) || !held_item.is(items::FLINT_AND_STEEL) && !held_item.is(items::FIRE_CHARGE) {
            if held_item.is(items::SHEARS) && self.sulfur_cube_has_body_item(id) {
                let item_stack_to_shear = self.item_by_slot(id, EquipmentSlot::Body).unwrap_or_else(ItemStack::empty);
                self.sulfur_cube_shear(id, SoundSource::Players, &held_item);
                self.entity_game_event_by(id, game_event::SHEAR, Some(player));
                self.hurt_and_break(player, hand.as_equipment_slot(), 1);
                crate::advancements::triggers::criteria_triggers::PLAYER_SHEARED_EQUIPMENT.trigger(self, player, &item_stack_to_shear, id);
                return InteractionResult::SUCCESS;
            }
            if is_swallowable_item(&held_item) {
                let it_worked = self.sulfur_cube_equip_item(id, &held_item);
                if it_worked {
                    held_item.consume(1, self, Some(player));
                    self.player_mut(player).set_item_in_hand(hand, Some(held_item));
                    self.entity_game_event(id, game_event::ENTITY_INTERACT);
                }
                return if it_worked { InteractionResult::SUCCESS_SERVER } else { InteractionResult::Pass };
            }
            return match crate::world::entity::bucketable::bucket_mob_pickup(self, player, hand, id) {
                Some(result) => result,
                None => self.ageable_mob_mob_interact(id, player, hand),
            };
        }
        if !self.game_rules.get_bool(game_rule::TNT_EXPLODES) {
            self.send_overlay_message(player, &crate::network::chat::Component::translatable("block.minecraft.tnt.disabled"));
            return InteractionResult::Pass;
        }
        self.sulfur_cube_prime_time(id, false);
        if held_item.is(items::FLINT_AND_STEEL) {
            self.hurt_and_break(player, hand.as_equipment_slot(), 1);
        } else {
            held_item.consume(1, self, Some(player));
            self.player_mut(player).set_item_in_hand(hand, Some(held_item.clone()));
        }
        self.award_stat(player, crate::stats::stats::ITEM_USED.get(held_item.get_item().0), 1);
        InteractionResult::SUCCESS_SERVER
    }

    pub fn sulfur_cube_equip_item(&mut self, id: EntityId, held_item: &ItemStack) -> bool {
        if self.is_baby(id) {
            return false;
        }
        if self.sulfur_cube_has_body_item(id) {
            let swallowed_item = self.item_by_slot(id, EquipmentSlot::Body).unwrap_or_else(ItemStack::empty);
            if held_item.is(swallowed_item.get_item()) {
                return false;
            }
            let equipment_spawn_offset = self.sulfur_cube_equipment_spawn_offset(id);
            self.spawn_at_location_with_offset(id, swallowed_item, equipment_spawn_offset);
        }
        let swallowed_item = self.item_by_slot(id, EquipmentSlot::Body).unwrap_or_else(ItemStack::empty);
        self.set_item_slot_and_drop_when_killed(id, EquipmentSlot::Body, held_item.copy_with_count(1));
        if !swallowed_item.is_empty() {
            let mut last_equipment_items: Vec<Option<ItemStack>> = vec![None; EquipmentSlot::VALUES.len()];
            last_equipment_items[EquipmentSlot::Body as usize] = Some(swallowed_item);
            self.collect_equipment_changes(id, &last_equipment_items);
        }
        self.entity_play_sound(id, sound_event("minecraft:entity.sulfur_cube.absorb"), 1.0, 1.0);
        true
    }

    fn sulfur_cube_equipment_spawn_offset(&self, id: EntityId) -> Vec3 {
        Vec3::new(0.0, self.entity(id).dimensions.height as f64, 0.0)
    }

    fn sulfur_cube_apply_contact_damage(&mut self, id: EntityId, entity: EntityId) {
        let damages = self.sulfur_cube_ref(id).contact_damages.clone();
        for damage in damages {
            let damage_source = if !damage.attribute_to_source && !self.is_player(entity) { None } else { Some(id) };
            let amount = damage.amount.sample(&mut self.entity_mut(id).random);
            let damage_type = wafer_data::DamageType::by_name(damage.damage_type).expect("a vanilla damage type");
            let source = match damage_source {
                Some(source) => DamageSource::entity(damage_type, source),
                None => DamageSource::new(damage_type),
            };
            self.hurt_server(entity, source, amount);
        }
    }

    pub fn sulfur_cube_shear(&mut self, id: EntityId, _sound_source: SoundSource, _tool: &ItemStack) {
        let equipment_spawn_offset = self.sulfur_cube_equipment_spawn_offset(id);
        let item_stack_to_shear = self.item_by_slot(id, EquipmentSlot::Body).unwrap_or_else(ItemStack::empty);
        self.set_item_slot(id, EquipmentSlot::Body, ItemStack::empty());
        self.spawn_at_location_with_offset(id, item_stack_to_shear, equipment_spawn_offset);
        self.entity_play_sound(id, sound_event("minecraft:entity.sulfur_cube.eject"), 1.0, 1.0);
        self.sulfur_cube_mut(id).pickup_timer = PICKUP_TIMER_DURATION;
    }

    pub fn sulfur_cube_pick_up_item(&mut self, id: EntityId, entity: EntityId) {
        let Some(LevelEntity::Item(item_entity)) = self.find_entity(entity) else { return };
        let item_stack = item_entity.item.clone();
        if self.mob(id).expect("a sulfur cube").sulfur_cube_can_hold_item(&item_stack) && self.sulfur_cube_ref(id).pickup_timer <= 0 {
            self.on_item_pickup(id, entity);
            let mut item_stack = item_stack;
            let one = item_stack.split(1);
            if let Some(LevelEntity::Item(item_entity)) = self.find_entity_mut(entity) {
                item_entity.set_item(item_stack);
            }
            self.set_item_slot(id, EquipmentSlot::Body, one);
            self.entity_play_sound(id, sound_event("minecraft:entity.sulfur_cube.absorb"), 1.0, 1.0);
            self.set_guaranteed_drop(id, EquipmentSlot::Body);
            self.take(id, entity, 1);
        }
    }

    pub fn sulfur_cube_get_base_experience_reward(&mut self, id: EntityId) -> i32 {
        if self.is_baby(id) { 0 } else { 1 + self.entity_mut(id).random.next_int_bounded(2) }
    }

    pub fn sulfur_cube_get_split_count(&self, id: EntityId) -> i32 {
        if self.mob(id).expect("a sulfur cube").sulfur_cube_is_primed() { 0 } else { SPLIT_COUNT }
    }

    pub fn sulfur_cube_age_boundary_reached(&mut self, id: EntityId) {
        if !self.is_baby(id) {
            self.set_size(id, 2, true);
        }
    }

    pub fn sulfur_cube_do_push(&mut self, id: EntityId, entity: EntityId) {
        self.sulfur_cube_apply_contact_damage(id, entity);
    }

    pub fn sulfur_cube_player_touch(&mut self, id: EntityId, player: EntityId) {
        if !self.sulfur_cube_has_body_item(id) {
            return;
        }
        let pusher = if self.is_passenger(player) { self.root_vehicle(player) } else { player };
        let (cube, push) = (self.entity(id), self.entity(pusher));
        let cube_to_pusher = cube.position().subtract(push.position());
        let pusher_feet_position = push.y();
        let sulfur_cube_bottom_position = cube.y();
        let sulfur_cube_top_position = sulfur_cube_bottom_position + cube.dimensions.height as f64;
        let pusher_top_position = pusher_feet_position + push.dimensions.height as f64;
        if cube_to_pusher.horizontal_distance() < PUSH_DISTANCE_THRESHOLD && pusher_feet_position <= sulfur_cube_top_position && pusher_top_position > sulfur_cube_bottom_position {
            let knockback = (1.0 - self.attribute_value(id, Attribute::KnockbackResistance)).max(0.0);
            let push_direction = cube_to_pusher.horizontal().normalize().scale(knockback);
            let push_speed_scale = if self.is_passenger(player) { VEHICLE_PUSH_SPEED_SCALE_MULTIPLIER } else { PLAYER_PUSH_SPEED_SCALE_MULTIPLIER };
            let mut player_speed = self.known_speed(player).length() * 2.0 * push_speed_scale as f64;
            player_speed = player_speed.clamp(0.0, MAX_PLAYER_PUSH_SPEED);
            let on_ground = self.entity(id).on_ground;
            let push_velocity = Vec3::new(push_direction.x, if on_ground { knockback * VERTICAL_PUSH_MULTIPLIER as f64 } else { 0.0 }, push_direction.z).scale(player_speed);
            self.entity_mut(id).needs_sync = true;
            let sound_settings = self.sulfur_cube_ref(id).sound_settings;
            let push_sound_threshold = sound_settings.push_sound_impulse_threshold;
            if push_velocity.length_sqr() > (push_sound_threshold * push_sound_threshold) as f64 && self.sulfur_cube_ref(id).push_sound_cooldown <= 0 {
                self.sulfur_cube_mut(id).push_sound_cooldown = (sound_settings.push_sound_cooldown * 20.0) as i32;
                self.entity_play_sound(id, sound_event(sound_settings.push_sound), 1.0, 1.0);
            }
            self.entity_mut(id).add_delta_movement(push_velocity);
            self.sulfur_cube_apply_contact_damage(id, player);
        }
    }

    pub fn sulfur_cube_knockback(&mut self, id: EntityId, power: f64, xd: f64, zd: f64, source: &DamageSource, damage: f32, comes_from_effect: bool) -> bool {
        let Some(source_entity) = source.causing_entity else { return false };
        if !self.sulfur_cube_has_body_item(id) {
            return false;
        }
        let sulfur_cube = self.sulfur_cube_ref(id);
        let mut horizontal_power = sulfur_cube.knockback_modifier.horizontal_power;
        let mut vertical_power = sulfur_cube.knockback_modifier.vertical_power;
        let original_horizontal_power = horizontal_power;
        let original_vertical_power = vertical_power;
        let hit_sound = sulfur_cube.sound_settings.hit_sound;
        let original_angle = Vec2::new(xd as f32, zd as f32);
        let attacker_eye = self.entity(source_entity).eye_position();
        let attacker_aim = self.entity(source_entity).look_angle().normalize();
        let target_center = self.entity(id).bounding_box().center();
        let new_angle = apply_horizontal_hit_angle_scale(HORIZONTAL_HIT_ANGLE_SCALE, original_angle, attacker_eye, attacker_aim, target_center);
        let bb_height = self.entity(id).dimensions.height;
        let new_power = apply_vertical_hit_angle_power_transfer(VERTICAL_HIT_ANGLE_SCALE, horizontal_power, vertical_power, attacker_eye, attacker_aim, target_center, bb_height);
        horizontal_power = new_power.x;
        vertical_power = new_power.y;
        let new_power = apply_vertical_position_angle_power_rotation(
            VERTICAL_POSITION_ANGLE_SCALE,
            horizontal_power,
            vertical_power,
            original_horizontal_power,
            original_vertical_power,
            self.entity(source_entity).position(),
            self.entity(id).position(),
        );
        horizontal_power = new_power.x;
        vertical_power = new_power.y;
        let (xd, zd) = (new_angle.x as f64, new_angle.y as f64);
        let power_multiplier = mth::sqrt(damage) * if comes_from_effect { power as f32 * 0.25 } else { 1.0 };
        horizontal_power *= power_multiplier;
        vertical_power *= power_multiplier;
        let knock_back_resistance = self.attribute_value(id, Attribute::KnockbackResistance);
        horizontal_power *= (1.0 - knock_back_resistance) as f32;
        vertical_power *= (1.0 - knock_back_resistance) as f32;
        let e = self.entity_mut(id);
        e.needs_sync = true;
        let delta_movement = e.delta_movement;
        horizontal_power *= 0.4;
        horizontal_power = horizontal_power.clamp(-128.0, 128.0);
        vertical_power = vertical_power.clamp(-128.0, 128.0);
        let horizontal_knockback = Vec3::new(xd, 0.0, zd).normalize().scale(horizontal_power as f64);
        e.set_delta_movement(Vec3::new(delta_movement.x - horizontal_knockback.x, delta_movement.y + vertical_power as f64 * 1.2, delta_movement.z - horizontal_knockback.z));
        self.entity_play_sound(id, sound_event(hit_sound), 1.0, 1.0);
        true
    }

    pub fn sulfur_cube_travel_in_fluid(&mut self, id: EntityId) {
        if self.sulfur_cube_has_body_item(id) && self.sulfur_cube_ref(id).floats_in_liquids {
            let e = self.entity(id);
            let vibe_amount = 0.2 * mth::sin((e.tick_count as f32 * 0.4) as f64);
            let fluid = if e.is_in_water() { &fluid_tags::WATER } else { &fluid_tags::LAVA };
            let immersion = e.fluid_height(fluid) - e.dimensions.height as f64 * 0.2 + vibe_amount as f64;
            if immersion > 0.0 {
                let e = self.entity_mut(id);
                e.set_delta_movement(e.delta_movement.add(0.0, immersion.min(1.0) * 0.04f32 as f64, 0.0));
            }
        }
    }

    pub fn sulfur_cube_look_control_tick(&mut self, id: EntityId) -> bool {
        if !self.sulfur_cube_has_body_item(id) {
            return false;
        }
        let e = self.entity_mut(id);
        let close_angle = mth::wrap_degrees_90(e.y_rot);
        e.set_y_rot(e.y_rot - close_angle);
        e.y_head_rot = e.y_rot;
        true
    }

    pub fn sulfur_cube_tempt_stop_navigation(&mut self, mob: EntityId) {
        self.set_wanted_movement(mob, 0.0);
    }

    pub fn sulfur_cube_tempt_navigate_towards(&mut self, mob: EntityId, player: EntityId) {
        self.look_at(mob, player, 10.0, 10.0);
        let y_rot = self.entity(mob).y_rot;
        self.set_direction(mob, y_rot, true);
    }

    pub fn sulfur_cube_dispense_block(&mut self, pos: BlockPos, dispensed: &mut ItemStack) -> bool {
        let aabb = wafer_core::AABB::new(pos.x as f64, pos.y as f64, pos.z as f64, (pos.x + 1) as f64, (pos.y + 1) as f64, (pos.z + 1) as f64);
        let cubes = self.get_entities_matching(None, &aabb, |level, entity| level.mob(entity).is_some_and(|mob| mob.sulfur_cube().is_some()));
        for entity in cubes {
            if self.sulfur_cube_equip_item(entity, dispensed) {
                dispensed.shrink(1);
                return true;
            }
        }
        false
    }

    pub fn try_ignite_explosive_entities(&mut self, pos: BlockPos) -> bool {
        let aabb = wafer_core::AABB::new(pos.x as f64, pos.y as f64, pos.z as f64, (pos.x + 1) as f64, (pos.y + 1) as f64, (pos.z + 1) as f64);
        let entities = self.get_entities_matching(None, &aabb, |level, entity| level.mob(entity).is_some_and(|mob| mob.sulfur_cube().is_some()) && level.sulfur_cube_can_explode(entity));
        let Some(&first) = entities.first() else { return false };
        self.sulfur_cube_prime_time(first, false);
        true
    }
}

fn apply_horizontal_hit_angle_scale(horizontal_angle_scale: f32, original_angle: Vec2, attacker_position: Vec3, attacker_aim_direction: Vec3, target_center: Vec3) -> Vec2 {
    let attacker_to_target = target_center.subtract(attacker_position).normalize();
    let angle_diff = (attacker_aim_direction.x * attacker_to_target.z - attacker_aim_direction.z * attacker_to_target.x)
        .atan2(attacker_aim_direction.x * attacker_to_target.x + attacker_aim_direction.z * attacker_to_target.z) as f32;
    original_angle.rotate((angle_diff * horizontal_angle_scale) as f64)
}

fn apply_vertical_hit_angle_power_transfer(
    vertical_hit_angle_scale: f32,
    horizontal_power: f32,
    vertical_power: f32,
    attacker_position: Vec3,
    attacker_aim_direction: Vec3,
    target_centered_position: Vec3,
    target_height: f32,
) -> Vec2 {
    let target_half_height = 0.5 * target_height;
    let target_top_pos = target_centered_position.add(0.0, target_half_height as f64, 0.0);
    let target_bottom_pos = target_centered_position.add(0.0, -target_half_height as f64, 0.0);
    let attacker_to_target_top = target_top_pos.subtract(attacker_position).normalize();
    let attacker_to_target_bottom = target_bottom_pos.subtract(attacker_position).normalize();
    let vertical_hit_angle_factor = mth::clamped_map(attacker_aim_direction.y, attacker_to_target_top.y, attacker_to_target_bottom.y, -1.0, 1.0) as f32;
    let mut transferred_power_ratio = (vertical_hit_angle_factor * vertical_hit_angle_scale).abs();
    if vertical_hit_angle_factor < 0.0 {
        transferred_power_ratio = -transferred_power_ratio;
    }
    let px = horizontal_power * (1.0 - transferred_power_ratio);
    let py = vertical_power * (1.0 + transferred_power_ratio);
    Vec2::new(px, py)
}

fn apply_vertical_position_angle_power_rotation(
    vertical_position_angle_scale: f32,
    horizontal_power: f32,
    vertical_power: f32,
    original_horizontal_power: f32,
    original_vertical_power: f32,
    attacker_feet_position: Vec3,
    target_feet_position: Vec3,
) -> Vec2 {
    let attacker_feet_to_target_feet = target_feet_position.subtract(attacker_feet_position);
    let vertical_position_angle = (-attacker_feet_to_target_feet.y).atan2(attacker_feet_to_target_feet.horizontal_distance()) as f32;
    let power_before_rotation = Vec2::new(horizontal_power, vertical_power);
    let mut rotated_power = power_before_rotation.rotate((-vertical_position_angle * vertical_position_angle_scale) as f64);
    let horizontal_ratio = if original_horizontal_power > 0.0 { rotated_power.x.abs() / original_horizontal_power } else { 0.0 };
    let vertical_ratio = if original_vertical_power > 0.0 { rotated_power.y.abs() / original_vertical_power } else { 0.0 };
    let max_ratio = horizontal_ratio.max(vertical_ratio);
    if max_ratio > 1.0 {
        rotated_power = rotated_power.scale(1.0 / max_ratio);
    }
    rotated_power
}

#[derive(Default)]
pub struct SulfurCubeSearchForItemsGoal {
    target_item: Option<EntityId>,
}

impl SulfurCubeSearchForItemsGoal {
    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        if level.is_baby(mob) || level.sulfur_cube_ref(mob).pickup_timer > 0 {
            return false;
        }
        let aabb = level.entity(mob).bounding_box().inflate(8.0, 8.0, 8.0);
        let candidates = level.get_entities_matching(None, &aabb, |level, entity| match level.find_entity(entity) {
            Some(LevelEntity::Item(item)) => !item.has_pickup_delay() && level.is_alive(entity) && is_swallowable_item(&item.item),
            _ => false,
        });
        let e = level.entity(mob);
        let (x, y, z) = (e.x(), e.y(), e.z());
        let mut best = -1.0;
        self.target_item = None;
        for entity in candidates {
            let dist = level.entity(entity).distance_to_sqr_xyz(x, y, z);
            if best == -1.0 || dist < best {
                best = dist;
                self.target_item = Some(entity);
            }
        }
        self.target_item.is_some()
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        if let Some(target_item) = self.target_item {
            level.look_at(mob, target_item, 10.0, 10.0);
        }
        let y_rot = level.entity(mob).y_rot;
        level.set_direction(mob, y_rot, true);
    }
}
