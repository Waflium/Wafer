use wafer_core::Vec3;
use wafer_data::{EquipmentSlot, damage_type_tags, game_rule, items, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use crate::server::level::ServerLevel;
use crate::util::codec::{self, DecodeResult};
use crate::util::extra_codecs;
use crate::world::level::storage::tag_value_input;
use crate::world::damagesource::DamageSource;
use crate::world::entity::attributes::{ARMOR_STAND_ATTRIBUTES, Attribute};
use crate::world::entity::data::{DataValue, ids, living_entity_defaults};
use crate::world::entity::living_entity::{self, LivingEntity};
use crate::world::entity::{Entity, EntityDimensions, EntityId, LevelEntity, RemovalReason, entity_event, sound_event};
use crate::world::item::ItemStack;
use crate::world::item::component::{self as dc};
use crate::world::{InteractionHand, InteractionResult};

pub const WOBBLE_TIME: i64 = 5;
pub const DEFAULT_HEAD_POSE: [f32; 3] = [0.0, 0.0, 0.0];
pub const DEFAULT_BODY_POSE: [f32; 3] = [0.0, 0.0, 0.0];
pub const DEFAULT_LEFT_ARM_POSE: [f32; 3] = [-10.0, 0.0, -10.0];
pub const DEFAULT_RIGHT_ARM_POSE: [f32; 3] = [-15.0, 0.0, 10.0];
pub const DEFAULT_LEFT_LEG_POSE: [f32; 3] = [-1.0, 0.0, -1.0];
pub const DEFAULT_RIGHT_LEG_POSE: [f32; 3] = [1.0, 0.0, 1.0];
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.5, 1.975).with_eye_height(1.7775);
const MARKER_DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.0, 0.0);
const BABY_DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.25, 0.9875).with_eye_height(0.9875);
const FEET_OFFSET: f64 = 0.1;
const CHEST_OFFSET: f64 = 0.9;
const LEGS_OFFSET: f64 = 0.4;
const HEAD_OFFSET: f64 = 1.6;
pub const DISABLE_TAKING_OFFSET: i32 = 8;
pub const DISABLE_PUTTING_OFFSET: i32 = 16;
pub const CLIENT_FLAG_SMALL: i8 = 1;
pub const CLIENT_FLAG_SHOW_ARMS: i8 = 4;
pub const CLIENT_FLAG_NO_BASEPLATE: i8 = 8;
pub const CLIENT_FLAG_MARKER: i8 = 16;
pub const DATA_CLIENT_FLAGS: usize = 15;
pub const DATA_HEAD_POSE: usize = 16;
pub const DATA_BODY_POSE: usize = 17;
pub const DATA_LEFT_ARM_POSE: usize = 18;
pub const DATA_RIGHT_ARM_POSE: usize = 19;
pub const DATA_LEFT_LEG_POSE: usize = 20;
pub const DATA_RIGHT_LEG_POSE: usize = 21;
const POSES: [(&str, usize, [f32; 3]); 6] = [
    ("Head", DATA_HEAD_POSE, DEFAULT_HEAD_POSE),
    ("Body", DATA_BODY_POSE, DEFAULT_BODY_POSE),
    ("LeftArm", DATA_LEFT_ARM_POSE, DEFAULT_LEFT_ARM_POSE),
    ("RightArm", DATA_RIGHT_ARM_POSE, DEFAULT_RIGHT_ARM_POSE),
    ("LeftLeg", DATA_LEFT_LEG_POSE, DEFAULT_LEFT_LEG_POSE),
    ("RightLeg", DATA_RIGHT_LEG_POSE, DEFAULT_RIGHT_LEG_POSE),
];

pub struct ArmorStand {
    pub entity: Entity,
    pub living: LivingEntity,
    pub health: f32,
    pub absorption: f32,
    pub equipment: [Option<ItemStack>; 8],
    invisible: bool,
    pub last_hit: i64,
    disabled_slots: i32,
}

