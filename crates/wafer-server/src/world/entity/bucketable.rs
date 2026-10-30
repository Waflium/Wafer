use wafer_data::items;
use wafer_protocol::nbt::{Nbt, TagMap};

use super::animal::axolotl::axolotl::{self, Axolotl};
use super::animal::frog::tadpole::{self, Tadpole};
use super::data::{DataValue, ids};
use super::mob::{Mob, MobKind};
use super::{EntityId, RemovalReason, sound_event};
use crate::advancements::triggers::criteria_triggers;
use crate::server::level::ServerLevel;
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::item::item_utils::{self, has_infinite_materials};
use crate::world::{InteractionHand, InteractionResult};

pub fn save_default_data_to_bucket_tag(entity: &Mob, bucket: &mut ItemStack) {
    let custom_name = match entity.entity.data.get(ids::CUSTOM_NAME) {
        DataValue::OptionalComponent(name) => name.clone(),
        _ => None,
    };
    bucket.set(dc::CUSTOM_NAME, custom_name);
    let mut tag = match bucket.get(dc::BUCKET_ENTITY_DATA) {
        Some(Nbt::Compound(tag)) => tag.copy(),
        _ => TagMap::new(),
    };
    let mut put_boolean = |key: &str, value: bool| tag.put(key, Nbt::Byte(value as i8));
    if entity.is_no_ai() {
        put_boolean("NoAI", entity.is_no_ai());
    }
    if entity.entity.is_silent() {
        put_boolean("Silent", entity.entity.is_silent());
    }
    if entity.entity.is_no_gravity() {
        put_boolean("NoGravity", entity.entity.is_no_gravity());
    }
    if entity.entity.has_glowing_tag {
        put_boolean("Glowing", entity.entity.has_glowing_tag);
    }
    if entity.entity.is_permanently_invulnerable() {
        put_boolean("Invulnerable", entity.entity.is_permanently_invulnerable());
    }
    if entity.is_persistence_required() {
        put_boolean("PersistenceRequired", entity.is_persistence_required());
    }
    tag.put("Health", Nbt::Float(entity.health));
    bucket.set(dc::BUCKET_ENTITY_DATA, Some(Nbt::Compound(tag)));
}

pub fn load_default_data_from_bucket_tag(entity: &mut Mob, tag: &Nbt) {
    let boolean = |key: &str| tag.get(key).and_then(Nbt::as_i64).map(|v| v != 0);
    if let Some(no_ai) = boolean("NoAI") {
        entity.set_no_ai(no_ai);
    }
    if let Some(silent) = boolean("Silent") {
        entity.entity.data.set(ids::SILENT, DataValue::Boolean(silent));
    }
    if let Some(no_gravity) = boolean("NoGravity") {
        entity.entity.data.set(ids::NO_GRAVITY, DataValue::Boolean(no_gravity));
    }
    if let Some(glowing) = boolean("Glowing") {
        entity.entity.has_glowing_tag = glowing;
        entity.entity.set_shared_flag(super::flags::GLOWING, glowing);
    }
    if let Some(invulnerable) = boolean("Invulnerable") {
        entity.entity.invulnerable = invulnerable;
    }
    if let Some(true) = boolean("PersistenceRequired") {
        entity.persistence_required = true;
    }
    if let Some(health) = tag.get("Health").and_then(Nbt::as_f64) {
        entity.set_health(health as f32);
    }
}

pub fn bucket_mob_pickup(level: &mut ServerLevel, player: EntityId, hand: InteractionHand, pickup_entity: EntityId) -> Option<InteractionResult> {
    let item_stack = level.player(player).item_in_hand(hand).unwrap_or_else(ItemStack::empty);
    let can_be_picked_up = if level.mob(pickup_entity).is_some_and(|mob| mob.sulfur_cube().is_some()) { item_stack.get_item() == items::BUCKET } else { can_be_picked_up_with_bucket(&item_stack) };
    if !(can_be_picked_up && level.is_alive(pickup_entity)) {
        return None;
    }
    let mob = level.mob(pickup_entity).expect("a bucketable mob");
    let pickup_sound = mob.get_pickup_sound();
    level.entity_play_sound(pickup_entity, sound_event(pickup_sound), 1.0, 1.0);
    let mob = level.mob(pickup_entity).expect("a bucketable mob");
    let mut bucket = mob.get_bucket_item_stack();
    mob.save_to_bucket_tag(&mut bucket);
    let triggered = if has_infinite_materials(level, player) { ItemStack::empty() } else { bucket.clone() };
    let result = item_utils::create_filled_result(level, player, hand, item_stack, bucket, false);
    level.player_mut(player).set_item_in_hand(hand, Some(result));
    criteria_triggers::FILLED_BUCKET.trigger(level, player, &triggered);
    level.drop_leash(pickup_entity);
    level.remove(pickup_entity, RemovalReason::Discarded);
    Some(InteractionResult::SUCCESS)
}

pub fn can_be_picked_up_with_bucket(item_stack: &ItemStack) -> bool {
    item_stack.get_item() == items::WATER_BUCKET
}

impl Mob {
    pub fn is_bucketable(&self) -> bool {
        self.is_abstract_fish() || matches!(self.kind, MobKind::Axolotl(_) | MobKind::SulfurCube(_))
    }

    pub fn save_to_bucket_tag(&self, bucket: &mut ItemStack) {
        match self.kind {
            MobKind::Salmon(_) => self.salmon_save_to_bucket_tag(bucket),
            MobKind::TropicalFish(_) => self.tropical_fish_save_to_bucket_tag(bucket),
            MobKind::Axolotl(_) => Axolotl::save_to_bucket_tag(self, bucket),
            MobKind::Tadpole(_) => Tadpole::save_to_bucket_tag(self, bucket),
            MobKind::SulfurCube(_) => self.sulfur_cube_save_to_bucket_tag(bucket),
            _ => self.abstract_fish_save_to_bucket_tag(bucket),
        }
    }

    pub fn load_from_bucket_tag(&mut self, tag: &Nbt) {
        match self.kind {
            MobKind::Axolotl(_) => Axolotl::load_from_bucket_tag(self, tag),
            MobKind::Tadpole(_) => Tadpole::load_from_bucket_tag(self, tag),
            MobKind::SulfurCube(_) => self.sulfur_cube_load_from_bucket_tag(tag),
            _ => self.abstract_fish_load_from_bucket_tag(tag),
        }
    }

    pub fn get_bucket_item_stack(&self) -> ItemStack {
        let item = match self.kind {
            MobKind::Cod(_) => items::COD_BUCKET,
            MobKind::Salmon(_) => items::SALMON_BUCKET,
            MobKind::Pufferfish(_) => items::PUFFERFISH_BUCKET,
            MobKind::TropicalFish(_) => items::TROPICAL_FISH_BUCKET,
            MobKind::Axolotl(_) => return axolotl::get_bucket_item_stack(),
            MobKind::Tadpole(_) => return tadpole::get_bucket_item_stack(),
            MobKind::SulfurCube(_) => items::SULFUR_CUBE_BUCKET,
            _ => unreachable!("not bucketable"),
        };
        ItemStack::new(item, 1)
    }

    pub fn get_pickup_sound(&self) -> &'static str {
        match self.kind {
            MobKind::Axolotl(_) => axolotl::get_pickup_sound(),
            MobKind::Tadpole(_) => tadpole::get_pickup_sound(),
            MobKind::SulfurCube(_) => "minecraft:item.bucket.fill_sulfur_cube",
            _ => super::animal::fish::abstract_fish::AbstractFish::get_pickup_sound(),
        }
    }
}
