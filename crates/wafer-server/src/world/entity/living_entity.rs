use wafer_core::random::RandomSource;
use wafer_core::{AABB, BlockPos, Vec3, mth};
use wafer_data::packets::play::clientbound;
use wafer_data::properties::{HORIZONTAL_FACING, OCCUPIED, OPEN};
use wafer_protocol::nbt::Nbt;
use wafer_data::{BlockState, EquipmentSlot, EquipmentSlotGroup, MobEffect, block_tags, blocks, damage_type_tags, damage_types, entity_type_tags, fluid_tags, game_rule, item_tags, items, mob_effects};

use super::ai::brain::Brain;
use super::attributes::{Attribute, AttributeMap, AttributeModifier, AttributeSupplier, Operation, PLAYER_ATTRIBUTES, SPEED_MODIFIER_SPRINTING};
use super::data::{DataValue, Pose, ids};
use super::monster::ravager::Ravager;
use super::mob::{Mob, MobKind};
use super::using_item::FLAG_SPIN_ATTACK;
use super::{Entity, EntityId, MoverType, RemovalReason, compute_modified_friction, entity_event, flags, sound_event};
use crate::advancements::triggers::criteria_triggers;
use crate::core::holder_set::HolderSet;
use crate::network::connection::encode;
use crate::server::level::ServerLevel;
use crate::stats::stats;
use crate::world::difficulty::Difficulty;
use crate::world::item::component::codec as component_codec;
use crate::world::InteractionHand;
use crate::world::damagesource::DamageSource;
use crate::world::damagesource::combat_rules;
use crate::world::damagesource::combat_tracker::CombatTracker;
use crate::world::effect::MobEffectInstance;
use crate::world::effect::mob_effect::{add_attribute_modifiers, remove_attribute_modifiers, should_apply_effect_tick_this_tick};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::item::component::attack_range::AttackRange;
use crate::world::item::component::blocks_attacks::BlocksAttacks;
use crate::world::item::component::swing_animation::SwingAnimation;
use crate::world::item::enchantment::{enchantment, enchantment_helper};
use crate::world::level::BlockGetter;
use crate::world::level::block::abstract_bed_block;
use crate::world::level::update_flags::UPDATE_ALL;
use crate::world::level::clip::{ClipBlock, ClipContext, ClipFluid};
use crate::world::level::storage::loot::loot_params::LootParams;
use crate::world::level::storage::loot::loot_table::LootTable;
use crate::world::scores::team::CollisionRule;
use crate::world::waypoints::waypoint::Icon;
use crate::world::waypoints::waypoint_transmitter::{self, Connection, EntityAzimuthConnection, EntityBlockConnection, EntityChunkConnection};

const SPEED_MODIFIER_POWDER_SNOW_ID: &str = "minecraft:powder_snow";
const TICKS_REQUIRED_TO_FREEZE: i32 = 140;