fn decode_armor_stand_pose(tag: &Nbt) -> DecodeResult<Vec<Option<[f32; 3]>>> {
    if let Err(error) = codec::map(tag) {
        return DecodeResult::error(error);
    }
    let results: Vec<DecodeResult<[f32; 3]>> = POSES
        .iter()
        .map(|(name, _, _)| match tag.get(name) {
            Some(rotation) => extra_codecs::fixed_size_list_partial(rotation, 3, |element| codec::float(element).into()).map(|v| rotations(v[0], v[1], v[2])),
            None => DecodeResult { value: None, error: None },
        })
        .collect();
    let errors: Vec<Option<&String>> = results.iter().map(|result| result.error.as_ref()).collect();
    let error = codec::record_errors(&errors).err();
    DecodeResult { value: Some(results.into_iter().map(|result| result.value).collect()), error }
}

pub fn rotations(x: f32, y: f32, z: f32) -> [f32; 3] {
    let sanitize = |v: f32| if v.is_finite() { v % 360.0 } else { 0.0 };
    [sanitize(x), sanitize(y), sanitize(z)]
}

fn same_rotations(a: [f32; 3], b: [f32; 3]) -> bool {
    a.iter().zip(b.iter()).all(|(a, b)| a.to_bits() == b.to_bits())
}

fn filter_bit(slot: EquipmentSlot, offset: i32) -> i32 {
    let id = match slot {
        EquipmentSlot::Mainhand => 0,
        EquipmentSlot::Feet => 1,
        EquipmentSlot::Legs => 2,
        EquipmentSlot::Chest => 3,
        EquipmentSlot::Head => 4,
        EquipmentSlot::Offhand => 5,
        EquipmentSlot::Body => 6,
        EquipmentSlot::Saddle => 7,
    };
    id + offset
}

fn is_hand(slot: EquipmentSlot) -> bool {
    matches!(slot, EquipmentSlot::Mainhand | EquipmentSlot::Offhand)
}

impl ArmorStand {
    pub fn of_type() -> ArmorStand {
        let mut data = living_entity_defaults();
        data.push(DataValue::Byte(0));
        for (_, _, default) in POSES {
            data.push(DataValue::Rotations(default));
        }
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:armor_stand"), DIMENSIONS, data);
        let living = LivingEntity::new(&ARMOR_STAND_ATTRIBUTES);
        let mut stand = ArmorStand { entity, living, health: 1.0, absorption: 0.0, equipment: Default::default(), invisible: false, last_hit: 0, disabled_slots: 0 };
        let max_health = stand.living.attributes.value(Attribute::MaxHealth) as f32;
        stand.set_health(max_health);
        stand.entity.reapply_position();
        stand
    }

    pub fn at(x: f64, y: f64, z: f64) -> ArmorStand {
        let mut stand = ArmorStand::of_type();
        stand.entity.set_pos(Vec3::new(x, y, z));
        stand
    }

    pub fn set_health(&mut self, health: f32) {
        let max_health = self.living.attributes.value(Attribute::MaxHealth) as f32;
        self.health = health.clamp(0.0, max_health);
        self.entity.data.set(ids::HEALTH, DataValue::Float(self.health));
    }

    fn client_flags(&self) -> i8 {
        match self.entity.data.get(DATA_CLIENT_FLAGS) {
            DataValue::Byte(flags) => *flags,
            _ => 0,
        }
    }

    fn set_client_flag(&mut self, bit: i8, value: bool) {
        let flags = self.client_flags();
        self.entity.data.set(DATA_CLIENT_FLAGS, DataValue::Byte(if value { flags | bit } else { flags & !bit }));
        self.on_client_flags_updated();
    }

    fn on_client_flags_updated(&mut self) {
        self.refresh_dimensions();
    }

