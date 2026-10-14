use wafer_core::BlockPos;
use wafer_data::{EquipmentSlot, game_rule};

use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::EntityId;
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::entity_type::EntityType;
use crate::world::entity::mob;
use crate::world::item::{ItemStack, projectile_weapon_item};
use crate::world::level::dimension::dimension_type;
use crate::world::level::light_layer::LightLayer;
use crate::world::level::server_level_accessor::ServerLevelAccessor;

pub const XP_REWARD: i32 = 5;

impl ServerLevel {
    pub fn monster_get_sound_source(&self) -> SoundSource {
        SoundSource::Hostile
    }

    pub fn monster_ai_step(&mut self, id: EntityId) {
        self.update_no_action_time(id);
        self.mob_ai_step(id);
    }

    pub fn update_no_action_time(&mut self, id: EntityId) {
        let br = self.get_entity_light_level_dependent_magic_value(id);
        if br > 0.5 {
            self.living_mut(id).no_action_time += 2;
        }
    }

    pub fn monster_get_swim_sound(&self) -> &'static str {
        "minecraft:entity.hostile.swim"
    }

    pub fn monster_get_swim_splash_sound(&self) -> &'static str {
        "minecraft:entity.hostile.splash"
    }

    pub fn monster_get_hurt_sound(&self) -> &'static str {
        "minecraft:entity.hostile.hurt"
    }

    pub fn monster_get_death_sound(&self) -> &'static str {
        "minecraft:entity.hostile.death"
    }

    pub fn monster_get_fall_sounds(&self) -> (&'static str, &'static str) {
        ("minecraft:entity.hostile.small_fall", "minecraft:entity.hostile.big_fall")
    }

    pub fn monster_get_walk_target_value(&self, pos: BlockPos) -> f32 {
        -self.get_pathfinding_cost_from_light_levels(pos)
    }

    pub fn monster_should_drop_experience(&self) -> bool {
        true
    }

    pub fn monster_should_drop_loot(&self) -> bool {
        self.game_rules.get_bool(game_rule::MOB_DROPS)
    }

    pub fn is_preventing_player_rest(&self, _id: EntityId, _player: EntityId) -> bool {
        true
    }

    pub fn monster_get_projectile(&self, id: EntityId, held_weapon: &ItemStack) -> ItemStack {
        let item = held_weapon.get_item();
        if !projectile_weapon_item::is_projectile_weapon(item) {
            return ItemStack::empty();
        }
        let supported_projectiles = projectile_weapon_item::get_supported_held_projectiles(item);
        let held_projectile = [EquipmentSlot::Offhand, EquipmentSlot::Mainhand].into_iter().find_map(|slot| self.item_by_slot(id, slot).filter(|stack| supported_projectiles(stack)));
        held_projectile.unwrap_or_else(|| ItemStack::new(wafer_data::items::ARROW, 1))
    }
}

pub fn get_walk_target_value(pos: BlockPos, level: &mut (impl ServerLevelAccessor + ?Sized)) -> f32 {
    -level.get_pathfinding_cost_from_light_levels(pos)
}

pub fn is_dark_enough_to_spawn(level: &mut (impl ServerLevelAccessor + ?Sized), pos: BlockPos) -> bool {
    if level.get_brightness(LightLayer::Sky, pos) > level.get_random().next_int_bounded(32) {
        return false;
    }
    let dimension_type = level.dimension_type();
    let block_light_limit = dimension_type::monster_spawn_block_light_limit(dimension_type);
    if block_light_limit < 15 && level.get_brightness(LightLayer::Block, pos) > block_light_limit {
        return false;
    }
    let brightness = if level.is_thundering() { level.get_max_local_raw_brightness_with(pos, 10) } else { level.get_max_local_raw_brightness(pos) };
    brightness <= dimension_type::monster_spawn_light_test(dimension_type).sample(level.get_random())
}

pub fn check_monster_spawn_rules(entity_type: &EntityType, level: &mut (impl ServerLevelAccessor + ?Sized), spawn_reason: EntitySpawnReason, pos: BlockPos) -> bool {
    (EntitySpawnReason::ignores_light_requirements(spawn_reason) || is_dark_enough_to_spawn(level, pos)) && mob::check_mob_spawn_rules(entity_type, level, spawn_reason, pos)
}

pub fn check_any_light_monster_spawn_rules(entity_type: &EntityType, level: &mut (impl ServerLevelAccessor + ?Sized), spawn_reason: EntitySpawnReason, pos: BlockPos) -> bool {
    mob::check_mob_spawn_rules(entity_type, level, spawn_reason, pos)
}

pub fn check_surface_monsters_spawn_rules(entity_type: &EntityType, level: &mut (impl ServerLevelAccessor + ?Sized), spawn_reason: EntitySpawnReason, pos: BlockPos) -> bool {
    check_monster_spawn_rules(entity_type, level, spawn_reason, pos) && (EntitySpawnReason::is_spawner(spawn_reason) || level.can_see_sky(pos))
}
