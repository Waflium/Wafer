use wafer_core::AABB;
use wafer_data::{BlockState, Tag, block_tags, blocks};

use super::entity_types;
use super::mob_category::MobCategory;
use crate::world::difficulty::Difficulty;
use crate::world::level::pathfinder::node_evaluator;

pub fn fire_immune(entity_type: i32) -> bool {
    EntityType::by_id(entity_type).fire_immune()
}

pub struct EntityType {
    pub id: i32,
    category: MobCategory,
    immune_to: fn() -> &'static Tag,
    summon: bool,
    fire_immune: bool,
    can_spawn_far_from_player: bool,
    width: f32,
    height: f32,
    spawn_dimensions_scale: f32,
    allowed_in_peaceful: bool,
}

impl EntityType {
    pub fn by_id(id: i32) -> &'static EntityType {
        entity_types::BY_ID[id as usize]
    }

    pub fn by_name(name: &str) -> Option<&'static EntityType> {
        wafer_data::registry::ENTITY_TYPE.iter().position(|&e| e == name).map(|i| entity_types::BY_ID[i])
    }

    pub fn get_key(&self) -> &'static str {
        wafer_data::registry::ENTITY_TYPE[self.id as usize]
    }

    pub fn get_category(&self) -> MobCategory {
        self.category
    }

    pub fn can_summon(&self) -> bool {
        self.summon
    }

    pub fn fire_immune(&self) -> bool {
        self.fire_immune
    }

    pub fn can_spawn_far_from_player(&self) -> bool {
        self.can_spawn_far_from_player
    }

    pub fn get_width(&self) -> f32 {
        self.width
    }

    pub fn get_height(&self) -> f32 {
        self.height
    }

    pub fn can_spawn(&self, difficulty: Difficulty) -> bool {
        self.is_allowed_in_peaceful() || difficulty != Difficulty::Peaceful
    }

    pub fn is_allowed_in_peaceful(&self) -> bool {
        self.allowed_in_peaceful
    }

    pub fn get_spawn_aabb(&self, x: f64, y: f64, z: f64) -> AABB {
        let half_width = self.spawn_dimensions_scale * self.width / 2.0;
        let height = self.spawn_dimensions_scale * self.height;
        AABB::new(x - half_width as f64, y, z - half_width as f64, x + half_width as f64, y + height as f64, z + half_width as f64)
    }

    pub fn is_block_dangerous(&self, state: BlockState) -> bool {
        if state.is((self.immune_to)()) {
            return false;
        }
        if !self.fire_immune && node_evaluator::is_burning_block(state) {
            return true;
        }
        state.is_block(blocks::WITHER_ROSE) || state.is_block(blocks::SWEET_BERRY_BUSH) || state.is_block(blocks::CACTUS) || state.is_block(blocks::POWDER_SNOW)
    }
}

impl PartialEq for EntityType {
    fn eq(&self, other: &EntityType) -> bool {
        self.id == other.id
    }
}

pub struct Builder(EntityType);

impl Builder {
    pub const fn of(id: i32, category: MobCategory) -> Builder {
        Builder(EntityType {
            id,
            category,
            immune_to: || &block_tags::DEFAULT_IMMUNE_TO,
            summon: true,
            fire_immune: false,
            can_spawn_far_from_player: matches!(category, MobCategory::Creature | MobCategory::Misc),
            width: 0.6,
            height: 1.8,
            spawn_dimensions_scale: 1.0,
            allowed_in_peaceful: true,
        })
    }

    pub const fn sized(self, width: f32, height: f32) -> Builder {
        Builder(EntityType { width, height, ..self.0 })
    }

    pub const fn spawn_dimensions_scale(self, spawn_dimensions_scale: f32) -> Builder {
        Builder(EntityType { spawn_dimensions_scale, ..self.0 })
    }

    pub const fn no_summon(self) -> Builder {
        Builder(EntityType { summon: false, ..self.0 })
    }

    pub const fn fire_immune(self) -> Builder {
        Builder(EntityType { fire_immune: true, ..self.0 })
    }

    pub const fn immune_to(self, immune_to: fn() -> &'static Tag) -> Builder {
        Builder(EntityType { immune_to, ..self.0 })
    }

    pub const fn can_spawn_far_from_player(self) -> Builder {
        Builder(EntityType { can_spawn_far_from_player: true, ..self.0 })
    }

    pub const fn not_in_peaceful(self) -> Builder {
        Builder(EntityType { allowed_in_peaceful: false, ..self.0 })
    }

    pub const fn build(self) -> EntityType {
        self.0
    }
}