    pub fn refresh_dimensions(&mut self) {
        let position = self.entity.position();
        let scale = self.living.attributes.value(Attribute::Scale) as f32;
        self.entity.dimensions = self.get_default_dimensions().scale(scale);
        self.entity.reapply_position();
        self.entity.set_pos(position);
    }

    pub fn get_default_dimensions(&self) -> EntityDimensions {
        self.get_dimensions_marker(self.is_marker())
    }

    fn get_dimensions_marker(&self, is_marker: bool) -> EntityDimensions {
        if is_marker {
            MARKER_DIMENSIONS
        } else if self.is_small() {
            BABY_DIMENSIONS
        } else {
            DIMENSIONS
        }
    }

    pub fn is_small(&self) -> bool {
        self.client_flags() & CLIENT_FLAG_SMALL != 0
    }

    fn set_small(&mut self, value: bool) {
        self.set_client_flag(CLIENT_FLAG_SMALL, value);
    }

    pub fn show_arms(&self) -> bool {
        self.client_flags() & CLIENT_FLAG_SHOW_ARMS != 0
    }

    pub fn set_show_arms(&mut self, value: bool) {
        self.set_client_flag(CLIENT_FLAG_SHOW_ARMS, value);
    }

    pub fn show_base_plate(&self) -> bool {
        self.client_flags() & CLIENT_FLAG_NO_BASEPLATE == 0
    }

    pub fn set_no_base_plate(&mut self, value: bool) {
        self.set_client_flag(CLIENT_FLAG_NO_BASEPLATE, value);
    }

    pub fn is_marker(&self) -> bool {
        self.client_flags() & CLIENT_FLAG_MARKER != 0
    }

    fn set_marker(&mut self, value: bool) {
        self.set_client_flag(CLIENT_FLAG_MARKER, value);
    }

    pub fn has_physics(&self) -> bool {
        !self.is_marker() && !self.entity.is_no_gravity()
    }

    pub fn is_invisible(&self) -> bool {
        self.invisible
    }

    pub fn set_invisible(&mut self, invisible: bool) {
        self.invisible = invisible;
        self.entity.set_shared_flag(crate::world::entity::flags::INVISIBLE, invisible);
    }

    fn pose(&self, accessor: usize) -> [f32; 3] {
        match self.entity.data.get(accessor) {
            DataValue::Rotations(rotations) => *rotations,
            _ => [0.0; 3],
        }
    }

    fn set_pose(&mut self, accessor: usize, rotations: [f32; 3]) {
        self.entity.data.set(accessor, DataValue::Rotations(rotations));
    }

    pub fn item_by_slot(&self, slot: EquipmentSlot) -> Option<&ItemStack> {
        self.equipment[slot as usize].as_ref().filter(|stack| !stack.is_empty())
    }

    fn has_item_in_slot(&self, slot: EquipmentSlot) -> bool {
        self.item_by_slot(slot).is_some()
    }

    pub fn can_use_slot(&self, slot: EquipmentSlot) -> bool {
        slot != EquipmentSlot::Body && slot != EquipmentSlot::Saddle && !self.is_disabled(slot)
    }

    fn is_disabled(&self, slot: EquipmentSlot) -> bool {
        self.disabled_slots & (1 << filter_bit(slot, 0)) != 0 || is_hand(slot) && !self.show_arms()
    }