pub struct LivingEntity {
    pub attributes: AttributeMap,
    pub y_body_rot: f32,
    pub y_body_rot_o: f32,
    pub y_head_rot_o: f32,
    pub no_action_time: i32,
    pub last_hurt_by_player: Option<u128>,
    pub last_hurt_by_player_memory_time: i32,
    pub last_hurt_by_mob: Option<u128>,
    pub last_hurt_by_mob_timestamp: i32,
    pub last_hurt_mob: Option<EntityId>,
    pub last_hurt_mob_timestamp: i32,
    pub jumping: bool,
    pub xxa: f32,
    pub yya: f32,
    pub zza: f32,
    pub speed: f32,
    pub no_jump_delay: i32,
    pub last_climbable_pos: Option<BlockPos>,
    pub current_impulse_impact_pos: Option<Vec3>,
    pub current_impulse_context_reset_grace_time: i32,
    pub last_pos: Option<BlockPos>,
    active_location_dependent_enchantments: [Vec<(&'static enchantment::Enchantment, Vec<usize>)>; 8],
    pub hurt_time: i32,
    pub hurt_duration: i32,
    pub death_time: i32,
    pub damage_cooldown_time: i32,
    pub last_hurt: f32,
    pub dead: bool,
    pub last_damage_source: Option<DamageSource>,
    pub last_damage_stamp: i64,
    pub combat_tracker: CombatTracker,
    pub hurt_dir: f32,
    pub use_item: Option<ItemStack>,
    pub use_item_remaining: i32,
    pub auto_spin_attack_ticks: i32,
    pub auto_spin_attack_dmg: f32,
    pub auto_spin_attack_item_stack: Option<ItemStack>,
    pub remove_arrow_time: i32,
    pub fall_fly_ticks: i32,
    pub last_equipment_items: [Option<ItemStack>; 8],
    pub swing_state: SwingState,
    pub active_effects: Vec<MobEffectInstance>,
    pub effects_dirty: bool,
    pub effects_mod_count: u32,
    pub locator_bar_icon: Icon,
    pub discard_friction: bool,
    pub brain: Brain,
}

impl LivingEntity {
    pub fn player() -> LivingEntity {
        LivingEntity::new(&PLAYER_ATTRIBUTES)
    }

    pub fn new(attributes: &'static AttributeSupplier) -> LivingEntity {
        LivingEntity {
            attributes: AttributeMap::new(attributes),
            y_body_rot: 0.0,
            y_body_rot_o: 0.0,
            y_head_rot_o: 0.0,
            no_action_time: 0,
            last_hurt_by_player: None,
            last_hurt_by_player_memory_time: 0,
            last_hurt_by_mob: None,
            last_hurt_by_mob_timestamp: 0,
            last_hurt_mob: None,
            last_hurt_mob_timestamp: 0,
            jumping: false,
            xxa: 0.0,
            yya: 0.0,
            zza: 0.0,
            speed: 0.0,
            no_jump_delay: 0,
            last_climbable_pos: None,
            current_impulse_impact_pos: None,
            current_impulse_context_reset_grace_time: 0,
            last_pos: None,
            active_location_dependent_enchantments: Default::default(),
            hurt_time: 0,
            hurt_duration: 0,
            death_time: 0,
            damage_cooldown_time: 0,
            last_hurt: 0.0,
            dead: false,
            last_damage_source: None,
            last_damage_stamp: 0,
            combat_tracker: Default::default(),
            hurt_dir: 0.0,
            use_item: None,
            use_item_remaining: 0,
            auto_spin_attack_ticks: 0,
            auto_spin_attack_dmg: 0.0,
            auto_spin_attack_item_stack: None,
            remove_arrow_time: 0,
            fall_fly_ticks: 0,
            last_equipment_items: Default::default(),
            swing_state: SwingState::default(),
            active_effects: Vec::new(),
            effects_dirty: true,
            effects_mod_count: 0,
            locator_bar_icon: Icon::new(),
            discard_friction: false,
            brain: Brain::new(),
        }
    }

    pub fn set_ignore_fall_damage_from_current_impulse(&mut self, ignore_fall_damage: bool, new_impulse_impact_pos: Vec3) {
        if ignore_fall_damage {
            self.apply_post_impulse_grace_time(40);
            self.current_impulse_impact_pos = Some(new_impulse_impact_pos);
        } else {
            self.current_impulse_context_reset_grace_time = 0;
        }
    }

    pub fn apply_post_impulse_grace_time(&mut self, ticks: i32) {
        self.current_impulse_context_reset_grace_time = self.current_impulse_context_reset_grace_time.max(ticks);
    }

    pub fn active_location_dependent_effects(&self, slot: EquipmentSlot, enchantment: &'static enchantment::Enchantment) -> Option<&Vec<usize>> {
        self.active_location_dependent_enchantments[slot as usize].iter().find(|(e, _)| std::ptr::eq(*e, enchantment)).map(|(_, effects)| effects)
    }

    pub fn add_active_location_dependent_effect(&mut self, slot: EquipmentSlot, enchantment: &'static enchantment::Enchantment, effect: usize) {
        let active = &mut self.active_location_dependent_enchantments[slot as usize];
        match active.iter_mut().find(|(e, _)| std::ptr::eq(*e, enchantment)) {
            Some((_, effects)) => effects.push(effect),
            None => active.push((enchantment, vec![effect])),
        }
    }

    pub fn remove_active_location_dependent_effect(&mut self, slot: EquipmentSlot, enchantment: &'static enchantment::Enchantment, effect: usize) -> bool {
        let Some((_, effects)) = self.active_location_dependent_enchantments[slot as usize].iter_mut().find(|(e, _)| std::ptr::eq(*e, enchantment)) else { return false };
        let Some(index) = effects.iter().position(|&e| e == effect) else { return false };
        effects.remove(index);
        true
    }

    pub fn remove_active_location_dependent_enchantment(&mut self, slot: EquipmentSlot, enchantment: &'static enchantment::Enchantment) -> Option<Vec<usize>> {
        let active = &mut self.active_location_dependent_enchantments[slot as usize];
        let index = active.iter().position(|(e, _)| std::ptr::eq(*e, enchantment))?;
        Some(active.remove(index).1)
    }

    pub fn reset_current_impulse_context(&mut self) {
        self.current_impulse_context_reset_grace_time = 0;
        self.current_impulse_impact_pos = None;
    }

    pub fn try_reset_current_impulse_context(&mut self) {
        if self.current_impulse_context_reset_grace_time == 0 {
            self.reset_current_impulse_context();
        }
    }

    pub fn is_in_post_impulse_grace_time(&self) -> bool {
        self.current_impulse_context_reset_grace_time > 0
    }

    fn effect_index(&self, effect: MobEffect) -> Result<usize, usize> {
        self.active_effects.binary_search_by_key(&effect, |e| e.effect)
    }

    pub fn has_effect(&self, effect: MobEffect) -> bool {
        self.effect_index(effect).is_ok()
    }

    pub fn effect(&self, effect: MobEffect) -> Option<&MobEffectInstance> {
        self.effect_index(effect).ok().map(|i| &self.active_effects[i])
    }

    fn remove_effect_no_update(&mut self, effect: MobEffect) -> Option<MobEffectInstance> {
        let index = self.effect_index(effect).ok()?;
        self.effects_mod_count += 1;
        Some(self.active_effects.remove(index))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SwingDescription {
    pub hand: InteractionHand,
    pub animation: SwingAnimation,
    pub duration_ticks: i32,
}

#[derive(Clone, Copy, Default, Debug)]
pub struct SwingState {
    pub current_swing: Option<SwingDescription>,
    ticks: i32,
    old_animation: f32,
    pub animation: f32,
}

impl SwingState {
    pub fn start_if_able(&mut self, hand: InteractionHand, animation: SwingAnimation, duration_ticks: i32) -> bool {
        if self.current_swing.is_some_and(|s| self.ticks <= s.duration_ticks / 2) && self.ticks > 0 {
            return false;
        }
        let old_swing = self.current_swing;
        let swing = SwingDescription { hand, animation, duration_ticks };
        self.current_swing = Some(swing);
        if old_swing != Some(swing) {
            self.old_animation = 0.0;
            self.animation = 0.0;
        }
        self.ticks = 0;
        true
    }

    pub fn tick(&mut self) {
        self.old_animation = self.animation;
        match self.current_swing {
            Some(swing) => {
                if swing.duration_ticks > 0 {
                    self.animation = (self.ticks as f32 / swing.duration_ticks as f32).min(1.0);
                }
                let ticks = self.ticks;
                self.ticks += 1;
                if ticks > swing.duration_ticks {
                    self.current_swing = None;
                    self.animation = 0.0;
                }
            }
            None => self.animation = 0.0,
        }
    }

    pub fn is_swinging(&self) -> bool {
        self.current_swing.is_some()
    }
}

pub fn get_comfortable_fall_distance(allowed_damage: f32) -> i32 {
    mth::floor_f(allowed_damage + 3.0)
}

pub fn can_glide_using(item_stack: &ItemStack, slot: EquipmentSlot) -> bool {
    if !item_stack.has(dc::GLIDER) {
        return false;
    }
    let Some(equippable) = item_stack.get(dc::EQUIPPABLE).map(dc::values::Equippable::from_nbt) else { return false };
    EquipmentSlot::by_name(&equippable.slot) == Some(slot) && !item_stack.next_damage_will_break()
}

pub fn entity_event_for_equipment_break(slot: EquipmentSlot) -> u8 {
    match slot {
        EquipmentSlot::Mainhand => entity_event::MAINHAND_BREAK,
        EquipmentSlot::Offhand => entity_event::OFFHAND_BREAK,
        EquipmentSlot::Head => entity_event::HEAD_BREAK,
        EquipmentSlot::Chest => entity_event::CHEST_BREAK,
        EquipmentSlot::Feet => entity_event::FEET_BREAK,
        EquipmentSlot::Legs => entity_event::LEGS_BREAK,
        EquipmentSlot::Body => entity_event::BODY_BREAK,
        EquipmentSlot::Saddle => entity_event::SADDLE_BREAK,
    }
}

fn for_each_modifier(stack: &ItemStack, slot: EquipmentSlot, mut f: impl FnMut(Attribute, AttributeModifier)) {
    if let Some(modifiers) = stack.get(dc::ATTRIBUTE_MODIFIERS) {
        for entry in modifiers.as_list() {
            let group = entry.get("slot").map_or(Ok(EquipmentSlotGroup::Any), enchantment::slot_group);
            if !group.is_ok_and(|group| group.test(slot)) {
                continue;
            }
            let attribute = entry.get("type").and_then(Nbt::as_str).and_then(Attribute::by_name);
            let id = entry.get("id").and_then(Nbt::as_str);
            let operation = entry.get("operation").and_then(Nbt::as_str).and_then(Operation::by_name);
            let amount = entry.get("amount").map(crate::world::item::component::codec::as_f64);
            if let (Some(attribute), Some(id), Some(operation), Some(amount)) = (attribute, id, operation, amount) {
                f(attribute, AttributeModifier { id: std::borrow::Cow::Owned(id.to_string()), amount, operation });
            }
        }
    }
    enchantment_helper::for_each_modifier(stack, slot, f);
}

pub fn set_equipment(id: EntityId, slots: &[(EquipmentSlot, Option<ItemStack>)]) -> Vec<u8> {
    encode(clientbound::SET_EQUIPMENT, |p| {
        p.varint(id);
        for (i, (slot, stack)) in slots.iter().enumerate() {
            let last = i == slots.len() - 1;
            p.u8(*slot as u8 | if last { 0 } else { 0x80 });
            crate::world::item::write_slot(p, stack.as_ref());
        }
    })
}

pub fn update_mob_effect(entity_id: EntityId, effect: &MobEffectInstance, blend: bool) -> Vec<u8> {
    encode(clientbound::UPDATE_MOB_EFFECT, |p| {
        p.varint(entity_id);
        p.varint(effect.effect.protocol_id());
        p.varint(effect.amplifier);
        p.varint(effect.duration);
        p.u8(effect.ambient as u8 | (effect.visible as u8) << 1 | (effect.show_icon as u8) << 2 | (blend as u8) << 3);
    })
}

pub fn remove_mob_effect(entity_id: EntityId, effect: MobEffect) -> Vec<u8> {
    encode(clientbound::REMOVE_MOB_EFFECT, |p| {
        p.varint(entity_id);
        p.varint(effect.protocol_id());
    })
}

fn are_all_effects_ambient(effects: &[MobEffectInstance]) -> bool {
    effects.iter().all(|e| !e.visible || e.ambient)
}

impl ServerLevel {
    pub fn living(&self, id: EntityId) -> &LivingEntity {
        match self.players.get(&id) {
            Some(p) => &p.living,
            None => &self.mob(id).expect("a living entity").living,
        }
    }

    pub fn living_mut(&mut self, id: EntityId) -> &mut LivingEntity {
        if self.players.contains_key(&id) {
            return &mut self.player_mut(id).living;
        }
        &mut self.mob_mut(id).expect("a living entity").living
    }

    pub fn entity_and_living(&self, id: EntityId) -> (&Entity, &LivingEntity) {
        match self.players.get(&id) {
            Some(p) => (&p.entity, &p.living),
            None => {
                let mob = self.mob(id).expect("a living entity");
                (&mob.entity, &mob.living)
            }
        }
    }

    pub fn entity_and_living_mut(&mut self, id: EntityId) -> (&mut Entity, &mut LivingEntity) {
        if self.players.contains_key(&id) {
            let p = self.player_mut(id);
            return (&mut p.entity, &mut p.living);
        }
        let mob = self.mob_mut(id).expect("a living entity");
        (&mut mob.entity, &mut mob.living)
    }

    pub fn health(&self, id: EntityId) -> f32 {
        match self.players.get(&id) {
            Some(p) => p.health,
            None => self.mob(id).expect("a living entity").health,
        }
    }

    pub fn item_by_slot(&self, id: EntityId, slot: EquipmentSlot) -> Option<ItemStack> {
        if let Some(mob) = self.mob(id) {
            return mob.item_by_slot(slot).cloned();
        }
        let index = self.equipment_inventory_slot(id, slot)?;
        self.player(id).inventory.slots[index].clone().filter(|s| !s.is_empty())
    }

    pub fn get_item_in_hand(&self, id: EntityId, hand: InteractionHand) -> Option<ItemStack> {
        match self.mob(id) {
            Some(mob) => mob.item_by_slot(hand.as_equipment_slot()).cloned(),
            None => self.player(id).item_in_hand(hand),
        }
    }

    pub fn set_item_in_hand(&mut self, id: EntityId, hand: InteractionHand, item_stack: ItemStack) {
        if self.is_player(id) {
            self.player_mut(id).set_item_in_hand(hand, Some(item_stack));
        } else {
            self.living_set_item_slot(id, hand.as_equipment_slot(), item_stack);
        }
    }

    pub fn is_holding(&self, id: EntityId, item: wafer_data::Item) -> bool {
        [EquipmentSlot::Mainhand, EquipmentSlot::Offhand].into_iter().any(|slot| self.item_by_slot_ref(id, slot).is_some_and(|held_item| held_item.is(item)))
    }

    pub fn get_ticks_using_item(&self, id: EntityId) -> i32 {
        if !self.is_using_item(id) {
            return 0;
        }
        let living = self.living(id);
        living.use_item.as_ref().map_or(0, crate::world::item::get_use_duration) - living.use_item_remaining
    }

    pub fn on_equip_item(&mut self, id: EntityId, slot: EquipmentSlot, old_stack: &ItemStack, stack: &ItemStack) {
        if self.mob(id).is_some_and(Mob::is_abstract_skeleton) {
            self.abstract_skeleton_on_equip_item(id, slot, old_stack, stack);
        } else {
            self.living_entity_on_equip_item(id, slot, old_stack, stack);
        }
    }

    pub fn living_entity_on_equip_item(&mut self, id: EntityId, slot: EquipmentSlot, old_stack: &ItemStack, stack: &ItemStack) {
        if self.is_spectator(id) || ItemStack::is_same_item_same_components(old_stack, stack) || self.entity(id).first_tick {
            return;
        }
        let Some(equippable) = stack.get(crate::world::item::component::EQUIPPABLE).map(crate::world::item::component::values::Equippable::from_nbt) else { return };
        if !self.entity(id).is_silent() && EquipmentSlot::by_name(&equippable.slot) == Some(slot) {
            let Some(sound) = self.get_equip_sound(id, slot, &equippable) else { return };
            let source = self.sound_source(id);
            let e = self.entity_mut(id);
            let (x, y, z) = (e.x(), e.y(), e.z());
            let seed = e.random.next_long();
            self.play_seeded_sound_at(None, x, y, z, sound, source, 1.0, 1.0, seed);
        }
    }

    pub fn can_use_slot(&self, id: EntityId, slot: EquipmentSlot) -> bool {
        match self.mob(id).map(|mob| &mob.kind) {
            Some(MobKind::Pig(_)) => self.pig_can_use_slot(id, slot),
            Some(MobKind::Horse(_)) => true,
            Some(MobKind::Donkey(_) | MobKind::Mule(_)) => self.abstract_horse_can_use_slot(id, slot),
            _ => true,
        }
    }

    fn get_equip_sound(&self, id: EntityId, slot: EquipmentSlot, equippable: &crate::world::item::component::values::Equippable) -> Option<crate::sounds::sound_event::SoundEvent> {
        if slot == EquipmentSlot::Saddle && matches!(self.mob(id).map(|mob| &mob.kind), Some(MobKind::Pig(_))) {
            return Some(sound_event("minecraft:entity.pig.saddle").into());
        }
        if slot == EquipmentSlot::Saddle && self.is_abstract_horse(id) {
            return self.get_equip_sound_for_saddle(id).map(Into::into);
        }
        crate::sounds::sound_event::SoundEvent::from_nbt(&equippable.equip_sound)
    }

    pub fn get_equipment_slot_for_item(&self, id: EntityId, item_stack: &ItemStack) -> EquipmentSlot {
        let equippable = item_stack.get(crate::world::item::component::EQUIPPABLE).map(crate::world::item::component::values::Equippable::from_nbt);
        match equippable.and_then(|equippable| EquipmentSlot::by_name(&equippable.slot)) {
            Some(slot) if self.can_use_slot(id, slot) => slot,
            _ => EquipmentSlot::Mainhand,
        }
    }

    pub fn is_equippable_in_slot(&self, id: EntityId, item_stack: &ItemStack, slot: EquipmentSlot) -> bool {
        let Some(equippable) = item_stack.get(crate::world::item::component::EQUIPPABLE).map(crate::world::item::component::values::Equippable::from_nbt) else {
            return slot == EquipmentSlot::Mainhand && self.can_use_slot(id, EquipmentSlot::Mainhand);
        };
        let entity_type = self.entity_ref(id).map_or(-1, |e| e.entity_type);
        EquipmentSlot::by_name(&equippable.slot) == Some(slot) && self.can_use_slot(id, slot) && equippable.can_be_equipped_by(entity_type)
    }

    pub fn living_set_item_slot(&mut self, id: EntityId, slot: EquipmentSlot, item_stack: ItemStack) {
        let old_stack = self.item_by_slot(id, slot).unwrap_or_else(ItemStack::empty);
        self.set_item_slot(id, slot, item_stack.clone());
        self.on_equip_item(id, slot, &old_stack, &item_stack);
    }

    pub fn set_item_slot(&mut self, id: EntityId, slot: EquipmentSlot, item_stack: ItemStack) {
        if let Some(mob) = self.mob_mut(id) {
            mob.equipment[slot as usize] = Some(item_stack).filter(|s| !s.is_empty());
            return;
        }
        let Some(index) = self.equipment_inventory_slot(id, slot) else { return };
        let p = self.player_mut(id);
        p.inventory.slots[index] = Some(item_stack).filter(|s| !s.is_empty());
    }

    pub fn get_arrow_count(&self, id: EntityId) -> i32 {
        match self.entity(id).data.get(ids::ARROW_COUNT) {
            DataValue::Int(count) => *count,
            _ => 0,
        }
    }

    pub fn set_arrow_count(&mut self, id: EntityId, count: i32) {
        self.entity_mut(id).data.set(ids::ARROW_COUNT, DataValue::Int(count));
    }

    pub fn is_affected_by_potions(&self, id: EntityId) -> bool {
        !self.is_dead_or_dying(id)
    }

    pub fn is_sensitive_to_water(&self, id: EntityId) -> bool {
        self.is_living(id)
            && matches!(wafer_data::registry::ENTITY_TYPE[self.entity(id).entity_type as usize], "minecraft:blaze" | "minecraft:enderman" | "minecraft:snow_golem" | "minecraft:strider")
    }

    pub fn random_teleport(&mut self, id: EntityId, xx: f64, yy: f64, zz: f64, show_particles: bool, is_invalid_position: &dyn Fn(BlockState) -> bool) -> bool {
        let mut y = yy;
        let clamped = self.world_border.clamp_vec3_to_bound(xx, y, zz);
        let mut pos = BlockPos::containing_vec(clamped);
        if self.has_chunk_at(pos) {
            while pos.y > self.get_min_y() {
                pos = pos.below();
                let state = self.block_state(pos);
                if state.is(&block_tags::ENTITIES_CAN_TELEPORT_TO) {
                    return self.check_position_and_teleport(id, clamped.x, y, clamped.z, show_particles, is_invalid_position, state);
                }
                y -= 1.0;
            }
        }
        false
    }

    #[allow(clippy::too_many_arguments)]
    fn check_position_and_teleport(&mut self, id: EntityId, xx: f64, y: f64, zz: f64, show_particles: bool, is_invalid_position: &dyn Fn(BlockState) -> bool, state: BlockState) -> bool {
        if is_invalid_position(state) {
            return false;
        }
        let aabb = self.entity(id).dimensions.make_bounding_box(Vec3::new(xx, y, zz));
        if self.no_collision(&aabb) && self.entity_collisions(None, &aabb).is_empty() && !self.contains_any_liquid(&aabb) {
            let from = BlockPos::containing(aabb.min_x, aabb.min_y, aabb.min_z);
            let to = BlockPos::containing(aabb.max_x, aabb.max_y, aabb.max_z);
            let has_invalid_position = self.find_blocks_in(from, to).filter_state(is_invalid_position).any_matched(self);
            if has_invalid_position {
                return false;
            }
            if self.can_randomly_teleport_to(id, xx, y, zz) {
                self.teleport_to(id, xx, y, zz);
                if show_particles {
                    self.broadcast_entity_event(id, entity_event::TELEPORT);
                }
                if self.is_mob(id) {
                    self.get_navigation(id, |navigation, _| navigation.stop());
                }
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    pub fn can_randomly_teleport_to(&mut self, id: EntityId, x: f64, y: f64, z: f64) -> bool {
        match self.mob(id).map(|mob| &mob.kind) {
            Some(MobKind::Enderman(_)) => self.enderman_can_randomly_teleport_to(x, y, z),
            _ => true,
        }
    }

    pub fn on_item_pickup(&mut self, id: EntityId, item_entity: EntityId) {
        let Some(super::LevelEntity::Item(item)) = self.entities.get(&item_entity) else { return };
        let item_stack = item.item.clone();
        let owner = item.thrower.and_then(|uuid| self.entity_manager.visible_entity_storage.get(uuid)).filter(|&owner| self.is_player(owner));
        if let Some(owner) = owner {
            crate::advancements::triggers::criteria_triggers::THROWN_ITEM_PICKED_UP_BY_ENTITY.trigger(self, owner, &item_stack, id);
        }
    }

    pub fn set_last_hurt_by_mob(&mut self, id: EntityId, hurt_by: Option<EntityId>) {
        match self.mob(id).map(|mob| &mob.kind) {
            Some(MobKind::Villager(_)) => self.villager_set_last_hurt_by_mob(id, hurt_by),
            _ => self.living_entity_set_last_hurt_by_mob(id, hurt_by),
        }
    }

    pub fn living_entity_set_last_hurt_by_mob(&mut self, id: EntityId, hurt_by: Option<EntityId>) {
        let uuid = hurt_by.map(|hurt_by| self.entity(hurt_by).uuid);
        let tick_count = self.entity(id).tick_count;
        let living = self.living_mut(id);
        living.last_hurt_by_mob = uuid;
        living.last_hurt_by_mob_timestamp = tick_count;
    }

    pub fn set_last_hurt_mob(&mut self, id: EntityId, target: EntityId) {
        let target = self.is_living(target).then_some(target);
        let tick_count = self.entity(id).tick_count;
        let living = self.living_mut(id);
        living.last_hurt_mob = target;
        living.last_hurt_mob_timestamp = tick_count;
    }

    pub fn get_knockback(&mut self, id: EntityId, target: EntityId, damage_source: &DamageSource) -> f32 {
        let knockback = self.attribute_value(id, Attribute::AttackKnockback) as f32;
        let weapon = self.get_weapon_item(id);
        self.modify_knockback(&weapon, target, damage_source, knockback) / 2.0
    }

    pub fn post_piercing_attack(&mut self, id: EntityId) {
        self.do_post_piercing_attack_effects(id);
    }

    pub fn get_attack_range_with(&self, id: EntityId, weapon_item: &ItemStack) -> AttackRange {
        match weapon_item.get(dc::ATTACK_RANGE) {
            Some(attack_range) => AttackRange::from_nbt(attack_range),
            None => AttackRange::default_for(self.attribute_value(id, Attribute::EntityInteractionRange) as f32),
        }
    }

    pub fn get_weapon_item(&self, id: EntityId) -> ItemStack {
        if self.is_player(id)
            && let Some(item_stack) = self.player_get_weapon_item(id)
        {
            return item_stack;
        }
        self.item_by_slot(id, EquipmentSlot::Mainhand).unwrap_or_else(ItemStack::empty)
    }

    pub fn get_active_item(&self, id: EntityId) -> ItemStack {
        if self.is_spectator(id) {
            ItemStack::empty()
        } else if self.is_using_item(id) {
            self.get_use_item(id)
        } else {
            self.get_weapon_item(id)
        }
    }

    pub fn get_use_item(&self, id: EntityId) -> ItemStack {
        self.living(id).use_item.clone().unwrap_or_else(ItemStack::empty)
    }

    pub fn get_item_blocking_with(&self, id: EntityId) -> Option<ItemStack> {
        if !self.is_using_item(id) {
            return None;
        }
        let use_item = self.living(id).use_item.as_ref()?;
        let blocks_attacks = BlocksAttacks::from_nbt(use_item.get(dc::BLOCKS_ATTACKS)?);
        let elapsed_ticks = crate::world::item::get_use_duration(use_item) - self.living(id).use_item_remaining;
        (elapsed_ticks >= blocks_attacks.block_delay_ticks()).then(|| use_item.clone())
    }

    pub fn get_seconds_to_disable_blocking(&self, id: EntityId) -> f32 {
        let weapon_item = self.get_weapon_item(id);
        match weapon_item.get(dc::WEAPON) {
            Some(weapon) if weapon_item == self.get_active_item(id) => weapon.get("disable_blocking_for_seconds").map_or(0.0, component_codec::as_f32),
            _ => 0.0,
        }
    }

    pub fn hurt_and_break(&mut self, owner: EntityId, slot: EquipmentSlot, amount: i32) {
        let Some(mut item_stack) = self.item_by_slot(owner, slot) else { return };
        let new_amount = if !item_stack.is_damageable_item() || self.players.get(&owner).is_some_and(|p| p.abilities.instabuild) {
            0
        } else if amount > 0 {
            self.process_durability_change(&item_stack, amount)
        } else {
            amount
        };
        if new_amount == 0 {
            return;
        }
        let new_damage = item_stack.damage_value() + new_amount;
        if self.is_player(owner) {
            criteria_triggers::ITEM_DURABILITY_CHANGED.trigger(self, owner, &item_stack, new_damage);
        }
        item_stack.set_damage_value(new_damage);
        if item_stack.is_broken() {
            let broken = item_stack.copy();
            item_stack.shrink(1);
            self.set_item_slot(owner, slot, item_stack);
            self.on_equipped_item_broken(owner, &broken, slot);
        } else {
            self.set_item_slot(owner, slot, item_stack);
        }
    }

    pub fn on_equipped_item_broken(&mut self, id: EntityId, broken_item: &ItemStack, in_slot: EquipmentSlot) {
        self.broadcast_entity_event(id, entity_event_for_equipment_break(in_slot));
        self.stop_location_based_effects(id, broken_item, in_slot);
        if self.is_player(id) {
            self.award_stat(id, stats::ITEM_BROKEN.get(broken_item.get_item().0), 1);
        }
    }

    pub fn max_health(&self, id: EntityId) -> f32 {
        self.attribute_value(id, Attribute::MaxHealth) as f32
    }

    pub fn max_absorption(&self, id: EntityId) -> f32 {
        self.attribute_value(id, Attribute::MaxAbsorption) as f32
    }

    pub fn absorption_amount(&self, id: EntityId) -> f32 {
        if let Some(mob) = self.mob(id) {
            return mob.absorption;
        }
        match self.player(id).entity.data.get(ids::PLAYER_ABSORPTION) {
            DataValue::Float(v) => *v,
            _ => 0.0,
        }
    }

    pub fn set_absorption_amount(&mut self, id: EntityId, amount: f32) {
        let amount = mth::clamp_f(amount, 0.0, self.max_absorption(id));
        if let Some(mob) = self.mob_mut(id) {
            mob.absorption = amount;
            return;
        }
        self.player_mut(id).entity.data.set(ids::PLAYER_ABSORPTION, DataValue::Float(amount));
    }

    pub fn set_health(&mut self, id: EntityId, health: f32) {
        let health = mth::clamp_f(health, 0.0, self.max_health(id));
        if let Some(mob) = self.mob_mut(id) {
            mob.health = health;
            mob.entity.data.set(ids::HEALTH, DataValue::Float(health));
            return;
        }
        let p = self.player_mut(id);
        p.health = health;
        p.entity.data.set(ids::HEALTH, DataValue::Float(health));
    }

    pub fn heal(&mut self, id: EntityId, amount: f32) {
        let health = self.health(id);
        if health > 0.0 {
            self.set_health(id, health + amount);
        }
    }

    pub fn is_dead_or_dying(&self, id: EntityId) -> bool {
        self.health(id) <= 0.0 || self.living(id).dead
    }

    pub fn is_baby(&self, id: EntityId) -> bool {
        self.mob(id).is_some_and(|mob| mob.is_baby())
    }

    pub fn armor_value(&self, id: EntityId) -> i32 {
        mth::floor(self.attribute_value(id, Attribute::Armor))
    }

    pub fn has_line_of_sight(&mut self, id: EntityId, target: EntityId) -> bool {
        if self.is_ravager(id) && !Ravager::has_line_of_sight(self.mob(id).expect("a ravager")) {
            return false;
        }
        let eye_height = self.entity(target).eye_y();
        self.has_line_of_sight_with(id, target, ClipBlock::Collider, ClipFluid::None, eye_height)
    }

    pub fn has_line_of_sight_with(&mut self, id: EntityId, target: EntityId, block_colliding_context: ClipBlock, fluid_colliding_context: ClipFluid, eye_height: f64) -> bool {
        let e = self.entity(id);
        let from = Vec3::new(e.x(), e.eye_y(), e.z());
        let t = self.entity(target);
        let to = Vec3::new(t.x(), eye_height, t.z());
        if to.distance_to(from) > 128.0 {
            return false;
        }
        let ctx = ClipContext { from, to, block: block_colliding_context, fluid: fluid_colliding_context, is_player: self.is_player(id) };
        self.clip(&ctx).miss
    }

    pub fn player_not_wearing_disguise_item(&self, id: EntityId) -> bool {
        if self.is_player(id) {
            !self.item_by_slot_ref(id, EquipmentSlot::Head).is_some_and(|helmet| helmet.get_item().is(&item_tags::GAZE_DISGUISE_EQUIPMENT))
        } else {
            true
        }
    }

    pub fn get_view_vector(&self, id: EntityId) -> Vec3 {
        let e = self.entity(id);
        let view_y_rot = if self.is_living(id) { e.y_head_rot } else { e.y_rot };
        super::calculate_view_vector(e.x_rot, view_y_rot)
    }

    pub fn is_looking_at_me(&mut self, id: EntityId, target: EntityId, cone_size: f64, adjust_for_distance: bool, see_through_transparent_blocks: bool, gaze_heights: &[f64]) -> bool {
        let look = self.get_view_vector(target).normalize();
        for &gaze_height in gaze_heights {
            let (e, t) = (self.entity(id), self.entity(target));
            let dir = Vec3::new(e.x() - t.x(), gaze_height - t.eye_y(), e.z() - t.z());
            let dist = dir.length();
            let dir = dir.normalize();
            let dot = look.dot(dir);
            let block = if see_through_transparent_blocks { ClipBlock::Visual } else { ClipBlock::Collider };
            if dot > 1.0 - cone_size / if adjust_for_distance { dist } else { 1.0 } && self.has_line_of_sight_with(target, id, block, ClipFluid::None, gaze_height) {
                return true;
            }
        }
        false
    }

    pub fn get_visibility_percent(&self, id: EntityId, targeting_entity: Option<EntityId>) -> f64 {
        let mut visibility_percent = 1.0;
        let e = self.entity(id);
        if e.is_discrete() {
            visibility_percent *= 0.8;
        }
        if e.shared_flag(flags::INVISIBLE) {
            let mut cover_percentage = self.get_armor_cover_percentage(id);
            if cover_percentage < 0.1 {
                cover_percentage = 0.1;
            }
            visibility_percent *= 0.7 * cover_percentage as f64;
        }
        if let Some(targeting_entity) = targeting_entity {
            let targeting_type = self.entity(targeting_entity).entity_type;
            for slot in EquipmentSlot::VALUES {
                let Some(item) = self.item_by_slot_ref(id, slot) else { continue };
                let Some(mob_visibility) = item.get(dc::MOB_VISIBILITY) else { continue };
                let equippable_slot = item.get(dc::EQUIPPABLE).and_then(|equippable| equippable.get("slot")).and_then(Nbt::as_str).and_then(EquipmentSlot::by_name);
                let targets = mob_visibility
                    .get("targeting_entity_types")
                    .and_then(|types| HolderSet::decode("minecraft:entity_type", types).ok())
                    .is_some_and(|types| types.contains(targeting_type));
                if equippable_slot == Some(slot) && targets {
                    visibility_percent *= mob_visibility.get("visibility").map_or(0.0, component_codec::as_f32) as f64;
                }
            }
        }
        visibility_percent.clamp(0.0, 10.0)
    }

    pub fn get_armor_cover_percentage(&self, id: EntityId) -> f32 {
        let mut total = 0;
        let mut count = 0;
        for slot in [EquipmentSlot::Feet, EquipmentSlot::Legs, EquipmentSlot::Chest, EquipmentSlot::Head] {
            if self.item_by_slot_ref(id, slot).is_some() {
                count += 1;
            }
            total += 1;
        }
        if total > 0 { count as f32 / total as f32 } else { 0.0 }
    }

    pub fn can_attack(&self, id: EntityId, target: EntityId) -> bool {
        if self.is_tamable_animal(id) {
            return self.tamable_animal_can_attack(id, target);
        }
        match self.mob(id).map(|mob| &mob.kind) {
            Some(MobKind::IronGolem(_)) => self.iron_golem_can_attack(id, target),
            Some(MobKind::Vindicator(_) | MobKind::Pillager(_) | MobKind::Evoker(_) | MobKind::Illusioner(_)) => self.abstract_illager_can_attack(id, target),
            Some(_) => self.mob_can_attack(id, target),
            None => self.living_entity_can_attack(id, target),
        }
    }

    pub fn living_entity_can_attack(&self, _id: EntityId, target: EntityId) -> bool {
        if self.is_player(target) && self.difficulty == Difficulty::Peaceful { false } else { self.can_be_seen_as_enemy(target) }
    }

    pub fn living_entity_cause_extra_knockback(&mut self, id: EntityId, target: EntityId, knockback: f32, _old_movement: Vec3) {
        if knockback > 0.0 && self.is_living(target) {
            let y_rot = self.entity(id).y_rot * mth::DEG_TO_RAD;
            let (sin, cos) = (mth::sin(y_rot as f64) as f64, mth::cos(y_rot as f64) as f64);
            self.knockback(target, knockback as f64, sin, -cos);
            let e = self.entity_mut(id);
            e.set_delta_movement(e.delta_movement.multiply(0.6, 1.0, 0.6));
        }
    }

    pub fn can_be_seen_as_enemy(&self, id: EntityId) -> bool {
        if matches!(self.mob(id).map(|mob| &mob.kind), Some(MobKind::Axolotl(_))) {
            return self.axolotl_can_be_seen_as_enemy(id);
        }
        self.living_entity_can_be_seen_as_enemy(id)
    }

    pub fn living_entity_can_be_seen_as_enemy(&self, id: EntityId) -> bool {
        if self.is_player(id) && self.player(id).abilities.invulnerable {
            return false;
        }
        !self.entity(id).is_invulnerable() && self.can_be_seen_by_anyone(id)
    }

    pub fn can_be_seen_by_anyone(&self, id: EntityId) -> bool {
        !self.is_spectator(id) && self.is_alive(id)
    }

    pub fn get_last_damage_source(&self, id: EntityId) -> Option<&DamageSource> {
        self.get_last_damage_source_with_timeout(id, 40)
    }

    pub fn get_last_damage_source_with_timeout(&self, id: EntityId, damage_source_timeout: i64) -> Option<&DamageSource> {
        let living = self.living(id);
        if self.game_time - living.last_damage_stamp > damage_source_timeout { None } else { living.last_damage_source.as_ref() }
    }

    pub fn item_by_slot_ref(&self, id: EntityId, slot: EquipmentSlot) -> Option<&ItemStack> {
        if let Some(mob) = self.mob(id) {
            return mob.item_by_slot(slot);
        }
        let index = self.equipment_inventory_slot(id, slot)?;
        self.player(id).inventory.slots[index].as_ref().filter(|s| !s.is_empty())
    }

    pub fn swing(&mut self, id: EntityId, hand: InteractionHand, animation: SwingAnimation, send_to_swinging_entity: bool) -> bool {
        let duration = animation.duration;
        if !self.living_mut(id).swing_state.start_if_able(hand, animation, duration) {
            return false;
        }
        let packet = encode(clientbound::SWING_ANIMATION, |w| {
            w.varint(id);
            w.varint(hand as i32);
            animation.write(w);
        });
        if send_to_swinging_entity {
            self.send_to_tracking_players_and_self(id, &packet);
        } else {
            self.send_to_tracking_players(id, &packet);
        }
        true
    }

    pub fn set_sprinting(&mut self, id: EntityId, sprinting: bool) {
        let (e, living) = self.entity_and_living_mut(id);
        e.set_shared_flag(flags::SPRINTING, sprinting);
        let speed = living.attributes.instance(Attribute::MovementSpeed).expect("living entities have movement speed");
        speed.remove_modifier(&SPEED_MODIFIER_SPRINTING.id);
        if sprinting {
            speed.add_transient_modifier(SPEED_MODIFIER_SPRINTING);
        }
    }

    pub fn living_entity_is_invulnerable_to(&mut self, id: EntityId, source: &DamageSource) -> bool {
        self.is_invulnerable_to_base(id, source) || self.is_immune_to_damage(id, source)
    }

    pub fn living_entity_hurt_server(&mut self, id: EntityId, source: DamageSource, mut damage: f32) -> bool {
        if self.is_invulnerable_to(id, &source) || self.is_dead_or_dying(id) {
            return false;
        }
        if source.is(&damage_type_tags::IS_FIRE) && self.has_effect(id, mob_effects::FIRE_RESISTANCE) {
            return false;
        }
        if self.is_sleeping(id) {
            self.stop_sleeping(id);
        }
        self.living_mut(id).no_action_time = 0;
        if damage < 0.0 {
            damage = 0.0;
        }
        let original_damage = damage;
        let item_in_use = self.get_use_item(id);
        let item_in_use_slot = self.used_item_hand(id).as_equipment_slot();
        let damage_blocked = self.apply_item_blocking(id, &source, damage);
        damage -= damage_blocked;
        let blocked = damage_blocked > 0.0;
        if source.is(&damage_type_tags::IS_FREEZING) && entity_type_tags::FREEZE_HURTS_EXTRA_TYPES.contains(self.entity(id).entity_type as usize) {
            damage *= 5.0;
        }
        if source.is(&damage_type_tags::DAMAGES_HELMET) && self.item_by_slot(id, EquipmentSlot::Head).is_some() {
            self.hurt_helmet(id, &source, damage);
            damage *= 0.75;
        }
        if !damage.is_finite() {
            damage = f32::MAX;
        }
        let mut took_full_damage = true;
        let living = &self.living(id);
        if living.damage_cooldown_time as f32 > 10.0 && !source.is(&damage_type_tags::BYPASSES_COOLDOWN) {
            if damage <= living.last_hurt {
                return false;
            }
            let last_hurt = living.last_hurt;
            self.actually_hurt(id, &source, damage - last_hurt);
            self.living_mut(id).last_hurt = damage;
            took_full_damage = false;
        } else {
            let living = &mut self.living_mut(id);
            living.last_hurt = damage;
            living.damage_cooldown_time = 20;
            self.actually_hurt(id, &source, damage);
            let living = &mut self.living_mut(id);
            living.hurt_duration = 10;
            living.hurt_time = living.hurt_duration;
        }
        self.resolve_mob_responsible_for_damage(id, &source);
        self.resolve_player_responsible_for_damage(id, &source);
        if took_full_damage {
            let item_in_use_broken = !item_in_use.is_empty() && self.item_by_slot(id, item_in_use_slot).is_none();
            match item_in_use.get(dc::BLOCKS_ATTACKS).filter(|_| blocked && !item_in_use_broken) {
                Some(blocks_attacks) => BlocksAttacks::from_nbt(blocks_attacks).on_blocked(self, id),
                None => self.broadcast_damage_event(id, &source),
            }
            if !source.is(&damage_type_tags::NO_IMPACT) && (!blocked || damage > 0.0) {
                self.mark_hurt(id);
            }
            let fully_blocked = blocked && damage <= 0.0;
            if !source.is(&damage_type_tags::NO_KNOCKBACK) && !fully_blocked {
                self.deal_default_knockback(id, &source, blocked);
            }
        }
        if self.is_dead_or_dying(id) {
            if !self.check_totem_death_protection(id, &source) {
                if took_full_damage {
                    let sound = self.get_death_sound(id);
                    self.make_sound(id, sound);
                    self.play_secondary_hurt_sound(id, &source);
                }
                self.die(id, source);
            }
        } else if took_full_damage {
            self.play_hurt_sound(id, &source);
            self.play_secondary_hurt_sound(id, &source);
        }
        let success = !blocked || damage > 0.0;
        if success {
            let game_time = self.game_time;
            let living = &mut self.living_mut(id);
            living.last_damage_source = Some(source);
            living.last_damage_stamp = game_time;
            let mut index = 0;
            while let Some(effect) = self.living(id).active_effects.get(index) {
                let (effect, amplifier) = (effect.effect, effect.amplifier);
                self.on_mob_hurt(id, effect, amplifier, &source, damage);
                index += 1;
            }
        }
        if self.is_player(id) {
            criteria_triggers::ENTITY_HURT_PLAYER.trigger(self, id, &source, original_damage, damage, blocked);
            if damage_blocked > 0.0 && damage_blocked < 3.4028235E37 {
                self.award_stat(id, stats::DAMAGE_BLOCKED_BY_SHIELD, crate::util::java_round_f(damage_blocked * 10.0));
            }
        }
        if let Some(source_player) = source.causing_entity.filter(|&entity| self.is_player(entity)) {
            criteria_triggers::PLAYER_HURT_ENTITY.trigger(self, source_player, id, &source, original_damage, damage, blocked);
        }
        success
    }

    pub fn actually_hurt(&mut self, id: EntityId, source: &DamageSource, damage: f32) {
        if self.is_player(id) {
            self.player_actually_hurt(id, source, damage);
        } else if self.is_wolf(id) {
            self.wolf_actually_hurt(id, source, damage);
        } else if self.mob(id).is_some_and(|mob| matches!(mob.kind, MobKind::Armadillo(_))) {
            self.armadillo_actually_hurt(id, source, damage);
        } else if self.mob(id).is_some_and(|mob| mob.animal().is_some()) {
            self.animal_actually_hurt(id, source, damage);
        } else {
            self.living_entity_actually_hurt(id, source, damage);
        }
    }

    pub fn living_entity_actually_hurt(&mut self, id: EntityId, source: &DamageSource, damage: f32) {
        if self.is_invulnerable_to(id, source) {
            return;
        }
        let damage = self.damage_after_armor_absorb(id, source, damage);
        let damage = self.damage_after_magic_absorb(id, source, damage);
        let original_damage = damage;
        let absorption = self.absorption_amount(id);
        let damage = (damage - absorption).max(0.0);
        self.set_absorption_amount(id, absorption - (original_damage - damage));
        let absorbed_damage = original_damage - damage;
        if absorbed_damage > 0.0
            && absorbed_damage < 3.4028235E37
            && let Some(attacker) = source.causing_entity.filter(|&e| self.is_player(e))
        {
            self.award_stat(attacker, stats::DAMAGE_DEALT_ABSORBED, crate::util::java_round_f(absorbed_damage * 10.0));
        }
        if damage != 0.0 {
            self.record_damage(id, *source, damage);
            let health = self.health(id);
            self.set_health(id, health - damage);
            let absorption = self.absorption_amount(id);
            self.set_absorption_amount(id, absorption - damage);
        }
    }

    fn resolve_mob_responsible_for_damage(&mut self, id: EntityId, source: &DamageSource) {
        let Some(living_source) = source.causing_entity.filter(|&e| self.is_living(e)) else { return };
        if source.is(&damage_type_tags::NO_ANGER) || source.is_type(damage_types::WIND_CHARGE) && entity_type_tags::NO_ANGER_FROM_WIND_CHARGE.contains(self.entity(id).entity_type as usize) {
            return;
        }
        let uuid = self.entity(living_source).uuid;
        let tick_count = self.entity(id).tick_count;
        let living = self.living_mut(id);
        living.last_hurt_by_mob = Some(uuid);
        living.last_hurt_by_mob_timestamp = tick_count;
    }

    fn resolve_player_responsible_for_damage(&mut self, id: EntityId, source: &DamageSource) {
        if let Some(player) = source.causing_entity.filter(|&e| self.is_player(e)) {
            let uuid = self.entity(player).uuid;
            let living = self.living_mut(id);
            living.last_hurt_by_player = Some(uuid);
            living.last_hurt_by_player_memory_time = 100;
        }
    }

    fn block_using_item(&mut self, id: EntityId, attacker: EntityId, fully_blocked: bool) {
        self.blocked_by_item(attacker, id, fully_blocked);
        if self.is_player(id) {
            let item_blocking_with = self.get_item_blocking_with(id);
            let blocks_attacks = item_blocking_with.as_ref().and_then(|item| item.get(dc::BLOCKS_ATTACKS)).map(BlocksAttacks::from_nbt);
            let seconds_to_disable_blocking = self.get_seconds_to_disable_blocking(attacker);
            if seconds_to_disable_blocking > 0.0
                && let (Some(blocks_attacks), Some(item_blocking_with)) = (blocks_attacks, item_blocking_with)
            {
                blocks_attacks.disable(self, id, seconds_to_disable_blocking, &item_blocking_with);
            }
        }
    }

    fn blocked_by_item(&mut self, id: EntityId, defender: EntityId, fully_blocked: bool) {
        if self.is_ravager(id) {
            self.ravager_blocked_by_item(id, defender);
            return;
        }
        if !fully_blocked {
            let (d, e) = (self.entity(defender), self.entity(id));
            let (xd, zd) = (d.x() - e.x(), d.z() - e.z());
            self.knockback(defender, 0.5, xd, zd);
        }
    }

    fn get_death_sound(&self, id: EntityId) -> i32 {
        match self.mob(id) {
            Some(mob) => sound_event(mob.get_death_sound()),
            None => sound_event("minecraft:entity.player.death"),
        }
    }

    fn play_hurt_sound(&mut self, id: EntityId, source: &DamageSource) {
        if let Some(mob) = self.mob_mut(id) {
            mob.ambient_sound_time = -mob.ambient_sound_interval();
        }
        let sound = self.get_hurt_sound(id, source);
        self.make_sound(id, sound);
    }

    fn get_hurt_sound(&self, id: EntityId, source: &DamageSource) -> i32 {
        match self.mob(id) {
            Some(_) if self.is_wolf(id) => sound_event(self.wolf_get_hurt_sound(id, source)),
            Some(mob) => sound_event(mob.get_hurt_sound()),
            None => sound_event(source.ty.info().effects.sound()),
        }
    }

    fn hurt_armor(&mut self, id: EntityId, source: &DamageSource, damage: f32) {
        if self.is_player(id) {
            self.do_hurt_equipment(id, source, damage, &[EquipmentSlot::Feet, EquipmentSlot::Legs, EquipmentSlot::Chest, EquipmentSlot::Head]);
        } else if self.is_wolf(id) {
            self.wolf_hurt_armor(id, source, damage);
        } else if self.is_horse(id) {
            self.horse_hurt_armor(id, source, damage);
        }
    }

    fn hurt_helmet(&mut self, id: EntityId, source: &DamageSource, damage: f32) {
        if self.is_player(id) {
            self.do_hurt_equipment(id, source, damage, &[EquipmentSlot::Head]);
        }
    }

    pub fn damage_after_armor_absorb(&mut self, id: EntityId, source: &DamageSource, damage: f32) -> f32 {
        if source.is(&damage_type_tags::BYPASSES_ARMOR) {
            return damage;
        }
        self.hurt_armor(id, source, damage);
        let armor = self.armor_value(id) as f32;
        let toughness = self.attribute_value(id, Attribute::ArmorToughness) as f32;
        combat_rules::get_damage_after_absorb(self, id, damage, source, armor, toughness)
    }

    pub fn damage_after_magic_absorb(&mut self, id: EntityId, source: &DamageSource, damage: f32) -> f32 {
        match self.mob(id).map(|mob| &mob.kind) {
            Some(MobKind::Witch(_)) => self.witch_get_damage_after_magic_absorb(id, source, damage),
            _ => self.living_entity_damage_after_magic_absorb(id, source, damage),
        }
    }

    pub fn living_entity_damage_after_magic_absorb(&mut self, id: EntityId, source: &DamageSource, mut damage: f32) -> f32 {
        if source.is(&damage_type_tags::BYPASSES_EFFECTS) {
            return damage;
        }
        if let Some(resistance) = self.living(id).effect(mob_effects::RESISTANCE)
            && !source.is(&damage_type_tags::BYPASSES_RESISTANCE)
        {
            let absorb = 25 - (resistance.amplifier + 1) * 5;
            let old_damage = damage;
            damage = (damage * absorb as f32 / 25.0).max(0.0);
            let damage_resisted = old_damage - damage;
            if damage_resisted > 0.0 && damage_resisted < 3.4028235E37 {
                let amount = crate::util::java_round_f(damage_resisted * 10.0);
                if self.is_player(id) {
                    self.award_stat(id, stats::DAMAGE_RESISTED, amount);
                } else if let Some(attacker) = source.causing_entity.filter(|&e| self.is_player(e)) {
                    self.award_stat(attacker, stats::DAMAGE_DEALT_RESISTED, amount);
                }
            }
        }
        if damage <= 0.0 {
            return 0.0;
        }
        if source.is(&damage_type_tags::BYPASSES_ENCHANTMENTS) {
            return damage;
        }
        let enchantment_armor = self.get_damage_protection(id, source);
        if enchantment_armor > 0.0 {
            damage = combat_rules::get_damage_after_magic_absorb(damage, enchantment_armor);
        }
        damage
    }

    pub fn do_hurt_equipment(&mut self, id: EntityId, source: &DamageSource, damage: f32, slots: &[EquipmentSlot]) {
        if damage <= 0.0 {
            return;
        }
        let durability_damage = (damage / 4.0).max(1.0) as i32;
        for &slot in slots {
            let Some(stack) = self.item_by_slot(id, slot) else { continue };
            let damage_on_hurt = stack.get(dc::EQUIPPABLE).is_some_and(|equippable| crate::world::item::component::values::Equippable::from_nbt(equippable).damage_on_hurt);
            if damage_on_hurt && stack.is_damageable_item() && stack.can_be_hurt_by(source) {
                self.hurt_and_break(id, slot, durability_damage);
            }
        }
    }

    fn broadcast_damage_event(&self, id: EntityId, source: &DamageSource) {
        let packet = encode(clientbound::DAMAGE_EVENT, |p| {
            p.varint(id);
            p.varint(source.ty.0 as i32);
            p.varint(source.causing_entity.map_or(-1, |e| e) + 1);
            p.varint(source.direct_entity.map_or(-1, |e| e) + 1);
            match source.damage_source_position {
                Some(pos) => {
                    p.bool(true);
                    p.f64(pos.x);
                    p.f64(pos.y);
                    p.f64(pos.z);
                }
                None => p.bool(false),
            }
        });
        self.send_to_tracking_players_and_self(id, &packet);
    }

    fn deal_default_knockback(&mut self, id: EntityId, source: &DamageSource, blocked: bool) {
        let (xd, zd) = match source.direct_entity.filter(|&direct| self.is_projectile(direct)) {
            Some(projectile) => {
                let (x, z) = self.calculate_horizontal_hurt_knockback_direction(projectile, id);
                (-x, -z)
            }
            None => match source.source_position(self) {
                Some(pos) => {
                    let e = self.entity(id);
                    (pos.x - e.x(), pos.z - e.z())
                }
                None => (0.0, 0.0),
            },
        };
        self.knockback(id, 0.4f32 as f64, xd, zd);
        if !blocked {
            self.indicate_damage(id, xd, zd);
        }
    }

    pub fn apply_item_blocking(&mut self, id: EntityId, source: &DamageSource, damage: f32) -> f32 {
        if damage <= 0.0 {
            return 0.0;
        }
        let Some(blocking_with) = self.get_item_blocking_with(id) else { return 0.0 };
        let Some(blocks_attacks) = blocking_with.get(dc::BLOCKS_ATTACKS).map(BlocksAttacks::from_nbt) else { return 0.0 };
        if blocks_attacks.bypassed_by.as_ref().is_some_and(|types| types.contains(source.ty.0 as i32)) {
            return 0.0;
        }
        if source.direct_entity.and_then(|direct| self.as_projectile(direct)).is_some_and(|projectile| projectile.abstract_arrow().is_some() && projectile.get_pierce_level() > 0) {
            return 0.0;
        }
        let angle = match source.source_position(self) {
            Some(source_position) => {
                let e = self.entity(id);
                let view_vector = super::calculate_view_vector(0.0, e.y_head_rot);
                let vector_to = source_position.subtract(e.position());
                let vector_to = Vec3::new(vector_to.x, 0.0, vector_to.z).normalize();
                vector_to.dot(view_vector).acos()
            }
            None => std::f32::consts::PI as f64,
        };
        let damage_blocked = blocks_attacks.resolve_blocked_damage(source, damage, angle);
        let hand = self.used_item_hand(id);
        blocks_attacks.hurt_blocking_item(self, &blocking_with, id, hand, damage_blocked);
        if damage_blocked > 0.0
            && !source.is(&damage_type_tags::IS_PROJECTILE)
            && let Some(living_entity) = source.direct_entity.filter(|&direct| self.is_living(direct))
        {
            self.block_using_item(id, living_entity, damage_blocked >= damage);
        }
        damage_blocked
    }

    pub fn knockback(&mut self, id: EntityId, power: f64, mut xd: f64, mut zd: f64) {
        let power = power * (1.0 - self.attribute_value(id, Attribute::KnockbackResistance));
        if power <= 0.0 {
            return;
        }
        let e = self.entity_mut(id);
        e.needs_sync = true;
        let movement = e.delta_movement;
        while xd * xd + zd * zd < 1.0e-5f32 as f64 {
            xd = (e.random.next_double() - e.random.next_double()) * 0.01;
            zd = (e.random.next_double() - e.random.next_double()) * 0.01;
        }
        let delta = Vec3::new(xd, 0.0, zd).normalize().scale(power);
        let y = if e.on_ground { (movement.y / 2.0 + power).min(0.4) } else { movement.y };
        e.set_delta_movement(Vec3::new(movement.x / 2.0 - delta.x, y, movement.z / 2.0 - delta.z));
    }

    fn indicate_damage(&mut self, id: EntityId, xd: f64, zd: f64) {
        if self.is_player(id) {
            self.server_player_indicate_damage(id, xd, zd);
        }
    }

    fn get_sound_volume(&self, id: EntityId) -> f32 {
        self.mob(id).map_or(1.0, |mob| mob.get_sound_volume())
    }

    pub fn make_sound(&mut self, id: EntityId, sound: i32) {
        let volume = self.get_sound_volume(id);
        let pitch = self.voice_pitch(id);
        self.entity_play_sound(id, sound, volume, pitch);
    }

    pub fn voice_pitch(&mut self, id: EntityId) -> f32 {
        match self.mob(id).map(|mob| &mob.kind) {
            Some(MobKind::Bat(_)) => self.bat_voice_pitch(id),
            Some(MobKind::ZombieVillager(_)) => self.zombie_villager_voice_pitch(id),
            _ => self.living_entity_voice_pitch(id),
        }
    }

    pub fn living_entity_voice_pitch(&mut self, id: EntityId) -> f32 {
        if self.is_player(id) {
            return 1.0;
        }
        let baby = self.is_baby(id);
        let random = &mut self.entity_mut(id).random;
        let spread = (random.next_float() - random.next_float()) * 0.2;
        if baby { spread + 1.5 } else { spread + 1.0 }
    }

    fn play_secondary_hurt_sound(&mut self, id: EntityId, source: &DamageSource) {
        if source.is_type(damage_types::THORNS) {
            let pos = self.entity(id).position();
            self.play_sound_at(None, pos.x, pos.y, pos.z, sound_event("minecraft:enchant.thorns.hit"), crate::server::level::SoundSource::Players, 1.0, 1.0);
        }
    }

    fn check_totem_death_protection(&mut self, id: EntityId, source: &DamageSource) -> bool {
        if source.is(&damage_type_tags::BYPASSES_INVULNERABILITY) {
            return false;
        }
        for slot in [EquipmentSlot::Mainhand, EquipmentSlot::Offhand] {
            let Some(mut stack) = self.item_by_slot(id, slot) else { continue };
            let Some(death_protection) = stack.get(crate::world::item::component::DEATH_PROTECTION) else { continue };
            let death_effects = crate::world::item::consume_effects::ConsumeEffect::list_from_nbt(death_protection.get("death_effects"));
            let protection_item = stack.copy();
            stack.shrink(1);
            self.set_item_slot(id, slot, stack);
            if self.is_player(id) {
                self.award_stat(id, stats::ITEM_USED.get(protection_item.get_item().0), 1);
                criteria_triggers::USED_TOTEM.trigger(self, id, &protection_item);
            }
            self.set_health(id, 1.0);
            for effect in &death_effects {
                self.apply_consume_effect(id, effect);
            }
            self.broadcast_entity_event(id, entity_event::PROTECTED_FROM_DEATH);
            return true;
        }
        false
    }

    pub fn die(&mut self, id: EntityId, source: DamageSource) {
        if self.is_player(id) {
            self.server_player_die(id, source);
        } else if self.mob(id).and_then(Mob::raider).is_some() {
            self.raider_die(id, source);
        } else if self.is_wolf(id) {
            self.wolf_die(id, source);
        } else if self.is_cat(id) {
            self.tamable_animal_die(id, source);
        } else if self.is_villager(id) {
            self.villager_die(id, source);
        } else {
            self.living_entity_die(id, source);
        }
    }

    pub fn living_entity_die(&mut self, id: EntityId, source: DamageSource) {
        if self.entity(id).is_removed() || self.living(id).dead {
            return;
        }
        if let Some(killer) = self.kill_credit(id) {
            self.award_kill_score(killer, id, &source);
        }
        if self.is_sleeping(id) {
            self.stop_sleeping(id);
        }
        if self.entity(id).custom_name().is_some() {
            let message = self.death_message(id);
            info!("Named entity {} died: {}", self.entity_to_string(id), message.get_string());
        }
        self.living_mut(id).dead = true;
        if source.causing_entity.is_none_or(|source_entity| self.killed_entity(source_entity, id, &source)) {
            self.drop_all_death_loot(id, &source);
        }
        self.broadcast_entity_event(id, entity_event::DEATH);
        self.entity_mut(id).set_pose(Pose::Dying);
    }

    pub fn killed_entity(&mut self, id: EntityId, entity: EntityId, source: &DamageSource) -> bool {
        if self.is_player(id) {
            return self.player_killed_entity(id, entity);
        }
        if matches!(self.mob(id).map(|mob| &mob.kind), Some(MobKind::Creeper(_))) {
            return self.creeper_killed_entity(id, entity, source);
        }
        if self.mob(id).is_some_and(Mob::is_zombie) {
            return self.zombie_killed_entity(id, entity, source);
        }
        self.entity_killed_entity(id, entity, source)
    }

    pub fn entity_killed_entity(&mut self, _id: EntityId, _entity: EntityId, _source: &DamageSource) -> bool {
        true
    }

    pub fn award_kill_score(&mut self, killer: EntityId, victim: EntityId, killing_blow: &DamageSource) {
        if self.is_player(killer) {
            self.server_player_award_kill_score(killer, victim, killing_blow);
        } else {
            self.entity_award_kill_score(killer, victim, killing_blow);
        }
    }

    pub fn entity_award_kill_score(&mut self, killer: EntityId, victim: EntityId, killing_blow: &DamageSource) {
        if self.is_player(victim) {
            criteria_triggers::ENTITY_KILLED_PLAYER.trigger(self, victim, killer, killing_blow);
        }
    }

    pub fn drop_all_death_loot(&mut self, id: EntityId, source: &DamageSource) {
        match self.mob(id).map(|mob| &mob.kind) {
            Some(MobKind::Fox(_)) => self.fox_drop_all_death_loot(id, source),
            _ => self.living_entity_drop_all_death_loot(id, source),
        }
    }

    pub fn living_entity_drop_all_death_loot(&mut self, id: EntityId, source: &DamageSource) {
        let player_killed = self.living(id).last_hurt_by_player_memory_time > 0;
        if self.should_drop_loot(id) {
            self.drop_from_loot_table(id, source, player_killed);
            self.drop_custom_death_loot(id, source, player_killed);
        }
        self.drop_equipment(id);
        self.drop_experience(id, source.causing_entity);
    }

    fn drop_custom_death_loot(&mut self, id: EntityId, source: &DamageSource, killed_by_player: bool) {
        match self.mob(id).map(|mob| &mob.kind) {
            Some(MobKind::Enderman(_)) => self.enderman_drop_custom_death_loot(id, source, killed_by_player),
            Some(_) => self.mob_drop_custom_death_loot(id, source, killed_by_player),
            None => {}
        }
    }

    pub fn should_drop_loot(&self, id: EntityId) -> bool {
        if self.is_monster(id) {
            return self.monster_should_drop_loot();
        }
        !self.is_baby(id) && self.game_rules.get_bool(game_rule::MOB_DROPS)
    }

    fn should_drop_experience(&self, id: EntityId) -> bool {
        if self.is_monster(id) {
            return self.monster_should_drop_experience();
        }
        if matches!(self.mob(id).map(|mob| &mob.kind), Some(MobKind::Tadpole(_))) {
            return super::animal::frog::tadpole::should_drop_experience();
        }
        !self.is_baby(id)
    }

    fn is_always_experience_dropper(&self, id: EntityId) -> bool {
        self.is_player(id)
    }

    fn drop_equipment(&mut self, id: EntityId) {
        if self.is_player(id) {
            self.player_drop_equipment(id);
        } else if self.is_abstract_chested_horse(id) {
            self.abstract_chested_horse_drop_equipment(id);
        } else if self.is_abstract_horse(id) {
            self.abstract_horse_drop_equipment(id);
        }
    }

    fn drop_experience(&mut self, id: EntityId, killer: Option<EntityId>) {
        let living = self.living(id);
        if self.is_always_experience_dropper(id) || living.last_hurt_by_player_memory_time > 0 && self.should_drop_experience(id) && self.game_rules.get_bool(game_rule::MOB_DROPS) {
            let reward = self.get_experience_reward(id, killer);
            let pos = self.entity(id).position();
            self.award_experience(pos, reward);
        }
    }

    fn get_experience_reward(&mut self, id: EntityId, killer: Option<EntityId>) -> i32 {
        let base = self.get_base_experience_reward(id);
        self.process_mob_experience(killer, base)
    }

    fn get_base_experience_reward(&mut self, id: EntityId) -> i32 {
        if self.is_player(id) {
            return self.player_get_base_experience_reward(id);
        }
        match self.mob(id).map(|mob| &mob.kind) {
            Some(MobKind::Chicken(_)) => self.chicken_get_base_experience_reward(id),
            Some(MobKind::Zombie(_) | MobKind::ZombieVillager(_) | MobKind::Husk(_) | MobKind::Drowned(_) | MobKind::ZombifiedPiglin(_)) => self.zombie_get_base_experience_reward(id),
            Some(MobKind::Blaze(_) | MobKind::Ghast(_)) => self.mob_get_base_experience_reward(id),
            Some(
                MobKind::Creeper(_)
                | MobKind::Spider(_)
                | MobKind::CaveSpider(_)
                | MobKind::Skeleton(_)
                | MobKind::Stray(_)
                | MobKind::WitherSkeleton(_)
                | MobKind::Bogged(_)
                | MobKind::Silverfish(_)
                | MobKind::Endermite(_)
                | MobKind::Giant(_),
            ) => self.mob_get_base_experience_reward(id),
            Some(MobKind::Enderman(_)) => self.mob_get_base_experience_reward(id),
            Some(MobKind::Witch(_) | MobKind::Vindicator(_) | MobKind::Pillager(_) | MobKind::Evoker(_) | MobKind::Illusioner(_) | MobKind::Vex(_) | MobKind::Ravager(_)) => self.mob_get_base_experience_reward(id),
            Some(MobKind::Slime(_) | MobKind::MagmaCube(_)) => self.mob_get_base_experience_reward(id),
            Some(MobKind::Bat(_)) => self.mob_get_base_experience_reward(id),
            Some(MobKind::Squid(_) | MobKind::GlowSquid(_)) => self.ageable_water_creature_get_base_experience_reward(id),
            Some(MobKind::IronGolem(_) | MobKind::SnowGolem(_)) => self.mob_get_base_experience_reward(id),
            Some(MobKind::Cod(_) | MobKind::Salmon(_) | MobKind::Pufferfish(_) | MobKind::TropicalFish(_) | MobKind::Tadpole(_)) => self.water_animal_get_base_experience_reward(id),
            Some(MobKind::Villager(_)) => self.mob_get_base_experience_reward(id),
            Some(_) => self.animal_get_base_experience_reward(id),
            None => 0,
        }
    }

    fn drop_from_loot_table(&mut self, id: EntityId, source: &DamageSource, player_killed: bool) {
        let Some(loot_table) = self.get_loot_table(id) else { return };
        self.drop_from_loot_table_into(id, source, player_killed, &loot_table, &mut |level, item_stack| {
            level.spawn_at_location(id, item_stack, 0.0);
        });
        if let Some(mob) = self.mob_mut(id) {
            mob.loot_table = None;
        }
    }

    pub fn get_last_hurt_by_mob(&self, id: EntityId) -> Option<EntityId> {
        self.living(id).last_hurt_by_mob.and_then(|uuid| self.entity_manager.visible_entity_storage.get(uuid)).filter(|&e| self.is_living(e))
    }

    pub fn get_last_hurt_by_player(&self, id: EntityId) -> Option<EntityId> {
        self.living(id).last_hurt_by_player.as_ref().and_then(|&uuid| self.entity_manager.visible_entity_storage.get(uuid)).filter(|&p| self.is_player(p))
    }

    pub fn get_loot_table_seed(&self, id: EntityId) -> i64 {
        self.mob(id).map_or(0, |mob| mob.loot_table_seed)
    }

    pub fn drop_from_loot_table_into(
        &mut self,
        id: EntityId,
        source: &DamageSource,
        player_killed: bool,
        loot_table: &str,
        item_stack_consumer: &mut dyn FnMut(&mut ServerLevel, ItemStack),
    ) {
        let table = LootTable::get(loot_table);
        let position = self.entity(id).position();
        let killer_player = self.get_last_hurt_by_player(id).filter(|_| player_killed);
        let luck = killer_player.map(|player| self.attribute_value(player, Attribute::Luck) as f32);
        let loot_table_seed = self.get_loot_table_seed(id);
        let mut params = LootParams::new(self);
        params.this_entity = Some(id);
        params.origin = Some(position);
        params.damage_source = Some(*source);
        params.attacking_entity = source.causing_entity;
        params.direct_attacking_entity = source.direct_entity;
        if let (Some(killer_player), Some(luck)) = (killer_player, luck) {
            params.last_damage_player = Some(killer_player);
            params.luck = luck;
        }
        table.get_random_items_with_seed(params, loot_table_seed, item_stack_consumer);
    }

    pub fn drop_from_gift_loot_table(&mut self, id: EntityId, key: &str, consumer: impl FnMut(&mut ServerLevel, ItemStack)) -> bool {
        let position = self.entity(id).position();
        self.drop_from_loot_table_with_params(
            key,
            |params| {
                params.origin = Some(position);
                params.this_entity = Some(id);
            },
            consumer,
        )
    }

    pub fn drop_from_entity_interact_loot_table(
        &mut self,
        id: EntityId,
        key: &str,
        interacting_entity: Option<EntityId>,
        tool: &ItemStack,
        mut consumer: impl FnMut(&mut ServerLevel, ItemStack),
    ) -> bool {
        let loot_table = LootTable::get(key);
        let mut params = LootParams::new(self);
        params.target_entity = Some(id);
        params.interacting_entity = interacting_entity;
        params.tool = Some(tool);
        let drops = loot_table.get_random_items(params);
        if drops.is_empty() {
            return false;
        }
        for stack in drops {
            consumer(self, stack);
        }
        true
    }

    pub fn drop_from_shearing_loot_table(&mut self, id: EntityId, key: &str, tool: &ItemStack, mut consumer: impl FnMut(&mut ServerLevel, ItemStack)) {
        let position = self.entity(id).position();
        let loot_table = LootTable::get(key);
        let mut params = LootParams::new(self);
        params.origin = Some(position);
        params.this_entity = Some(id);
        params.tool = Some(tool);
        for stack in loot_table.get_random_items(params) {
            consumer(self, stack);
        }
    }

    fn drop_from_loot_table_with_params(&mut self, key: &str, params_builder: impl FnOnce(&mut LootParams), mut consumer: impl FnMut(&mut ServerLevel, ItemStack)) -> bool {
        let loot_table = LootTable::get(key);
        let mut params = LootParams::new(self);
        params_builder(&mut params);
        let drops = loot_table.get_random_items(params);
        if drops.is_empty() {
            return false;
        }
        for stack in drops {
            consumer(self, stack);
        }
        true
    }

    pub fn living_entity_tick(&mut self, id: EntityId) {
        self.entity_tick(id);
        self.updating_using_item(id);
        let arrow_count = self.get_arrow_count(id);
        if arrow_count > 0 {
            let living = self.living_mut(id);
            if living.remove_arrow_time <= 0 {
                living.remove_arrow_time = 20 * (30 - arrow_count);
            }
            living.remove_arrow_time -= 1;
            if living.remove_arrow_time <= 0 {
                self.set_arrow_count(id, arrow_count - 1);
            }
        }
        self.detect_equipment_updates(id);
        if self.entity(id).tick_count % 20 == 0 {
            self.recheck_combat_status(id);
        }
        if self.is_sleeping(id) {
            let can_interact_with_level = self.is_alive(id) && !self.entity(id).is_removed() && !self.is_spectator(id);
            if !can_interact_with_level || !self.check_bed_exists(id) {
                self.stop_sleeping(id);
            }
        }
        if !self.entity(id).is_removed() {
            self.ai_step(id);
        }
        if !self.is_living(id) {
            return;
        }
        let e = self.entity(id);
        let (xd, zd) = (e.x() - e.xo.x, e.z() - e.xo.z);
        let side_dist = (xd * xd + zd * zd) as f32;
        let mut y_body_rot_t = self.living(id).y_body_rot;
        if side_dist > 0.0025000002 {
            let walk_direction = mth::atan2(zd, xd) as f32 * (180.0 / std::f32::consts::PI) - 90.0;
            let diff_between_direction_and_facing = (mth::wrap_degrees(e.y_rot) - walk_direction).abs();
            y_body_rot_t = if 95.0 < diff_between_direction_and_facing && diff_between_direction_and_facing < 265.0 { walk_direction - 180.0 } else { walk_direction };
        }
        if self.living(id).swing_state.animation > 0.0 {
            y_body_rot_t = e.y_rot;
        }
        if self.is_mob(id) {
            self.mob_tick_head_turn(id);
        } else {
            self.living_entity_tick_head_turn(id, y_body_rot_t);
        }
        let e = self.entity_mut(id);
        while e.y_rot - e.y_rot_o < -180.0 {
            e.y_rot_o -= 360.0;
        }
        while e.y_rot - e.y_rot_o >= 180.0 {
            e.y_rot_o += 360.0;
        }
        let living = self.living_mut(id);
        while living.y_body_rot - living.y_body_rot_o < -180.0 {
            living.y_body_rot_o -= 360.0;
        }
        while living.y_body_rot - living.y_body_rot_o >= 180.0 {
            living.y_body_rot_o += 360.0;
        }
        let fall_flying = self.entity(id).is_fall_flying();
        let living = self.living_mut(id);
        living.fall_fly_ticks = if fall_flying { living.fall_fly_ticks + 1 } else { 0 };
        if self.is_sleeping(id) {
            self.entity_mut(id).set_x_rot(0.0);
        }
        self.refresh_dirty_attributes(id);
        let living = self.living_mut(id);
        if living.current_impulse_context_reset_grace_time > 0 {
            living.current_impulse_context_reset_grace_time -= 1;
        }
    }

    fn living_entity_tick_head_turn(&mut self, id: EntityId, y_body_rot_t: f32) {
        let max_head_rotation = self.get_max_head_rotation_relative_to_body(id);
        let y_rot = self.entity(id).y_rot;
        let living = self.living_mut(id);
        let y_body_rot_d = mth::wrap_degrees(y_body_rot_t - living.y_body_rot);
        living.y_body_rot += y_body_rot_d * 0.3;
        let head_diff = mth::wrap_degrees(y_rot - living.y_body_rot);
        if head_diff.abs() > max_head_rotation {
            living.y_body_rot += head_diff - mth::sign(head_diff as f64) as f32 * max_head_rotation;
        }
    }

    fn get_max_head_rotation_relative_to_body(&self, id: EntityId) -> f32 {
        if self.is_player(id) && self.get_item_blocking_with(id).is_some() { 15.0 } else { 50.0 }
    }

    pub fn living_entity_base_tick(&mut self, id: EntityId) {
        self.living_mut(id).swing_state.tick();
        self.tick_enchantment_effects(id);
        self.entity_base_tick(id);
        if self.is_alive(id) {
            if self.is_in_wall(id) {
                self.hurt_server(id, DamageSource::new(damage_types::IN_WALL), 1.0);
            } else if self.is_player(id) && !self.world_border.is_within_bounds_aabb(&self.entity(id).bounding_box()) {
                let border = &self.world_border;
                let dist = border.get_distance_to_border(self.entity(id).x(), self.entity(id).z()) + border.get_safe_zone();
                let damage_per_block = border.get_damage_per_block();
                if dist < 0.0 && damage_per_block > 0.0 {
                    self.hurt_server(id, DamageSource::new(damage_types::OUTSIDE_BORDER), mth::floor(-dist * damage_per_block).max(1) as f32);
                }
            }
            let e = self.entity(id);
            let eye_pos = BlockPos::containing(e.x(), e.eye_y(), e.z());
            if e.is_eye_in_fluid(&fluid_tags::WATER) && !self.block_state(eye_pos).is_block(blocks::BUBBLE_COLUMN) {
                let can_drown = !entity_type_tags::CAN_BREATHE_UNDER_WATER.contains(self.entity(id).entity_type as usize)
                    && !self.has_water_breathing(id)
                    && !self.players.get(&id).is_some_and(|p| p.abilities.invulnerable);
                if can_drown {
                    let supply = self.decrease_air_supply(id, self.entity(id).air_supply());
                    self.entity_mut(id).set_air_supply(supply);
                    if self.should_take_drowning_damage(id) {
                        self.entity_mut(id).set_air_supply(0);
                        self.broadcast_entity_event(id, entity_event::DROWN_PARTICLES);
                        self.hurt_server(id, DamageSource::new(damage_types::DROWN), 2.0);
                    }
                } else if self.entity(id).air_supply() < self.entity(id).get_max_air_supply() && self.should_effects_refill_air_supply(id) {
                    let supply = (self.entity(id).air_supply() + 4).min(self.entity(id).get_max_air_supply());
                    self.entity_mut(id).set_air_supply(supply);
                }
                if self.vehicle(id).is_some_and(|vehicle| self.dismounts_underwater(vehicle)) {
                    self.stop_riding(id);
                }
            } else if self.entity(id).air_supply() < self.entity(id).get_max_air_supply() {
                let supply = (self.entity(id).air_supply() + 4).min(self.entity(id).get_max_air_supply());
                self.entity_mut(id).set_air_supply(supply);
            }
            let pos = self.entity(id).block_position();
            if self.living(id).last_pos != Some(pos) {
                self.living_mut(id).last_pos = Some(pos);
                self.on_changed_block(id, pos);
            }
        }
        let player = self.is_player(id);
        let living = &mut self.living_mut(id);
        if living.hurt_time > 0 {
            living.hurt_time -= 1;
        }
        if living.damage_cooldown_time > 0 && !player {
            living.damage_cooldown_time -= 1;
        }
        if self.is_dead_or_dying(id) {
            self.tick_death(id);
        }
        let living = &mut self.living_mut(id);
        if living.last_hurt_by_player_memory_time > 0 {
            living.last_hurt_by_player_memory_time -= 1;
        } else {
            living.last_hurt_by_player = None;
        }
        if let Some(last_hurt_mob) = self.living(id).last_hurt_mob
            && !self.is_alive(last_hurt_mob)
        {
            self.living_mut(id).last_hurt_mob = None;
        }
        if let Some(hurt_by) = self.living(id).last_hurt_by_mob.and_then(|uuid| self.entity_manager.visible_entity_storage.get(uuid)).filter(|&e| self.is_living(e)) {
            let alive = self.is_alive(hurt_by);
            let tick_count = self.entity(id).tick_count;
            let living = self.living_mut(id);
            if !alive || tick_count - living.last_hurt_by_mob_timestamp > 100 {
                living.last_hurt_by_mob = None;
                living.last_hurt_by_mob_timestamp = tick_count;
            }
        }
        self.tick_effects(id);
        let y_head_rot = self.entity(id).y_head_rot;
        let living = self.living_mut(id);
        living.y_head_rot_o = y_head_rot;
        living.y_body_rot_o = living.y_body_rot;
    }

    pub fn should_take_drowning_damage(&self, id: EntityId) -> bool {
        self.entity(id).air_supply() <= -20
    }

    fn decrease_air_supply(&mut self, id: EntityId, supply: i32) -> i32 {
        match self.mob(id).map(|mob| &mob.kind) {
            Some(MobKind::IronGolem(_)) => self.iron_golem_decrease_air_supply(supply),
            _ => self.living_entity_decrease_air_supply(id, supply),
        }
    }

    fn living_entity_decrease_air_supply(&mut self, id: EntityId, supply: i32) -> i32 {
        let oxygen_bonus = self.attribute_value(id, Attribute::OxygenBonus);
        if oxygen_bonus > 0.0 && self.entity_mut(id).random.next_double() >= 1.0 / (oxygen_bonus + 1.0) { supply } else { supply - 1 }
    }

    fn tick_death(&mut self, id: EntityId) {
        let living = self.living_mut(id);
        living.death_time += 1;
        if living.death_time >= 20 && !self.entity(id).is_removed() {
            self.broadcast_entity_event(id, entity_event::POOF);
            self.remove(id, RemovalReason::Killed);
        }
    }

    pub fn remove(&mut self, id: EntityId, reason: RemovalReason) {
        if self.is_player(id) {
            self.player_remove(id, reason);
        } else if self.mob(id).is_some_and(|mob| mob.is_cube_mob()) {
            self.abstract_cube_mob_remove(id, reason);
        } else if self.is_living(id) {
            self.living_entity_remove(id, reason);
        } else if self.is_container_entity(id) {
            self.abstract_chest_boat_remove(id, reason);
        } else {
            self.set_removed(id, reason);
        }
    }

    pub fn living_entity_remove(&mut self, id: EntityId, reason: RemovalReason) {
        if self.entity(id).is_removed() {
            return;
        }
        if matches!(reason, RemovalReason::Killed | RemovalReason::Discarded) {
            self.trigger_on_death_mob_effects(id, reason);
        }
        self.set_removed(id, reason);
        if let Some(brain) = self.get_brain_mut_if_present(id) {
            brain.clear_memories();
        }
        self.get_waypoint_manager().untrack_waypoint(self, id);
    }

    pub fn can_freeze(&self, id: EntityId) -> bool {
        match self.mob(id).map(|mob| &mob.kind) {
            Some(MobKind::Skeleton(_)) => self.skeleton_can_freeze(),
            _ if self.is_living(id) => self.living_entity_can_freeze(id),
            _ => self.entity_can_freeze(id),
        }
    }

    pub fn entity_can_freeze(&self, id: EntityId) -> bool {
        !entity_type_tags::FREEZE_IMMUNE_ENTITY_TYPES.contains(self.entity(id).entity_type as usize)
    }

    pub fn living_entity_can_freeze(&self, id: EntityId) -> bool {
        if self.is_spectator(id) {
            return false;
        }
        for slot in [EquipmentSlot::Feet, EquipmentSlot::Legs, EquipmentSlot::Chest, EquipmentSlot::Head, EquipmentSlot::Body] {
            if self.item_by_slot(id, slot).is_some_and(|s| s.item.is(&item_tags::FREEZE_IMMUNE_WEARABLES)) {
                return false;
            }
        }
        self.entity_can_freeze(id)
    }

    pub fn tick_freezing_damage(&mut self, id: EntityId) {
        let e = self.entity(id);
        if e.tick_count % 40 == 0 && e.ticks_frozen() >= 140 && self.can_freeze(id) {
            self.hurt_server(id, DamageSource::new(damage_types::FREEZE), 1.0);
        }
    }

    pub fn detect_equipment_updates(&mut self, id: EntityId) {
        let mut changed: Vec<(EquipmentSlot, Option<ItemStack>)> = Vec::new();
        for slot in EquipmentSlot::VALUES {
            let previous = self.living(id).last_equipment_items[slot as usize].clone();
            let current = self.item_by_slot(id, slot);
            if previous != current {
                changed.push((slot, current.clone()));
                if let Some(previous) = previous {
                    self.stop_location_based_effects(id, &previous, slot);
                }
            }
        }
        if changed.is_empty() {
            return;
        }
        for (slot, current) in &changed {
            if let Some(current) = current
                && !current.is_broken()
            {
                for_each_modifier(current, *slot, |attribute, modifier| {
                    if let Some(instance) = self.living_mut(id).attributes.instance(attribute) {
                        instance.remove_modifier(&modifier.id);
                        instance.add_transient_modifier(modifier);
                    }
                });
                self.run_location_changed_effects_on_item(current, id, *slot);
            }
        }
        let last = self.living(id).last_equipment_items.clone();
        let main = changed.iter().find(|c| c.0 == EquipmentSlot::Mainhand).map(|c| c.1.clone());
        let off = changed.iter().find(|c| c.0 == EquipmentSlot::Offhand).map(|c| c.1.clone());
        if let (Some(main), Some(off)) = (main, off)
            && main == last[EquipmentSlot::Offhand as usize]
            && off == last[EquipmentSlot::Mainhand as usize]
        {
            let packet = encode(clientbound::ENTITY_EVENT, |w| {
                w.i32(id);
                w.u8(entity_event::SWAP_HANDS);
            });
            self.send_to_tracking_players(id, &packet);
            changed.retain(|c| c.0 != EquipmentSlot::Mainhand && c.0 != EquipmentSlot::Offhand);
            let living = &mut self.living_mut(id);
            living.last_equipment_items[EquipmentSlot::Mainhand as usize] = main;
            living.last_equipment_items[EquipmentSlot::Offhand as usize] = off;
        }
        if changed.is_empty() {
            return;
        }
        let living = &mut self.living_mut(id);
        for (slot, current) in &changed {
            living.last_equipment_items[*slot as usize] = current.clone();
        }
        let packet = set_equipment(id, &changed);
        self.send_to_tracking_players(id, &packet);
    }

    pub fn stop_location_based_effects(&mut self, id: EntityId, previous: &ItemStack, slot: EquipmentSlot) {
        for_each_modifier(previous, slot, |attribute, modifier| {
            if let Some(instance) = self.living_mut(id).attributes.get_mut(attribute) {
                instance.remove_modifier(&modifier.id);
            }
        });
        self.stop_location_based_effects_on_item(previous, id, slot);
    }

    pub fn on_changed_block(&mut self, id: EntityId, _pos: BlockPos) {
        if self.is_player(id) && self.is_spectator(id) {
            return;
        }
        self.run_location_changed_effects(id);
    }

    pub fn refresh_dirty_attributes(&mut self, id: EntityId) {
        let updated = self.living_mut(id).attributes.take_to_update();
        for attribute in Attribute::ALL {
            if updated & 1 << attribute as u64 != 0 {
                self.on_attribute_updated(id, attribute);
            }
        }
    }

    fn on_attribute_updated(&mut self, id: EntityId, attribute: Attribute) {
        if self.is_player(id) {
            self.server_player_on_attribute_updated(id, attribute);
        } else {
            self.living_entity_on_attribute_updated(id, attribute);
        }
    }

    pub fn living_entity_on_attribute_updated(&mut self, id: EntityId, attribute: Attribute) {
        if attribute == Attribute::MaxHealth {
            let max_health = self.max_health(id);
            if self.health(id) > max_health {
                self.set_health(id, max_health);
            }
        } else if attribute == Attribute::MaxAbsorption {
            let max_absorption = self.max_absorption(id);
            if self.absorption_amount(id) > max_absorption {
                self.set_absorption_amount(id, max_absorption);
            }
        } else if attribute == Attribute::WaypointTransmitRange {
            if self.living(id).attributes.value(attribute) > 0.0 {
                self.get_waypoint_manager().track_waypoint(self, id);
            } else {
                self.get_waypoint_manager().untrack_waypoint(self, id);
            }
        }
    }

    pub fn is_transmitting_waypoint(&self, id: EntityId) -> bool {
        self.attribute_value(id, Attribute::WaypointTransmitRange) > 0.0
    }

    pub fn make_waypoint_connection_with(&self, id: EntityId, player: EntityId) -> Option<Connection> {
        if self.entity(id).first_tick || player == id {
            return None;
        }
        if waypoint_transmitter::does_source_ignore_receiver(self, id, player) {
            return None;
        }
        let icon = self.living(id).locator_bar_icon.clone_and_assign_style(self.team_of(id).and_then(|team| team.color()));
        if waypoint_transmitter::is_really_far(self, id, player) {
            Some(Connection::EntityAzimuth(EntityAzimuthConnection::new(self, id, icon, player)))
        } else if !waypoint_transmitter::is_chunk_visible(self, self.entity(id).chunk_position(), player) {
            Some(Connection::EntityChunk(EntityChunkConnection::new(self, id, icon, player)))
        } else {
            Some(Connection::EntityBlock(EntityBlockConnection::new(self, id, icon, player)))
        }
    }

    pub fn waypoint_icon(&mut self, id: EntityId) -> &mut Icon {
        &mut self.living_mut(id).locator_bar_icon
    }

    pub fn ai_step(&mut self, id: EntityId) {
        if self.is_player(id) {
            self.player_ai_step(id);
            return;
        }
        match self.mob(id).map(|mob| &mob.kind) {
            Some(MobKind::Chicken(_)) => self.chicken_ai_step(id),
            Some(MobKind::Goat(_)) => self.goat_ai_step(id),
            Some(
                MobKind::Zombie(_)
                | MobKind::ZombieVillager(_)
                | MobKind::Husk(_)
                | MobKind::Drowned(_)
                | MobKind::Creeper(_)
                | MobKind::Spider(_)
                | MobKind::CaveSpider(_)
                | MobKind::Skeleton(_)
                | MobKind::Stray(_)
                | MobKind::WitherSkeleton(_)
                | MobKind::Bogged(_)
                | MobKind::Silverfish(_)
                | MobKind::Giant(_),
            ) => self.monster_ai_step(id),
            Some(MobKind::Endermite(_)) => self.endermite_ai_step(id),
            Some(MobKind::Enderman(_)) => self.enderman_ai_step(id),
            Some(MobKind::Blaze(_)) => self.blaze_ai_step(id),
            Some(MobKind::Ghast(_)) => self.mob_ai_step(id),
            Some(MobKind::ZombifiedPiglin(_)) => self.monster_ai_step(id),
            Some(MobKind::Witch(_)) => self.witch_ai_step(id),
            Some(MobKind::Ravager(_)) => self.ravager_ai_step(id),
            Some(MobKind::Vindicator(_) | MobKind::Pillager(_) | MobKind::Evoker(_) | MobKind::Illusioner(_)) => self.raider_ai_step(id),
            Some(MobKind::Vex(_)) => self.monster_ai_step(id),
            Some(MobKind::Slime(_) | MobKind::MagmaCube(_)) => self.ageable_mob_ai_step(id),
            Some(MobKind::Wolf(_)) => self.wolf_ai_step(id),
            Some(MobKind::Bat(_)) => self.mob_ai_step(id),
            Some(MobKind::Squid(_)) => self.squid_ai_step(id),
            Some(MobKind::GlowSquid(_)) => self.glow_squid_ai_step(id),
            Some(MobKind::IronGolem(_)) => self.iron_golem_ai_step(id),
            Some(MobKind::SnowGolem(_)) => self.snow_golem_ai_step(id),
            Some(MobKind::Cod(_) | MobKind::Salmon(_) | MobKind::TropicalFish(_)) => self.abstract_fish_ai_step(id),
            Some(MobKind::Pufferfish(_)) => self.pufferfish_ai_step(id),
            Some(MobKind::Horse(_) | MobKind::Donkey(_) | MobKind::Mule(_)) => self.abstract_horse_ai_step(id),
            Some(MobKind::Tadpole(_)) => self.tadpole_ai_step(id),
            Some(MobKind::Villager(_)) => self.ageable_mob_ai_step(id),
            Some(MobKind::Fox(_)) => self.fox_ai_step(id),
            Some(MobKind::Rabbit(_)) => self.rabbit_ai_step(id),
            Some(MobKind::Turtle(_)) => self.turtle_ai_step(id),
            Some(_) => self.animal_ai_step(id),
            None => self.living_entity_ai_step(id),
        }
    }

    pub fn is_effective_ai(&self, id: EntityId) -> bool {
        if self.is_player(id) {
            return true;
        }
        self.is_local_instance_authoritative(id) && self.mob(id).is_none_or(|mob| !mob.is_no_ai())
    }

    pub fn is_immobile(&self, id: EntityId) -> bool {
        if self.is_player(id) {
            self.living_entity_is_immobile(id) || self.is_sleeping(id)
        } else if self.is_abstract_horse(id) {
            self.abstract_horse_is_immobile(id)
        } else if self.is_ravager(id) {
            self.living_entity_is_immobile(id) || Ravager::is_immobile(self.mob(id).expect("a ravager"))
        } else {
            self.living_entity_is_immobile(id)
        }
    }

    pub fn living_entity_is_immobile(&self, id: EntityId) -> bool {
        self.is_dead_or_dying(id)
    }

    fn travel_ridden(&mut self, id: EntityId, controller: EntityId, self_input: Vec3) {
        let ridden_input = self.get_ridden_input(id, controller, self_input);
        self.tick_ridden_by(id, controller, ridden_input);
        if self.can_simulate_movement(id) {
            let speed = self.get_ridden_speed(id, controller);
            self.set_speed(id, speed);
            self.travel(id, ridden_input);
        } else {
            self.entity_mut(id).set_delta_movement(Vec3::ZERO);
        }
    }

    pub fn tick_ridden_by(&mut self, id: EntityId, controller: EntityId, ridden_input: Vec3) {
        if self.is_abstract_horse(id) {
            self.abstract_horse_tick_ridden(id, controller, ridden_input);
        } else if self.is_pig(id) {
            self.pig_tick_ridden(id, controller, ridden_input);
        } else {
            self.living_entity_tick_ridden(id, controller, ridden_input);
        }
    }

    pub fn living_entity_tick_ridden(&mut self, _id: EntityId, _controller: EntityId, _ridden_input: Vec3) {}

    pub fn get_ridden_input(&self, id: EntityId, controller: EntityId, self_input: Vec3) -> Vec3 {
        if self.is_abstract_horse(id) {
            self.abstract_horse_get_ridden_input(id, controller, self_input)
        } else if self.is_pig(id) {
            self.pig_get_ridden_input(id, controller, self_input)
        } else {
            self_input
        }
    }

    pub fn get_ridden_speed(&self, id: EntityId, controller: EntityId) -> f32 {
        if self.is_abstract_horse(id) {
            self.abstract_horse_get_ridden_speed(id, controller)
        } else if self.is_pig(id) {
            self.pig_get_ridden_speed(id, controller)
        } else {
            self.living(id).speed
        }
    }

    fn server_ai_step(&mut self, id: EntityId) {
        if self.is_mob(id) {
            self.mob_server_ai_step(id);
        }
    }

    pub fn living_entity_ai_step(&mut self, id: EntityId) {
        let player = self.is_player(id);
        let immobile = self.is_immobile(id);
        let living = self.living_mut(id);
        if living.no_jump_delay > 0 {
            living.no_jump_delay -= 1;
        }
        if !self.can_simulate_movement(id) {
            let e = self.entity_mut(id);
            e.set_delta_movement(e.delta_movement.scale(0.98));
        }
        if player {
            self.player_equipment_tick(id);
        }
        let (e, living) = self.entity_and_living_mut(id);
        let movement = e.delta_movement;
        let (mut dx, mut dy, mut dz) = (movement.x, movement.y, movement.z);
        if player {
            if movement.horizontal_distance_sqr() < 9.0e-6 {
                dx = 0.0;
                dz = 0.0;
            }
        } else {
            if movement.x.abs() < 0.003 {
                dx = 0.0;
            }
            if movement.z.abs() < 0.003 {
                dz = 0.0;
            }
        }
        if movement.y.abs() < 0.003 {
            dy = 0.0;
        }
        e.set_delta_movement(Vec3::new(dx, dy, dz));
        living.xxa *= 0.98;
        living.zza *= 0.98;
        if immobile {
            living.jumping = false;
            living.xxa = 0.0;
            living.zza = 0.0;
        } else if self.is_effective_ai(id) {
            self.server_ai_step(id);
        }
        let affected_by_fluids = self.is_affected_by_fluids(id);
        let (e, living) = self.entity_and_living(id);
        if living.jumping && affected_by_fluids {
            let fluid_height = if e.is_in_lava() { e.fluid_height(&fluid_tags::LAVA) } else { e.fluid_height(&fluid_tags::WATER) };
            let in_water_and_has_fluid_height = e.is_in_water() && fluid_height > 0.0;
            let threshold = e.fluid_jump_threshold();
            if !in_water_and_has_fluid_height || e.on_ground && !(fluid_height > threshold) {
                if !e.is_in_lava() || e.on_ground && e.fluid_height(&fluid_tags::LAVA) <= threshold {
                    if (e.on_ground || in_water_and_has_fluid_height && fluid_height <= threshold) && living.no_jump_delay == 0 {
                        self.jump_from_ground(id);
                        self.living_mut(id).no_jump_delay = 10;
                    }
                } else {
                    self.jump_in_liquid(id, &fluid_tags::LAVA);
                }
            } else {
                self.jump_in_liquid(id, &fluid_tags::WATER);
            }
        } else {
            self.living_mut(id).no_jump_delay = 0;
        }
        if self.entity(id).is_fall_flying() {
            self.update_fall_flying(id);
        }
        let before_travel_box = self.entity(id).bounding_box();
        let l = &self.living(id);
        let input = Vec3::new(l.xxa as f64, l.yya as f64, l.zza as f64);
        if l.has_effect(mob_effects::SLOW_FALLING) || l.has_effect(mob_effects::LEVITATION) {
            self.reset_fall_distance(id);
        }
        match self.controlling_passenger(id).filter(|&controller| self.is_player(controller)) {
            Some(controller) if self.is_alive(id) => self.travel_ridden(id, controller, input),
            _ => {
                if self.can_simulate_movement(id) && self.is_effective_ai(id) {
                    self.travel(id, input);
                }
            }
        }
        self.apply_effects_from_blocks(id);
        let e = self.entity_mut(id);
        if !e.is_in_powder_snow || !self.can_freeze(id) {
            let e = self.entity_mut(id);
            let ticks = (e.ticks_frozen() - 2).max(0);
            e.set_ticks_frozen(ticks);
        }
        self.remove_frost(id);
        self.try_add_frost(id);
        self.tick_freezing_damage(id);
        if self.living(id).auto_spin_attack_ticks > 0 {
            self.living_mut(id).auto_spin_attack_ticks -= 1;
            let current = self.entity(id).bounding_box();
            self.check_auto_spin_attack(id, before_travel_box, current);
        }
        self.push_entities(id);
        if self.is_sensitive_to_water(id) && self.is_in_water_or_rain(id) {
            self.hurt_server(id, DamageSource::new(damage_types::DROWN), 1.0);
        }
    }

    fn update_fall_flying(&mut self, id: EntityId) {
        self.entity_mut(id).check_fall_distance_accumulation();
        if !self.can_glide(id) {
            self.entity_mut(id).set_shared_flag(flags::FALL_FLYING, false);
            return;
        }
        let check_fall_fly_ticks = self.living(id).fall_fly_ticks + 1;
        if check_fall_fly_ticks % 10 == 0 {
            let free_fall_interval = check_fall_fly_ticks / 10;
            if free_fall_interval % 2 == 0 {
                let slots_with_gliders: Vec<EquipmentSlot> = EquipmentSlot::VALUES
                    .into_iter()
                    .filter(|&slot| self.item_by_slot_ref(id, slot).is_some_and(|item_stack| can_glide_using(item_stack, slot)))
                    .collect();
                let index = self.entity_mut(id).random.next_int_bounded(slots_with_gliders.len() as i32);
                self.hurt_and_break(id, slots_with_gliders[index as usize], 1);
            }
        }
    }

    pub fn can_glide(&self, id: EntityId) -> bool {
        if self.is_player(id) { self.player_can_glide(id) } else { self.living_entity_can_glide(id) }
    }

    pub fn living_entity_can_glide(&self, id: EntityId) -> bool {
        if self.entity(id).on_ground || self.is_passenger(id) || self.has_effect(id, mob_effects::LEVITATION) {
            return false;
        }
        EquipmentSlot::VALUES.into_iter().any(|slot| self.item_by_slot_ref(id, slot).is_some_and(|item_stack| can_glide_using(item_stack, slot)))
    }

    fn check_auto_spin_attack(&mut self, id: EntityId, old: AABB, current: AABB) {
        let minmax = old.minmax(&current);
        let entities = self.get_entities(Some(id), &minmax);
        if !entities.is_empty() {
            if let Some(living_entity) = entities.into_iter().find(|&entity| self.is_living(entity)) {
                self.do_auto_attack_on_touch(id, living_entity);
                self.living_mut(id).auto_spin_attack_ticks = 0;
                let e = self.entity_mut(id);
                e.set_delta_movement(e.delta_movement.scale(-0.2));
            }
        } else if self.entity(id).horizontal_collision {
            self.living_mut(id).auto_spin_attack_ticks = 0;
        }
        if self.living(id).auto_spin_attack_ticks <= 0 {
            self.set_living_entity_flag(id, FLAG_SPIN_ATTACK, false);
            let living = self.living_mut(id);
            living.auto_spin_attack_dmg = 0.0;
            living.auto_spin_attack_item_stack = None;
        }
    }

    fn do_auto_attack_on_touch(&mut self, id: EntityId, entity: EntityId) {
        if self.is_player(id) {
            self.player_attack(id, entity);
        }
    }

    pub fn is_auto_spin_attack(&self, id: EntityId) -> bool {
        self.living_entity_flags(id) & FLAG_SPIN_ATTACK != 0
    }

    fn is_affected_by_fluids(&self, id: EntityId) -> bool {
        self.players.get(&id).is_none_or(|p| !p.abilities_flying())
    }

    pub fn living_entity_is_pushable(&mut self, id: EntityId) -> bool {
        self.is_alive(id) && !self.is_spectator(id) && !self.on_climbable(id)
    }

    pub fn pushable_by(&mut self, pusher: EntityId, other: EntityId) -> bool {
        let own_team = self.team_of(pusher).map(|t| (t.name().to_string(), t.collision_rule()));
        let own_rule = own_team.as_ref().map_or(CollisionRule::Always, |t| t.1);
        if own_rule == CollisionRule::Never || !self.is_pushable(other) {
            return false;
        }
        let their_team = self.team_of(other).map(|t| (t.name().to_string(), t.collision_rule()));
        let their_rule = their_team.as_ref().map_or(CollisionRule::Always, |t| t.1);
        if their_rule == CollisionRule::Never {
            return false;
        }
        let same_team = own_team.is_some() && own_team.as_ref().map(|t| &t.0) == their_team.as_ref().map(|t| &t.0);
        if (own_rule == CollisionRule::PushOwnTeam || their_rule == CollisionRule::PushOwnTeam) && same_team {
            return false;
        }
        own_rule != CollisionRule::PushOtherTeams && their_rule != CollisionRule::PushOtherTeams || same_team
    }

    fn push_entities(&mut self, id: EntityId) {
        if self.is_player(id) {
            self.server_player_push_entities(id);
        } else if matches!(self.mob(id).map(|mob| &mob.kind), Some(MobKind::Bat(_))) {
            self.bat_push_entities(id);
        } else {
            self.living_entity_push_entities(id);
        }
    }

    fn server_player_push_entities(&mut self, id: EntityId) {
        if self.tick_rate_manager.runs_normally() {
            self.living_entity_push_entities(id);
        }
    }

    fn living_entity_push_entities(&mut self, id: EntityId) {
        let area = self.entity(id).bounding_box();
        let candidates = self.get_entities(Some(id), &area);
        let pushable: Vec<EntityId> = candidates.into_iter().filter(|&other| self.pushable_by(id, other)).collect();
        if pushable.is_empty() {
            return;
        }
        let max_cramming = self.game_rules.get_int(game_rule::MAX_ENTITY_CRAMMING);
        if max_cramming > 0 && pushable.len() as i32 > max_cramming - 1 && self.entity_mut(id).random.next_int_bounded(4) == 0 {
            let count = pushable.iter().filter(|&&entity| !self.is_passenger(entity)).count();
            if count as i32 > max_cramming - 1 {
                self.hurt_server(id, DamageSource::new(damage_types::CRAMMING), 6.0);
            }
        }
        for other in pushable {
            self.do_push(id, other);
        }
    }

    fn do_push(&mut self, id: EntityId, entity: EntityId) {
        match self.mob(id).map(|mob| &mob.kind) {
            Some(MobKind::IronGolem(_)) => self.iron_golem_do_push(id, entity),
            _ => self.living_entity_do_push(id, entity),
        }
    }

    pub fn living_entity_do_push(&mut self, id: EntityId, entity: EntityId) {
        self.push_entity(entity, id);
    }

    fn remove_frost(&mut self, id: EntityId) {
        if let Some(speed) = self.living_mut(id).attributes.instance(Attribute::MovementSpeed)
            && speed.has_modifier(SPEED_MODIFIER_POWDER_SNOW_ID)
        {
            speed.remove_modifier(SPEED_MODIFIER_POWDER_SNOW_ID);
        }
    }

    fn try_add_frost(&mut self, id: EntityId) {
        let on = self.on_pos_legacy(id);
        if self.block_state(on).is_air() {
            return;
        }
        let ticks_frozen = self.entity(id).ticks_frozen();
        if ticks_frozen > 0 {
            let Some(speed) = self.living_mut(id).attributes.instance(Attribute::MovementSpeed) else { return };
            let percent_frozen = ticks_frozen.min(TICKS_REQUIRED_TO_FREEZE) as f32 / TICKS_REQUIRED_TO_FREEZE as f32;
            let slow_amount = -0.05f32 * percent_frozen;
            speed.add_transient_modifier(AttributeModifier {
                id: std::borrow::Cow::Borrowed(SPEED_MODIFIER_POWDER_SNOW_ID),
                amount: slow_amount as f64,
                operation: Operation::AddValue,
            });
        }
    }

    fn jump_in_liquid(&mut self, id: EntityId, ty: &wafer_data::Tag) {
        match self.mob(id).map(|mob| &mob.kind) {
            Some(MobKind::MagmaCube(_)) => self.magma_cube_jump_in_liquid(id, ty),
            Some(_) => self.mob_jump_in_liquid(id, ty),
            None => self.living_entity_jump_in_liquid(id, ty),
        }
    }

    pub fn mob_jump_in_liquid(&mut self, id: EntityId, ty: &wafer_data::Tag) {
        if self.mob(id).is_some_and(|mob| !mob.ai.navigation.can_float()) {
            let e = self.entity_mut(id);
            e.set_delta_movement(e.delta_movement.add(0.0, 0.3, 0.0));
        } else {
            self.living_entity_jump_in_liquid(id, ty);
        }
    }

    fn living_entity_jump_in_liquid(&mut self, id: EntityId, _ty: &wafer_data::Tag) {
        let e = self.entity_mut(id);
        e.set_delta_movement(e.delta_movement.add(0.0, 0.04f32 as f64, 0.0));
    }

    pub fn jump_from_ground(&mut self, id: EntityId) {
        match self.mob(id).map(|mob| &mob.kind) {
            Some(MobKind::Slime(_)) => self.abstract_cube_mob_jump_from_ground(id),
            Some(MobKind::MagmaCube(_)) => self.magma_cube_jump_from_ground(id),
            Some(MobKind::Rabbit(_)) => self.rabbit_jump_from_ground(id),
            _ if self.is_player(id) => self.server_player_jump_from_ground(id),
            _ => self.living_entity_jump_from_ground(id),
        }
    }

    pub fn get_jump_power(&mut self, id: EntityId) -> f32 {
        match self.mob(id).map(|mob| &mob.kind) {
            Some(MobKind::Rabbit(_)) => self.rabbit_get_jump_power(id),
            _ => self.living_entity_get_jump_power(id),
        }
    }

    pub fn living_entity_get_jump_power(&mut self, id: EntityId) -> f32 {
        self.get_jump_power_with_multiplier(id, 1.0)
    }

    pub fn get_jump_power_with_multiplier(&mut self, id: EntityId, multiplier: f32) -> f32 {
        self.attribute_value(id, Attribute::JumpStrength) as f32 * multiplier * self.block_jump_factor(id) + self.jump_boost_power(id)
    }

    pub fn jump_boost_power(&self, id: EntityId) -> f32 {
        match self.living(id).effect(mob_effects::JUMP_BOOST) {
            Some(jump_boost) => 0.1 * (jump_boost.amplifier as f32 + 1.0),
            None => 0.0,
        }
    }

    pub fn living_entity_jump_from_ground(&mut self, id: EntityId) {
        let jump_power = self.get_jump_power(id);
        if jump_power <= 1.0e-5 {
            return;
        }
        let e = self.entity_mut(id);
        let m = e.delta_movement;
        e.set_delta_movement(Vec3::new(m.x, mth::jmax(jump_power as f64, m.y), m.z));
        if e.is_sprinting() {
            let angle = e.y_rot * mth::DEG_TO_RAD;
            e.add_delta_movement(Vec3::new(-mth::sin(angle as f64) as f64 * 0.2, 0.0, mth::cos(angle as f64) as f64 * 0.2));
        }
        e.needs_sync = true;
    }

    pub fn travel(&mut self, id: EntityId, input: Vec3) {
        if self.is_player(id) {
            self.player_travel(id, input);
        } else if self.mob(id).is_some_and(Mob::is_squid) {
            self.squid_travel(id, input);
        } else if matches!(self.mob(id).map(|mob| &mob.kind), Some(MobKind::Ghast(_))) {
            self.ghast_travel(id, input);
        } else {
            self.living_entity_travel(id, input);
        }
    }

    pub fn travel_flying(&mut self, id: EntityId, input: Vec3, speed: f32) {
        self.travel_flying_with(id, input, 0.02, 0.02, speed);
    }

    pub fn travel_flying_with(&mut self, id: EntityId, input: Vec3, water_speed: f32, lava_speed: f32, air_speed: f32) {
        let (speed, inertia) = if self.entity(id).is_in_water() {
            (water_speed, 0.8f32 as f64)
        } else if self.entity(id).is_in_lava() {
            (lava_speed, 0.5)
        } else {
            (air_speed, 0.91f32 as f64)
        };
        self.entity_mut(id).move_relative(speed, input);
        let delta = self.entity(id).delta_movement;
        self.move_entity(id, MoverType::SelfMove, delta);
        let e = self.entity_mut(id);
        e.set_delta_movement(e.delta_movement.scale(inertia));
    }

    pub fn living_entity_travel(&mut self, id: EntityId, input: Vec3) {
        if self.entity(id).is_in_liquid() && self.is_affected_by_fluids(id) {
            self.travel_in_fluid(id, input);
        } else if self.entity(id).is_fall_flying() {
            self.travel_fall_flying(id, input);
        } else {
            self.travel_in_air(id, input);
        }
    }

    fn travel_fall_flying(&mut self, id: EntityId, input: Vec3) {
        if self.on_climbable(id) {
            self.travel_in_air(id, input);
            self.stop_fall_flying(id);
        } else {
            let last_movement = self.entity(id).delta_movement;
            let last_speed = last_movement.horizontal_distance();
            let movement = self.update_fall_flying_movement(id, last_movement);
            self.entity_mut(id).set_delta_movement(movement);
            self.move_entity(id, MoverType::SelfMove, movement);
            let new_speed = self.entity(id).delta_movement.horizontal_distance();
            self.handle_fall_flying_collisions(id, last_speed, new_speed);
        }
    }

    pub fn stop_fall_flying(&mut self, id: EntityId) {
        let e = self.entity_mut(id);
        e.set_shared_flag(flags::FALL_FLYING, true);
        e.set_shared_flag(flags::FALL_FLYING, false);
    }

    fn update_fall_flying_movement(&self, id: EntityId, mut movement: Vec3) -> Vec3 {
        let e = self.entity(id);
        let look_angle = e.look_angle();
        let lean_angle = e.x_rot * mth::DEG_TO_RAD;
        let look_hor_length = (look_angle.x * look_angle.x + look_angle.z * look_angle.z).sqrt();
        let move_hor_length = movement.horizontal_distance();
        let gravity = self.effective_gravity(id);
        let lift_force = mth::square((lean_angle as f64).cos());
        movement = movement.add(0.0, gravity * (-1.0 + lift_force * 0.75), 0.0);
        if movement.y < 0.0 && look_hor_length > 0.0 {
            let convert = movement.y * -0.1 * lift_force;
            movement = movement.add(look_angle.x * convert / look_hor_length, convert, look_angle.z * convert / look_hor_length);
        }
        if lean_angle < 0.0 && look_hor_length > 0.0 {
            let convert = move_hor_length * -mth::sin(lean_angle as f64) as f64 * 0.04;
            movement = movement.add(-look_angle.x * convert / look_hor_length, convert * 3.2, -look_angle.z * convert / look_hor_length);
        }
        if look_hor_length > 0.0 {
            movement = movement.add(
                (look_angle.x / look_hor_length * move_hor_length - movement.x) * 0.1,
                0.0,
                (look_angle.z / look_hor_length * move_hor_length - movement.z) * 0.1,
            );
        }
        movement.multiply(0.99f32 as f64, 0.98f32 as f64, 0.99f32 as f64)
    }

    fn handle_fall_flying_collisions(&mut self, id: EntityId, move_hor_length: f64, new_move_hor_length: f64) {
        if self.entity(id).horizontal_collision {
            let diff = move_hor_length - new_move_hor_length;
            let dmg = (diff * 10.0 - 3.0) as f32;
            if dmg > 0.0 {
                let sound = self.get_fall_damage_sound(id, dmg as i32);
                self.entity_play_sound(id, sound_event(sound), 1.0, 1.0);
                self.hurt(id, DamageSource::new(damage_types::FLY_INTO_WALL), dmg);
            }
        }
    }

    fn travel_in_air(&mut self, id: EntityId, input: Vec3) {
        let pos_below = self.block_pos_below_that_affects_my_movement(id);
        let block_friction = if self.entity(id).on_ground {
            let friction = self.block_state(pos_below).block().info().friction;
            compute_modified_friction(friction, self.attribute_value(id, Attribute::FrictionModifier) as f32)
        } else {
            1.0
        };
        let movement = self.handle_relative_friction_and_calculate_movement(id, input, block_friction);
        let movement_y = match self.living(id).effect(mob_effects::LEVITATION) {
            Some(levitation) => movement.y + (0.05 * (levitation.amplifier + 1) as f64 - movement.y) * 0.2,
            None => movement.y - self.effective_gravity(id),
        };
        if self.living(id).discard_friction {
            self.entity_mut(id).set_delta_movement(Vec3::new(movement.x, movement_y, movement.z));
            return;
        }
        let air_drag_modifier = self.attribute_value(id, Attribute::AirDragModifier) as f32;
        let air_drag = compute_modified_friction(0.91, air_drag_modifier);
        let friction = block_friction * air_drag;
        let vertical_friction = compute_modified_friction(0.98, air_drag_modifier);
        let e = self.entity_mut(id);
        e.set_delta_movement(Vec3::new(movement.x * friction as f64, movement_y * vertical_friction as f64, movement.z * friction as f64));
    }

    fn travel_in_fluid(&mut self, id: EntityId, input: Vec3) {
        let e = self.entity(id);
        let is_falling = e.delta_movement.y <= 0.0;
        let old_y = e.y();
        let base_gravity = self.effective_gravity(id);
        if self.entity(id).is_in_water() {
            self.travel_in_water(id, input, base_gravity, is_falling, old_y);
        } else {
            self.travel_in_lava(id, input, base_gravity, is_falling, old_y);
        }
    }

    fn travel_in_water(&mut self, id: EntityId, input: Vec3, base_gravity: f64, is_falling: bool, old_y: f64) {
        if self.is_drowned(id) {
            self.drowned_travel_in_water(id, input, base_gravity, is_falling, old_y);
        } else if self.mob(id).is_some_and(|mob| matches!(mob.kind, MobKind::Axolotl(_))) {
            self.axolotl_travel_in_water(id, input, base_gravity, is_falling, old_y);
        } else if self.mob(id).is_some_and(|mob| matches!(mob.kind, MobKind::Frog(_))) {
            self.frog_travel_in_water(id, input, base_gravity, is_falling, old_y);
        } else if self.mob(id).is_some_and(Mob::is_abstract_fish) {
            self.abstract_fish_travel_in_water(id, input, base_gravity, is_falling, old_y);
        } else if self.is_turtle(id) {
            self.turtle_travel_in_water(id, input);
        } else {
            self.living_entity_travel_in_water(id, input, base_gravity, is_falling, old_y);
        }
    }

    pub fn living_entity_travel_in_water(&mut self, id: EntityId, input: Vec3, base_gravity: f64, is_falling: bool, old_y: f64) {
        let e = self.entity(id);
        let mut slow_down = if e.is_sprinting() { 0.9f32 } else { 0.8 };
        let mut speed = 0.02f32;
        let mut water_walker = self.attribute_value(id, Attribute::WaterMovementEfficiency) as f32;
        if !e.on_ground {
            water_walker *= 0.5;
        }
        if water_walker > 0.0 {
            slow_down += (0.54600006 - slow_down) * water_walker;
            speed += (self.get_speed(id) - speed) * water_walker;
        }
        if self.living(id).has_effect(mob_effects::DOLPHINS_GRACE) {
            slow_down = 0.96;
        }
        self.entity_mut(id).move_relative(speed, input);
        let delta = self.entity(id).delta_movement;
        self.move_entity(id, MoverType::SelfMove, delta);
        let mut movement = self.entity(id).delta_movement;
        if self.entity(id).horizontal_collision && self.on_climbable(id) {
            movement = Vec3::new(movement.x, 0.2, movement.z);
        }
        let movement = movement.multiply(slow_down as f64, 0.8f32 as f64, slow_down as f64);
        let adjusted = self.fluid_falling_adjusted_movement(id, base_gravity, is_falling, movement);
        self.entity_mut(id).set_delta_movement(adjusted);
        self.jump_out_of_fluid(id, old_y);
    }

    fn travel_in_lava(&mut self, id: EntityId, input: Vec3, base_gravity: f64, is_falling: bool, old_y: f64) {
        self.entity_mut(id).move_relative(0.02, input);
        let delta = self.entity(id).delta_movement;
        self.move_entity(id, MoverType::SelfMove, delta);
        let e = self.entity(id);
        if e.fluid_height(&fluid_tags::LAVA) <= e.fluid_jump_threshold() {
            let m = e.delta_movement.multiply(0.5, 0.8f32 as f64, 0.5);
            self.entity_mut(id).set_delta_movement(m);
            let adjusted = self.fluid_falling_adjusted_movement(id, base_gravity, is_falling, m);
            self.entity_mut(id).set_delta_movement(adjusted);
        } else {
            let m = e.delta_movement.scale(0.5);
            self.entity_mut(id).set_delta_movement(m);
        }
        if base_gravity != 0.0 {
            let e = self.entity_mut(id);
            e.set_delta_movement(e.delta_movement.add(0.0, -base_gravity / 4.0, 0.0));
        }
        self.jump_out_of_fluid(id, old_y);
    }

    fn jump_out_of_fluid(&mut self, id: EntityId, old_y: f64) {
        let e = self.entity(id);
        let m = e.delta_movement;
        if e.horizontal_collision && self.is_free(id, m.x, m.y + 0.6f32 as f64 - self.entity(id).y() + old_y, m.z) {
            self.entity_mut(id).set_delta_movement(Vec3::new(m.x, 0.3f32 as f64, m.z));
        }
    }

    fn fluid_falling_adjusted_movement(&self, id: EntityId, base_gravity: f64, is_falling: bool, movement: Vec3) -> Vec3 {
        if base_gravity != 0.0 && !self.entity(id).is_sprinting() {
            let yd = if is_falling && (movement.y - 0.005).abs() >= 0.003 && (movement.y - base_gravity / 16.0).abs() < 0.003 {
                -0.003
            } else {
                movement.y - base_gravity / 16.0
            };
            Vec3::new(movement.x, yd, movement.z)
        } else {
            movement
        }
    }

    fn handle_relative_friction_and_calculate_movement(&mut self, id: EntityId, input: Vec3, friction: f32) -> Vec3 {
        let speed = self.friction_influenced_speed(id, friction);
        self.entity_mut(id).move_relative(speed, input);
        let delta = self.entity(id).delta_movement;
        let delta = self.handle_on_climbable(id, delta);
        self.entity_mut(id).set_delta_movement(delta);
        let delta = self.entity(id).delta_movement;
        self.move_entity(id, MoverType::SelfMove, delta);
        let e = self.entity(id);
        let mut movement = e.delta_movement;
        let jumping = self.living(id).jumping;
        if (e.horizontal_collision || jumping) && (self.on_climbable(id) || self.entity(id).was_in_powder_snow && self.can_entity_walk_on_powder_snow(id)) {
            movement = Vec3::new(movement.x, 0.2, movement.z);
        }
        movement
    }

    pub fn can_entity_walk_on_powder_snow(&self, id: EntityId) -> bool {
        entity_type_tags::POWDER_SNOW_WALKABLE_MOBS.contains(self.entity(id).entity_type as usize)
            || self.is_living(id) && self.item_by_slot_ref(id, EquipmentSlot::Feet).is_some_and(|s| s.item == items::LEATHER_BOOTS)
    }

    fn friction_influenced_speed(&self, id: EntityId, block_friction: f32) -> f32 {
        if self.entity(id).on_ground {
            let speed = self.get_speed(id);
            if block_friction as f64 > 0.6 { speed * (0.21600002 / (block_friction * block_friction * block_friction)) } else { speed }
        } else {
            self.get_flying_speed(id)
        }
    }

    fn get_flying_speed(&self, id: EntityId) -> f32 {
        if self.is_player(id) { self.player_get_flying_speed(id) } else { 0.02 }
    }

    pub fn get_speed(&self, id: EntityId) -> f32 {
        if self.is_player(id) { self.player_get_speed(id) } else { self.living(id).speed }
    }

    pub fn set_speed(&mut self, id: EntityId, speed: f32) {
        match self.mob_mut(id) {
            Some(mob) => mob.set_speed(speed),
            None => self.living_mut(id).speed = speed,
        }
    }

    fn handle_on_climbable(&mut self, id: EntityId, delta: Vec3) -> Vec3 {
        if !self.on_climbable(id) {
            return delta;
        }
        self.reset_fall_distance(id);
        let max = 0.15f32 as f64;
        let xd = mth::clamp(delta.x, -max, max);
        let zd = mth::clamp(delta.z, -max, max);
        let mut yd = delta.y.max(-max);
        if yd < 0.0 && !self.in_block_state(id).is_block(blocks::SCAFFOLDING) && self.entity(id).is_shift_key_down() {
            yd = 0.0;
        }
        Vec3::new(xd, yd, zd)
    }

    pub fn on_climbable_peek(&self, id: EntityId) -> bool {
        if let Some(mob) = self.mob(id).filter(|mob| mob.is_spider()) {
            return super::monster::spider::spider::Spider::on_climbable(mob);
        }
        if self.is_abstract_horse(id) {
            return super::animal::equine::abstract_horse::on_climbable();
        }
        if self.is_spectator(id) {
            return false;
        }
        let pos = self.entity(id).block_position();
        let state_at = |pos: BlockPos| self.chunks.loaded_chunk(pos.chunk()).map_or(blocks::AIR.default_state(), |chunk| chunk.block_state(pos));
        let state = state_at(pos);
        if self.entity(id).is_fall_flying() && state.is(&block_tags::CAN_GLIDE_THROUGH) {
            return false;
        }
        state.is(&block_tags::CLIMBABLE)
            || state.block().is_instance_of("TrapDoorBlock") && state.get(OPEN) && {
                let below = state_at(pos.below());
                below.is_block(blocks::LADDER) && below.get(HORIZONTAL_FACING) == state.get(HORIZONTAL_FACING)
            }
    }

    pub fn on_climbable(&mut self, id: EntityId) -> bool {
        match self.mob(id) {
            Some(mob) if mob.is_spider() => super::monster::spider::spider::Spider::on_climbable(mob),
            Some(mob) if mob.abstract_horse().is_some() => super::animal::equine::abstract_horse::on_climbable(),
            Some(mob) if matches!(mob.kind, MobKind::Ghast(_)) => super::monster::ghast::Ghast::on_climbable(),
            _ => self.living_entity_on_climbable(id),
        }
    }

    pub fn living_entity_on_climbable(&mut self, id: EntityId) -> bool {
        if self.is_spectator(id) {
            return false;
        }
        let pos = self.entity(id).block_position();
        let state = self.in_block_state(id);
        if self.entity(id).is_fall_flying() && state.is(&block_tags::CAN_GLIDE_THROUGH) {
            return false;
        }
        let climbable = state.is(&block_tags::CLIMBABLE)
            || state.block().is_instance_of("TrapDoorBlock") && state.get(OPEN) && {
                let below = self.block_state(pos.below());
                below.is_block(blocks::LADDER) && below.get(HORIZONTAL_FACING) == state.get(HORIZONTAL_FACING)
            };
        if climbable {
            self.living_mut(id).last_climbable_pos = Some(pos);
        }
        climbable
    }

    pub fn has_effect(&self, id: EntityId, effect: MobEffect) -> bool {
        self.living(id).has_effect(effect)
    }

    pub fn add_effect(&mut self, id: EntityId, new_effect: MobEffectInstance, source: Option<EntityId>) -> bool {
        if !self.can_be_affected(id, &new_effect) {
            return false;
        }
        let (effect, amplifier) = (new_effect.effect, new_effect.amplifier);
        let living = &mut self.living_mut(id);
        let changed = match living.effect_index(effect) {
            Err(index) => {
                living.active_effects.insert(index, new_effect);
                living.effects_mod_count += 1;
                self.on_effect_added(id, effect, source);
                self.on_mob_effect_added(id, effect);
                true
            }
            Ok(index) => {
                let changed = living.active_effects[index].update(&new_effect);
                if changed {
                    self.on_effect_updated(id, effect, true, source);
                }
                changed
            }
        };
        self.on_effect_started(id, effect, amplifier);
        changed
    }

    pub fn can_be_affected(&self, id: EntityId, effect: &MobEffectInstance) -> bool {
        match self.mob(id).map(|mob| &mob.kind) {
            Some(MobKind::Spider(_) | MobKind::CaveSpider(_)) => self.spider_can_be_affected(id, effect),
            Some(MobKind::WitherSkeleton(_)) => self.wither_skeleton_can_be_affected(id, effect),
            _ => self.living_entity_can_be_affected(id, effect),
        }
    }

    pub fn living_entity_can_be_affected(&self, id: EntityId, effect: &MobEffectInstance) -> bool {
        let entity_type = self.entity(id).entity_type as usize;
        if entity_type_tags::IMMUNE_TO_INFESTED.contains(entity_type) {
            effect.effect != mob_effects::INFESTED
        } else if entity_type_tags::IMMUNE_TO_OOZING.contains(entity_type) {
            effect.effect != mob_effects::OOZING
        } else {
            !entity_type_tags::IGNORES_POISON_AND_REGEN.contains(entity_type) || effect.effect != mob_effects::REGENERATION && effect.effect != mob_effects::POISON
        }
    }

    pub fn remove_effect(&mut self, id: EntityId, effect: MobEffect) -> bool {
        match self.living_mut(id).remove_effect_no_update(effect) {
            Some(removed) => {
                self.on_effects_removed(id, &[removed]);
                true
            }
            None => false,
        }
    }

    pub fn remove_all_effects(&mut self, id: EntityId) -> bool {
        let living = &mut self.living_mut(id);
        if living.active_effects.is_empty() {
            return false;
        }
        let removed = std::mem::take(&mut living.active_effects);
        living.effects_mod_count += 1;
        self.on_effects_removed(id, &removed);
        true
    }

    fn on_effect_added(&mut self, id: EntityId, effect: MobEffect, source: Option<EntityId>) {
        if self.is_player(id) {
            self.server_player_on_effect_added(id, effect, source);
        } else {
            self.living_entity_on_effect_added(id, effect, source);
        }
    }

    pub fn living_entity_on_effect_added(&mut self, id: EntityId, effect: MobEffect, _source: Option<EntityId>) {
        let living = self.living_mut(id);
        living.effects_dirty = true;
        let amplifier = living.effect(effect).expect("added effect").amplifier;
        add_attribute_modifiers(effect, &mut living.attributes, amplifier);
    }

    fn on_effect_updated(&mut self, id: EntityId, effect: MobEffect, refresh_attributes: bool, source: Option<EntityId>) {
        if self.is_player(id) {
            self.server_player_on_effect_updated(id, effect, refresh_attributes, source);
        } else {
            self.living_entity_on_effect_updated(id, effect, refresh_attributes, source);
        }
    }

    pub fn living_entity_on_effect_updated(&mut self, id: EntityId, effect: MobEffect, refresh_attributes: bool, _source: Option<EntityId>) {
        let living = &mut self.living_mut(id);
        living.effects_dirty = true;
        if refresh_attributes {
            let amplifier = living.effect(effect).expect("updated effect").amplifier;
            remove_attribute_modifiers(effect, &mut living.attributes);
            add_attribute_modifiers(effect, &mut living.attributes, amplifier);
            self.refresh_dirty_attributes(id);
        }
    }

    fn on_effects_removed(&mut self, id: EntityId, effects: &[MobEffectInstance]) {
        if self.is_player(id) {
            self.server_player_on_effects_removed(id, effects);
        } else {
            self.living_entity_on_effects_removed(id, effects);
        }
    }

    pub fn living_entity_on_effects_removed(&mut self, id: EntityId, effects: &[MobEffectInstance]) {
        let living = &mut self.living_mut(id);
        living.effects_dirty = true;
        for effect in effects {
            remove_attribute_modifiers(effect.effect, &mut living.attributes);
        }
        self.refresh_dirty_attributes(id);
    }

    pub fn tick_effects(&mut self, id: EntityId) {
        let mut index = 0;
        loop {
            let (e, living) = self.entity_and_living(id);
            let Some(instance) = living.active_effects.get(index) else { return };
            let (effect, amplifier, mod_count) = (instance.effect, instance.amplifier, living.effects_mod_count);
            let tick_count = if instance.is_infinite_duration() { e.tick_count } else { instance.duration };
            let mut keep = instance.has_remaining_duration();
            if keep {
                if should_apply_effect_tick_this_tick(effect, tick_count, amplifier) && !self.apply_effect_tick(id, effect, amplifier) {
                    keep = false;
                } else if self.living(id).effects_mod_count == mod_count {
                    let instance = &mut self.living_mut(id).active_effects[index];
                    instance.tick_down_duration();
                    if instance.downgrade_to_hidden_effect() {
                        self.on_effect_updated(id, effect, true, None);
                    }
                    keep = self.living(id).active_effects[index].has_remaining_duration();
                }
            }
            let living = &mut self.living_mut(id);
            if living.effects_mod_count != mod_count {
                return;
            }
            if !keep {
                let removed = living.active_effects.remove(index);
                living.effects_mod_count += 1;
                self.on_effects_removed(id, &[removed]);
                continue;
            }
            if living.active_effects[index].duration % 600 == 0 {
                self.on_effect_updated(id, effect, false, None);
            }
            index += 1;
        }
    }

    pub fn trigger_on_death_mob_effects(&mut self, id: EntityId, reason: RemovalReason) {
        for index in 0..self.living(id).active_effects.len() {
            let effect = self.living(id).active_effects[index].effect;
            self.on_mob_removed(id, effect, reason);
        }
        let living = &mut self.living_mut(id);
        living.active_effects.clear();
        living.effects_mod_count += 1;
    }

    pub fn update_effect_visibility(&mut self, id: EntityId) {
        self.living_mut(id).effects_dirty = true;
    }

    pub fn update_dirty_effects(&mut self, id: EntityId) {
        if !self.living(id).effects_dirty {
            return;
        }
        self.update_invisibility_status(id);
        let glowing = self.is_currently_glowing(id);
        let (e, living) = self.entity_and_living_mut(id);
        if e.shared_flag(flags::GLOWING) != glowing {
            e.set_shared_flag(flags::GLOWING, glowing);
        }
        living.effects_dirty = false;
    }

    fn update_invisibility_status(&mut self, id: EntityId) {
        let spectator = self.is_spectator(id);
        let (e, living) = self.entity_and_living_mut(id);
        let effects = &living.active_effects;
        if spectator || effects.is_empty() {
            e.data.set(ids::EFFECT_PARTICLES, DataValue::Particles(Vec::new()));
            e.set_shared_flag(flags::INVISIBLE, spectator);
        } else {
            let invisible = living.has_effect(mob_effects::INVISIBILITY);
            e.set_shared_flag(flags::INVISIBLE, invisible);
            let particles = effects.iter().filter(|e| e.visible).map(MobEffectInstance::particle_options).collect();
            let ambient = are_all_effects_ambient(effects);
            e.data.set(ids::EFFECT_PARTICLES, DataValue::Particles(particles));
            e.data.set(ids::EFFECT_AMBIENCE, DataValue::Boolean(ambient));
        }
    }

    pub fn is_currently_glowing(&self, id: EntityId) -> bool {
        let (e, living) = self.entity_and_living(id);
        living.has_effect(mob_effects::GLOWING) || e.has_glowing_tag
    }

    pub fn has_water_breathing(&self, id: EntityId) -> bool {
        let living = &self.living(id);
        living.has_effect(mob_effects::WATER_BREATHING) || living.has_effect(mob_effects::CONDUIT_POWER) || living.has_effect(mob_effects::BREATH_OF_THE_NAUTILUS)
    }

    pub fn should_effects_refill_air_supply(&self, id: EntityId) -> bool {
        let living = &self.living(id);
        !living.has_effect(mob_effects::BREATH_OF_THE_NAUTILUS) || living.has_effect(mob_effects::WATER_BREATHING) || living.has_effect(mob_effects::CONDUIT_POWER)
    }

    pub fn get_sleeping_pos(&self, id: EntityId) -> Option<BlockPos> {
        match self.entity(id).data.get(ids::SLEEPING_POS) {
            DataValue::OptionalBlockPos(pos) => *pos,
            _ => None,
        }
    }

    pub fn set_sleeping_pos(&mut self, id: EntityId, bed_position: BlockPos) {
        self.entity_mut(id).data.set(ids::SLEEPING_POS, DataValue::OptionalBlockPos(Some(bed_position)));
    }

    pub fn clear_sleeping_pos(&mut self, id: EntityId) {
        self.entity_mut(id).data.set(ids::SLEEPING_POS, DataValue::OptionalBlockPos(None));
    }

    pub fn is_sleeping(&self, id: EntityId) -> bool {
        match self.mob(id).map(|mob| &mob.kind) {
            Some(MobKind::Fox(_)) => crate::world::entity::animal::fox::fox::is_sleeping(self.entity(id)),
            _ => self.living_entity_is_sleeping(id),
        }
    }

    pub fn living_entity_is_sleeping(&self, id: EntityId) -> bool {
        self.is_living(id) && self.get_sleeping_pos(id).is_some()
    }

    pub fn living_entity_start_sleeping(&mut self, id: EntityId, bed_position: BlockPos) -> bool {
        let block_state = self.block_state(bed_position);
        if abstract_bed_block::kind(block_state.block()).is_none() {
            return false;
        }
        let Some(sleep_height) = abstract_bed_block::get_sleep_height(block_state, self, bed_position) else { return false };
        if self.is_passenger(id) {
            self.stop_riding(id);
        }
        self.set_pos_to_bed(id, sleep_height, bed_position);
        self.set_block(bed_position, block_state.set(OCCUPIED, true), UPDATE_ALL);
        self.set_entity_pose(id, Pose::Sleeping);
        self.set_sleeping_pos(id, bed_position);
        let e = self.entity_mut(id);
        e.set_delta_movement(Vec3::ZERO);
        e.needs_sync = true;
        true
    }

    fn set_pos_to_bed(&mut self, id: EntityId, sleep_height: f64, bed_position: BlockPos) {
        self.entity_mut(id).set_pos(Vec3::new(bed_position.x as f64 + 0.5, bed_position.y as f64 + sleep_height + 0.125, bed_position.z as f64 + 0.5));
    }

    pub fn check_bed_exists(&mut self, id: EntityId) -> bool {
        let Some(bed_position) = self.get_sleeping_pos(id) else { return false };
        let state = self.block_state(bed_position);
        abstract_bed_block::kind(state.block()).is_some() && abstract_bed_block::get_sleep_height(state, self, bed_position).is_some()
    }

    pub fn living_entity_stop_sleeping(&mut self, id: EntityId) {
        if let Some(bed_position) = self.get_sleeping_pos(id).filter(|&pos| self.has_chunk_at(pos)) {
            let state = self.block_state(bed_position);
            if let Some(kind) = abstract_bed_block::kind(state.block()) {
                let facing = state.get(HORIZONTAL_FACING);
                self.set_block(bed_position, state.set(OCCUPIED, false), UPDATE_ALL);
                abstract_bed_block::on_stop_sleeping(kind, self, bed_position);
                let y_rot = self.entity(id).y_rot;
                let entity_type = self.dismount_type(id);
                let stand_up = abstract_bed_block::find_stand_up_position(entity_type, self, bed_position, facing, y_rot).unwrap_or_else(|| {
                    let above = bed_position.above();
                    Vec3::new(above.x as f64 + 0.5, above.y as f64 + 0.1, above.z as f64 + 0.5)
                });
                let look_direction = Vec3::at_bottom_center_of(bed_position).subtract(stand_up).normalize();
                let yaw = mth::wrap_degrees_d(mth::atan2(look_direction.z, look_direction.x) * mth::RAD_TO_DEG as f64 - 90.0) as f32;
                let e = self.entity_mut(id);
                e.set_pos(stand_up);
                e.set_y_rot(yaw);
                e.set_x_rot(0.0);
            }
        }
        let pos = self.entity(id).position();
        self.set_entity_pose(id, Pose::Standing);
        self.entity_mut(id).set_pos(pos);
        self.clear_sleeping_pos(id);
    }

    pub fn stop_sleeping(&mut self, id: EntityId) {
        if self.is_player(id) {
            self.stop_sleep_in_bed(id, true, true);
        } else if self.is_villager(id) {
            self.villager_stop_sleeping(id);
        } else {
            self.living_entity_stop_sleeping(id);
        }
    }

    pub fn dismount_type(&self, id: EntityId) -> crate::world::entity::vehicle::dismount_helper::DismountType {
        crate::world::entity::vehicle::dismount_helper::DismountType {
            dimensions: if self.is_player(id) { super::player::player::get_default_dimensions(Pose::Standing) } else { self.entity(id).dimensions },
            entity_type: super::entity_type::EntityType::by_id(self.entity(id).entity_type),
            is_player: self.is_player(id),
        }
    }

    pub fn set_entity_pose(&mut self, id: EntityId, pose: Pose) {
        if self.is_player(id) {
            self.set_player_pose(id, pose);
        } else if self.is_mob(id) && self.entity(id).pose() != pose {
            self.entity_mut(id).set_pose(pose);
            self.refresh_mob_dimensions(id);
        } else {
            self.entity_mut(id).set_pose(pose);
        }
    }
}

#[cfg(test)]
mod tests {
    use wafer_core::Vec3;
    use wafer_data::damage_types;

    use crate::server::level::{ServerLevel, testing};
    use crate::world::damagesource::DamageSource;
    use crate::world::entity::animal::chicken::Chicken;
    use crate::world::entity::{EntityId, LevelEntity};

    fn chicken(level: &mut ServerLevel, age: i32) -> EntityId {
        let mut mob = Chicken::create();
        mob.set_age(age);
        let id = mob.entity.id;
        assert!(level.add_fresh_entity(LevelEntity::Mob(Box::new(mob))));
        id
    }

    fn experience_orbs(level: &ServerLevel) -> usize {
        level.entities.values().filter(|e| matches!(e, LevelEntity::ExperienceOrb(_))).count()
    }

    #[test]
    fn held_weapons_apply_their_and_their_enchantments_attribute_modifiers() {
        let mut level = testing::level();
        let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
        let mut sword = crate::world::item::ItemStack::new(wafer_data::items::DIAMOND_SWORD, 1);
        crate::world::item::enchantment::enchantment_helper::enchant(&mut sword, "minecraft:sweeping_edge", 3);
        level.set_item_slot(player, wafer_data::EquipmentSlot::Mainhand, sword);
        level.detect_equipment_updates(player);
        assert_eq!(level.attribute_value(player, crate::world::entity::attributes::Attribute::AttackDamage), 7.0);
        assert_eq!(level.attribute_value(player, crate::world::entity::attributes::Attribute::SweepingDamageRatio), 0.75);
    }

    #[test]
    fn protection_enchantments_reduce_damage() {
        let damage_taken = |protection: i32| {
            let mut level = testing::level();
            let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
            let mut boots = crate::world::item::ItemStack::new(wafer_data::items::LEATHER_BOOTS, 1);
            crate::world::item::enchantment::enchantment_helper::enchant(&mut boots, "minecraft:protection", protection);
            level.set_item_slot(player, wafer_data::EquipmentSlot::Feet, boots);
            level.detect_equipment_updates(player);
            level.player_mut(player).movement.client_loaded_timeout_timer = 0;
            level.hurt_server(player, DamageSource::new(damage_types::MAGIC), 10.0);
            20.0 - level.health(player)
        };
        assert_eq!(damage_taken(0), 10.0);
        assert_eq!(damage_taken(4), 10.0 * (1.0 - 4.0 / 25.0));
    }

    #[test]
    fn player_attacks_knock_chickens_back() {
        let mut level = testing::level();
        let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
        let id = chicken(&mut level, 0);
        let attack = DamageSource { causing_entity: Some(player), direct_entity: Some(player), ..DamageSource::new(damage_types::PLAYER_ATTACK) };
        assert!(level.hurt_server(id, attack, 1.0));
        assert_eq!(level.health(id), 3.0);
        assert!(level.entity(id).needs_sync);
    }

    fn raised_shield(level: &mut ServerLevel, attacker_z: f64) -> (EntityId, EntityId) {
        let player = testing::player(level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
        level.player_mut(player).movement.client_loaded_timeout_timer = 0;
        level.entity_mut(player).y_head_rot = 0.0;
        level.set_item_slot(player, wafer_data::EquipmentSlot::Offhand, crate::world::item::ItemStack::new(wafer_data::items::SHIELD, 1));
        level.start_using_item(player, crate::world::InteractionHand::OffHand);
        level.living_mut(player).use_item_remaining -= 5;
        let attacker = testing::player_with_profile(level, crate::network::login::Profile { uuid: 2, name: "Attacker".into(), properties: Vec::new() }, Vec3::new(0.5, -60.0, attacker_z), 180.0, 0.0);
        (player, attacker)
    }

    #[test]
    fn raised_shields_block_attacks_from_the_front() {
        let mut level = testing::level();
        let (player, attacker) = raised_shield(&mut level, 2.5);
        let attack = DamageSource { causing_entity: Some(attacker), direct_entity: Some(attacker), ..DamageSource::new(damage_types::PLAYER_ATTACK) };
        assert!(!level.hurt_server(player, attack, 4.0));
        assert_eq!(level.health(player), 20.0);
        let shield = level.item_by_slot(player, wafer_data::EquipmentSlot::Offhand).unwrap();
        assert_eq!(shield.damage_value(), 5);
        assert!(level.is_using_item(player));
    }

    #[test]
    fn raised_shields_do_not_block_attacks_from_behind() {
        let mut level = testing::level();
        let (player, attacker) = raised_shield(&mut level, -1.5);
        let attack = DamageSource { causing_entity: Some(attacker), direct_entity: Some(attacker), ..DamageSource::new(damage_types::PLAYER_ATTACK) };
        assert!(level.hurt_server(player, attack, 4.0));
        assert_eq!(level.health(player), 16.0);
        assert_eq!(level.item_by_slot(player, wafer_data::EquipmentSlot::Offhand).unwrap().damage_value(), 0);
    }

    #[test]
    fn axes_disable_raised_shields() {
        let mut level = testing::level();
        let (player, attacker) = raised_shield(&mut level, 2.5);
        level.set_item_slot(attacker, wafer_data::EquipmentSlot::Mainhand, crate::world::item::ItemStack::new(wafer_data::items::IRON_AXE, 1));
        let attack = DamageSource { causing_entity: Some(attacker), direct_entity: Some(attacker), ..DamageSource::new(damage_types::PLAYER_ATTACK) };
        level.hurt_server(player, attack, 9.0);
        assert!(!level.is_using_item(player));
        let shield = crate::world::item::ItemStack::new(wafer_data::items::SHIELD, 1);
        assert!(level.player(player).cooldowns.is_on_cooldown(&shield));
    }

    #[test]
    fn cows_take_fall_damage_and_chickens_do_not() {
        let mut level = testing::level();
        let mut cow = crate::world::entity::animal::cow::cow::Cow::create();
        cow.entity.set_pos(Vec3::new(0.5, -60.0, 0.5));
        let id = cow.entity.id;
        assert!(level.add_fresh_entity(LevelEntity::Mob(Box::new(cow))));
        let cow = id;
        assert!(level.cause_fall_damage(cow, 10.0, 1.0, DamageSource::new(damage_types::FALL)));
        assert_eq!(level.health(cow), 3.0);
        let chicken = chicken(&mut level, 0);
        assert!(!level.cause_fall_damage(chicken, 10.0, 1.0, DamageSource::new(damage_types::FALL)));
        assert_eq!(level.health(chicken), 4.0);
    }

    #[test]
    fn mobs_holding_totems_are_protected_from_death() {
        let mut level = testing::level();
        let id = chicken(&mut level, 0);
        level.set_item_slot(id, wafer_data::EquipmentSlot::Offhand, crate::world::item::ItemStack::new(wafer_data::items::TOTEM_OF_UNDYING, 1));
        level.hurt_server(id, DamageSource::new(damage_types::GENERIC), 100.0);
        assert_eq!(level.health(id), 1.0);
        assert!(!level.living(id).dead);
        assert!(level.item_by_slot(id, wafer_data::EquipmentSlot::Offhand).is_none());
        assert!(level.has_effect(id, wafer_data::mob_effects::REGENERATION));
    }

    #[test]
    fn mobs_glide_only_while_wearing_a_glider() {
        let mut level = testing::level();
        testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
        let gliding = chicken(&mut level, 0);
        let falling = chicken(&mut level, 0);
        for id in [gliding, falling] {
            level.entity_mut(id).set_pos(Vec3::new(0.5, -20.0, 0.5));
            level.entity_mut(id).set_shared_flag(super::flags::FALL_FLYING, true);
        }
        level.set_item_slot(gliding, wafer_data::EquipmentSlot::Chest, crate::world::item::ItemStack::new(wafer_data::items::ELYTRA, 1));
        for _ in 0..20 {
            level.tick();
        }
        assert!(level.entity(gliding).is_fall_flying());
        assert!(!level.entity(falling).is_fall_flying());
        assert_eq!(level.living(gliding).fall_fly_ticks, 20);
        assert_eq!(level.item_by_slot(gliding, wafer_data::EquipmentSlot::Chest).unwrap().damage_value(), 1);
    }

    #[test]
    fn mobs_do_not_wear_out_their_armor() {
        let mut level = testing::level();
        let id = chicken(&mut level, 0);
        level.set_item_slot(id, wafer_data::EquipmentSlot::Head, crate::world::item::ItemStack::new(wafer_data::items::LEATHER_HELMET, 1));
        level.hurt_server(id, DamageSource::new(damage_types::MOB_ATTACK), 1.0);
        assert_eq!(level.item_by_slot(id, wafer_data::EquipmentSlot::Head).unwrap().damage_value(), 0);
    }

    #[test]
    fn players_wear_out_their_armor() {
        let mut level = testing::level();
        let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
        level.player_mut(player).movement.client_loaded_timeout_timer = 0;
        level.set_item_slot(player, wafer_data::EquipmentSlot::Head, crate::world::item::ItemStack::new(wafer_data::items::LEATHER_HELMET, 1));
        level.hurt_server(player, DamageSource::new(damage_types::MOB_ATTACK), 8.0);
        assert_eq!(level.item_by_slot(player, wafer_data::EquipmentSlot::Head).unwrap().damage_value(), 2);
    }

    #[test]
    fn temporarily_invulnerable_entities_are_not_hurt() {
        let mut level = testing::level();
        let id = chicken(&mut level, 0);
        level.entity_mut(id).invulnerable_time = 2;
        assert!(!level.hurt_server(id, DamageSource::new(damage_types::GENERIC), 1.0));
        level.common_tick(id);
        level.common_tick(id);
        assert!(level.hurt_server(id, DamageSource::new(damage_types::GENERIC), 1.0));
    }

    #[test]
    fn only_adult_chickens_killed_by_players_drop_experience() {
        let mut level = testing::level();
        let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
        let attack = DamageSource { causing_entity: Some(player), direct_entity: Some(player), ..DamageSource::new(damage_types::PLAYER_ATTACK) };
        let baby = chicken(&mut level, -24000);
        level.hurt_server(baby, attack, 100.0);
        assert!(level.living(baby).dead);
        assert_eq!(experience_orbs(&level), 0);
        let adult = chicken(&mut level, 0);
        level.hurt_server(adult, attack, 100.0);
        assert!(level.living(adult).dead);
        assert!(experience_orbs(&level) > 0);
    }
}
