use wafer_core::BlockPos;
use wafer_data::{Item, items, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use super::ItemStack;
use super::bucket_item::BucketItem;
use super::component as dc;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::mob::MobKind;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::level::server_level_accessor::ServerLevelAccessor;

pub struct MobBucketItem {
    entity_type: &'static str,
    empty_sound: String,
}

impl MobBucketItem {
    pub fn for_item(item: Item, fields: &serde_json::Value) -> Option<MobBucketItem> {
        let entity_type = match item {
            items::COD_BUCKET => "minecraft:cod",
            items::SALMON_BUCKET => "minecraft:salmon",
            items::PUFFERFISH_BUCKET => "minecraft:pufferfish",
            items::TROPICAL_FISH_BUCKET => "minecraft:tropical_fish",
            items::AXOLOTL_BUCKET => "minecraft:axolotl",
            items::TADPOLE_BUCKET => "minecraft:tadpole",
            items::SULFUR_CUBE_BUCKET => "minecraft:sulfur_cube",
            _ => return None,
        };
        let empty_sound = fields["emptySound"].as_str()?.to_string();
        Some(MobBucketItem { entity_type, empty_sound })
    }

    pub fn check_extra_content(&self, level: &mut ServerLevel, user: Option<EntityId>, item_stack: &ItemStack, pos: BlockPos) {
        self.spawn(level, item_stack, pos);
        level.game_event_at(user, crate::world::level::gameevent::game_event::ENTITY_PLACE, pos);
    }

    pub fn play_empty_sound(&self, level: &mut ServerLevel, user: Option<EntityId>, pos: BlockPos) {
        level.play_sound(user, pos, registry_id(registry::SOUND_EVENT, &self.empty_sound), SoundSource::Neutral, 1.0, 1.0);
    }

    fn spawn(&self, level: &mut ServerLevel, item_stack: &ItemStack, spawn_pos: BlockPos) {
        let Some(mut entity) = level.create_entity_type_from_stack(self.entity_type, Some(item_stack), spawn_pos, EntitySpawnReason::Bucket, true, false) else {
            return;
        };
        let mut age_loaded = false;
        if let LevelEntity::Mob(mob) = &mut entity
            && mob.is_bucketable()
        {
            let entity_data = item_stack.get(dc::BUCKET_ENTITY_DATA).map_or(Nbt::Compound(Vec::new().into()), Nbt::copy);
            mob.load_from_bucket_tag(&entity_data);
            age_loaded = matches!(mob.kind, MobKind::Tadpole(_)) && matches!(entity_data.get("Age"), Some(Nbt::Int(_)));
            mob.set_from_bucket(true);
        }
        if age_loaded {
            entity = level.tadpole_set_age_detached(entity);
        }
        let id = entity.entity().id;
        level.add_fresh_entity_with_passengers(entity);
        if level.mob(id).is_some() {
            level.play_ambient_sound(id);
        }
    }
}

impl BucketItem {
    pub fn mob_bucket_item_empty_contents(&self, level: &mut ServerLevel, user: Option<EntityId>, pos: BlockPos, hit: Option<crate::world::level::block::BlockHitResult>) -> bool {
        if self.content() == wafer_data::Fluid::Empty {
            self.play_empty_sound(level, user, pos);
            true
        } else {
            self.bucket_item_empty_contents(level, user, pos, hit)
        }
    }
}