    fn get_clicked_slot(&self, location: Vec3) -> EquipmentSlot {
        let mut slot_clicked = EquipmentSlot::Mainhand;
        let small = self.is_small();
        let scale = self.living.attributes.value(Attribute::Scale) as f32 * if self.is_small() { 0.5 } else { 1.0 };
        let click_y_position = location.y / scale as f64;
        if click_y_position >= FEET_OFFSET && click_y_position < FEET_OFFSET + if small { 0.8 } else { 0.45 } && self.has_item_in_slot(EquipmentSlot::Feet) {
            slot_clicked = EquipmentSlot::Feet;
        } else if click_y_position >= CHEST_OFFSET + if small { 0.3 } else { 0.0 }
            && click_y_position < CHEST_OFFSET + if small { 1.0 } else { 0.7 }
            && self.has_item_in_slot(EquipmentSlot::Chest)
        {
            slot_clicked = EquipmentSlot::Chest;
        } else if click_y_position >= LEGS_OFFSET && click_y_position < LEGS_OFFSET + if small { 1.0 } else { 0.8 } && self.has_item_in_slot(EquipmentSlot::Legs) {
            slot_clicked = EquipmentSlot::Legs;
        } else if click_y_position >= HEAD_OFFSET && self.has_item_in_slot(EquipmentSlot::Head) {
            slot_clicked = EquipmentSlot::Head;
        } else if !self.has_item_in_slot(EquipmentSlot::Mainhand) && self.has_item_in_slot(EquipmentSlot::Offhand) {
            slot_clicked = EquipmentSlot::Offhand;
        }
        slot_clicked
    }

    pub fn add_additional_save_data(&self, output: &mut Vec<(String, Nbt)>) {
        living_entity::add_additional_save_data(&self.entity, &self.living, self.health, self.absorption, &self.equipment, output);
        output.push(("Invisible".into(), Nbt::Byte(self.is_invisible() as i8)));
        output.push(("Small".into(), Nbt::Byte(self.is_small() as i8)));
        output.push(("ShowArms".into(), Nbt::Byte(self.show_arms() as i8)));
        output.push(("DisabledSlots".into(), Nbt::Int(self.disabled_slots)));
        output.push(("NoBasePlate".into(), Nbt::Byte(!self.show_base_plate() as i8)));
        if self.is_marker() {
            output.push(("Marker".into(), Nbt::Byte(1)));
        }
        let pose: Vec<(String, Nbt)> = POSES
            .iter()
            .filter(|(_, accessor, default)| !same_rotations(self.pose(*accessor), *default))
            .map(|(name, accessor, _)| (name.to_string(), Nbt::List(self.pose(*accessor).iter().map(|v| Nbt::Float(*v)).collect())))
            .collect();
        output.push(("Pose".into(), Nbt::Compound(pose)));
    }

    pub fn read_additional_save_data(&mut self, nbt: &Nbt) {
        let int = |key: &str| tag_value_input::get_numeric(nbt, key).and_then(crate::world::level::block::entity::int_value);
        let boolean = |key: &str| tag_value_input::get_boolean(nbt, key).unwrap_or(false);
        let y_rot = self.entity.y_rot;
        self.set_y_head_rot(y_rot);
        self.set_y_body_rot(y_rot);
        living_entity::read_additional_save_data(&mut self.entity, &mut self.living, &mut self.health, &mut self.absorption, &mut self.equipment, nbt);
        self.set_invisible(boolean("Invisible"));
        self.set_small(boolean("Small"));
        self.set_show_arms(boolean("ShowArms"));
        self.disabled_slots = int("DisabledSlots").unwrap_or(0);
        self.set_no_base_plate(boolean("NoBasePlate"));
        self.set_marker(boolean("Marker"));
        self.entity.no_physics = !self.has_physics();
        if let Some(poses) = tag_value_input::read(nbt, "Pose", decode_armor_stand_pose) {
            for ((_, accessor, default), pose) in POSES.into_iter().zip(poses) {
                self.set_pose(accessor, pose.unwrap_or(default));
            }
        }
    }

    pub fn set_y_body_rot(&mut self, y_body_rot: f32) {
        self.entity.y_rot_o = y_body_rot;
        self.living.y_body_rot_o = y_body_rot;
        self.entity.y_head_rot = y_body_rot;
        self.living.y_head_rot_o = y_body_rot;
    }

    pub fn set_y_head_rot(&mut self, y_head_rot: f32) {
        self.entity.y_rot_o = y_head_rot;
        self.living.y_body_rot_o = y_head_rot;
        self.entity.y_head_rot = y_head_rot;
        self.living.y_head_rot_o = y_head_rot;
    }
}

pub fn get_hurt_sound() -> &'static str {
    "minecraft:entity.armor_stand.hit"
}

pub fn get_death_sound() -> &'static str {
    "minecraft:entity.armor_stand.break"
}

pub fn get_fall_sound() -> &'static str {
    "minecraft:entity.armor_stand.fall"
}

impl ServerLevel {
    pub fn is_armor_stand(&self, id: EntityId) -> bool {
        matches!(self.entities.get(&id), Some(LevelEntity::ArmorStand(_)))
    }

    pub fn armor_stand(&self, id: EntityId) -> Option<&ArmorStand> {
        match self.entities.get(&id) {
            Some(LevelEntity::ArmorStand(stand)) => Some(stand),
            _ => None,
        }
    }

    pub fn armor_stand_mut(&mut self, id: EntityId) -> Option<&mut ArmorStand> {
        match self.entities.get_mut(&id) {
            Some(LevelEntity::ArmorStand(stand)) => Some(stand),
            _ => None,
        }
    }

    pub fn tick_armor_stand(&mut self, id: EntityId) {
        self.living_entity_tick(id);
    }

    pub fn armor_stand_tick_head_turn(&mut self, id: EntityId) {
        let y_rot_o = self.entity(id).y_rot_o;
        let y_rot = self.entity(id).y_rot;
        let living = self.living_mut(id);
        living.y_body_rot_o = y_rot_o;
        living.y_body_rot = y_rot;
    }

    pub fn armor_stand_push_entities(&mut self, id: EntityId) {
        let bb = self.entity(id).bounding_box();
        let minecarts = self.get_entities_matching(Some(id), &bb, |level, entity| matches!(level.entities.get(&entity), Some(LevelEntity::Minecart(minecart)) if minecart.is_rideable()));
        for entity in minecarts {
            if self.entity(id).distance_to_sqr(self.entity(entity)) <= 0.2 {
                self.push_entity(entity, id);
            }
        }
    }

    pub fn armor_stand_interact(&mut self, id: EntityId, player: EntityId, hand: InteractionHand, location: Vec3) -> InteractionResult {
        let item_stack = self.player(player).item_in_hand(hand).unwrap_or_else(ItemStack::empty);
        if self.armor_stand(id).expect("an armor stand").is_marker() || item_stack.is(items::NAME_TAG) {
            return self.entity_interact(id, player, hand, location);
        }
        if self.is_spectator(player) {
            return InteractionResult::SUCCESS;
        }
        let item_in_hand_slot = self.get_equipment_slot_for_item(id, &item_stack);
        let stand = self.armor_stand(id).expect("an armor stand");
        if item_stack.is_empty() {
            let clicked_slot = stand.get_clicked_slot(location);
            let target_slot = if stand.is_disabled(clicked_slot) { item_in_hand_slot } else { clicked_slot };
            if stand.has_item_in_slot(target_slot) && self.armor_stand_swap_item(id, player, target_slot, item_stack, hand) {
                return InteractionResult::SUCCESS_SERVER;
            }
        } else {
            if stand.is_disabled(item_in_hand_slot) {
                return InteractionResult::Fail;
            }
            if is_hand(item_in_hand_slot) && !stand.show_arms() {
                return InteractionResult::Fail;
            }
            if self.armor_stand_swap_item(id, player, item_in_hand_slot, item_stack, hand) {
                return InteractionResult::SUCCESS_SERVER;
            }
        }
        self.entity_interact(id, player, hand, location)
    }

    fn armor_stand_swap_item(&mut self, id: EntityId, player: EntityId, slot: EquipmentSlot, mut player_item_stack: ItemStack, hand: InteractionHand) -> bool {
        let stand = self.armor_stand(id).expect("an armor stand");
        let item_stack = stand.item_by_slot(slot).cloned().unwrap_or_else(ItemStack::empty);
        if !item_stack.is_empty() && stand.disabled_slots & (1 << filter_bit(slot, DISABLE_TAKING_OFFSET)) != 0 {
            return false;
        }
        if item_stack.is_empty() && stand.disabled_slots & (1 << filter_bit(slot, DISABLE_PUTTING_OFFSET)) != 0 {
            return false;
        }
        if crate::world::item::item_utils::has_infinite_materials(self, player) && item_stack.is_empty() && !player_item_stack.is_empty() {
            self.living_set_item_slot(id, slot, player_item_stack.copy_with_count(1));
            return true;
        }
        if player_item_stack.is_empty() || player_item_stack.count <= 1 {
            self.living_set_item_slot(id, slot, player_item_stack);
            self.player_mut(player).set_item_in_hand(hand, Some(item_stack));
            return true;
        }
        if !item_stack.is_empty() {
            return false;
        }
        let split = player_item_stack.split(1);
        self.living_set_item_slot(id, slot, split);
        self.player_mut(player).update_item_in_hand(hand, &player_item_stack);
        true
    }

    pub fn armor_stand_hurt_server(&mut self, id: EntityId, source: DamageSource, _damage: f32) -> bool {
        if self.entity(id).is_removed() {
            return false;
        }
        if !self.game_rules.get_bool(game_rule::MOB_GRIEFING) && source.causing_entity.is_some_and(|entity| self.is_mob(entity)) {
            return false;
        }
        if source.is(&damage_type_tags::BYPASSES_INVULNERABILITY) {
            self.armor_stand_kill(id, source.causing_entity);
            return false;
        }
        let stand = self.armor_stand(id).expect("an armor stand");
        let (invisible, marker) = (stand.invisible, stand.is_marker());
        if self.is_invulnerable_to(id, &source) || invisible || marker {
            return false;
        }
        if source.is(&damage_type_tags::IS_EXPLOSION) {
            self.armor_stand_broken_by_anything(id, &source);
            self.armor_stand_kill(id, source.causing_entity);
            return false;
        }
        if source.is(&damage_type_tags::IGNITES_ARMOR_STANDS) {
            if self.is_on_fire(id) {
                self.armor_stand_cause_damage(id, &source, 0.15);
            } else {
                self.ignite_for_seconds(id, 5.0);
            }
            return false;
        }
        if source.is(&damage_type_tags::BURNS_ARMOR_STANDS) && self.armor_stand(id).expect("an armor stand").health > 0.5 {
            self.armor_stand_cause_damage(id, &source, 4.0);
            return false;
        }
        let allow_incremental_breaking = source.is(&damage_type_tags::CAN_BREAK_ARMOR_STAND);
        let should_kill = source.is(&damage_type_tags::ALWAYS_KILLS_ARMOR_STANDS);
        if !allow_incremental_breaking && !should_kill {
            return false;
        }
        if source.causing_entity.is_some_and(|entity| self.is_player(entity) && !self.player(entity).abilities.may_build) {
            return false;
        }
        if source.is_creative_player(self) {
            self.armor_stand_play_broken_sound(id);
            self.armor_stand_show_breaking_particles(id);
            self.armor_stand_kill(id, source.causing_entity);
            return true;
        }
        let time = self.game_time;
        if time - self.armor_stand(id).expect("an armor stand").last_hit > WOBBLE_TIME && !should_kill {
            self.broadcast_entity_event(id, entity_event::ARMORSTAND_WOBBLE);
            self.entity_game_event_by(id, crate::world::level::gameevent::game_event::ENTITY_DAMAGE, source.causing_entity);
            self.armor_stand_mut(id).expect("an armor stand").last_hit = time;
        } else {
            self.armor_stand_broken_by_player(id, &source);
            self.armor_stand_show_breaking_particles(id);
            self.armor_stand_kill(id, source.causing_entity);
        }
        true
    }

    fn armor_stand_show_breaking_particles(&mut self, id: EntityId) {
        let e = self.entity(id);
        let (x, y, z) = (e.x(), e.get_y(0.6666666666666666), e.z());
        let (width, height) = (e.dimensions.width, e.dimensions.height);
        let state = wafer_data::blocks::OAK_PLANKS.default_state();
        self.send_particles(
            "minecraft:block",
            |w| w.varint(state.id() as i32),
            x,
            y,
            z,
            10,
            ((width / 4.0) as f64, (height / 4.0) as f64, (width / 4.0) as f64),
            0.05,
        );
    }

    fn armor_stand_cause_damage(&mut self, id: EntityId, source: &DamageSource, dmg: f32) {
        let mut health = self.armor_stand(id).expect("an armor stand").health;
        health -= dmg;
        if health <= 0.5 {
            self.armor_stand_broken_by_anything(id, source);
            self.armor_stand_kill(id, source.causing_entity);
        } else {
            self.set_health(id, health);
            self.entity_game_event_by(id, crate::world::level::gameevent::game_event::ENTITY_DAMAGE, source.causing_entity);
        }
    }

    fn armor_stand_broken_by_player(&mut self, id: EntityId, source: &DamageSource) {
        let mut result = ItemStack::new(items::ARMOR_STAND, 1);
        let custom_name = match self.entity(id).data.get(ids::CUSTOM_NAME) {
            DataValue::OptionalComponent(Some(name)) => Some(name.clone()),
            _ => None,
        };
        result.set(dc::CUSTOM_NAME, custom_name);
        let block_position = self.entity(id).block_position();
        self.pop_resource(block_position, result);
        self.armor_stand_broken_by_anything(id, source);
    }

    fn armor_stand_broken_by_anything(&mut self, id: EntityId, source: &DamageSource) {
        self.armor_stand_play_broken_sound(id);
        self.drop_all_death_loot(id, source);
        for slot in EquipmentSlot::VALUES {
            let item_stack = self.armor_stand_mut(id).expect("an armor stand").equipment[slot as usize].take().unwrap_or_else(ItemStack::empty);
            if !item_stack.is_empty() && !crate::world::item::enchantment::enchantment_helper::has_prevent_equipment_drop(&item_stack) {
                let above = self.entity(id).block_position().above();
                self.pop_resource(above, item_stack);
            }
        }
    }

    fn armor_stand_play_broken_sound(&mut self, id: EntityId) {
        let e = self.entity(id);
        let (x, y, z) = (e.x(), e.y(), e.z());
        let source = self.sound_source(id);
        self.play_sound_at(None, x, y, z, sound_event("minecraft:entity.armor_stand.break"), source, 1.0, 1.0);
    }

    pub fn armor_stand_kill(&mut self, id: EntityId, attributed_to: Option<EntityId>) {
        self.remove(id, RemovalReason::Killed);
        self.entity_game_event_by(id, crate::world::level::gameevent::game_event::ENTITY_DIE, Some(attributed_to.unwrap_or(id)));
    }

    pub fn armor_stand_ignore_explosion(&self, id: EntityId, should_affect_blocklike_entities: bool) -> bool {
        if should_affect_blocklike_entities { self.armor_stand(id).expect("an armor stand").is_invisible() } else { true }
    }

    pub fn armor_stand_skip_attack_interaction(&self, id: EntityId, source: EntityId) -> bool {
        self.is_player(source) && !self.may_interact(source, self.entity(id).block_position())
    }

    pub fn armor_stand_can_be_seen_by_anyone(&self, id: EntityId) -> bool {
        let stand = self.armor_stand(id).expect("an armor stand");
        !stand.is_invisible() && !stand.is_marker()
    }
}
