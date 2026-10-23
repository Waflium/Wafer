use wafer_core::{AABB, Axis, BlockPos, Direction, Vec3};
use wafer_data::{game_rule, items, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use super::painting_variant;
use crate::server::level::ServerLevel;
use crate::util::codec;
use crate::world::entity::data::{DataValue, entity_defaults};
use crate::world::entity::decoration::block_attached_entity::{self, BlockAttachedEntity, BlockAttachedKind};
use crate::world::entity::decoration::hanging_entity::{self, HangingView};
use crate::world::entity::variant::variant_utils;
use crate::world::entity::{Entity, EntityDimensions, EntityId, LevelEntity, sound_event};
use crate::world::item::ItemStack;
use crate::world::item::component::{self as dc};
use crate::world::level::storage::tag_value_input;

const DATA_PAINTING_VARIANT_ID: usize = 9;
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.5, 0.5);

pub struct Painting {
    pub entity: Entity,
}

pub fn get_variant(entity: &Entity) -> i32 {
    match entity.data.get(DATA_PAINTING_VARIANT_ID) {
        DataValue::PaintingVariant(id) => *id,
        _ => 0,
    }
}

pub fn calculate_bounding_box(entity: &Entity, pos: BlockPos, direction: Direction) -> AABB {
    let attached_to_wall = Vec3::at_center_of(pos).relative(direction, -0.46875);
    let variant = painting_variant::get(get_variant(entity));
    let horizontal_offset = offset_for_painting_size(variant.width);
    let vertical_offset = offset_for_painting_size(variant.height);
    let left = direction.counter_clockwise();
    let position = attached_to_wall.relative(left, horizontal_offset).relative(Direction::Up, vertical_offset);
    let axis = direction.axis();
    let x_size = if axis == Axis::X { 0.0625 } else { variant.width as f64 };
    let y_size = variant.height as f64;
    let z_size = if axis == Axis::Z { 0.0625 } else { variant.width as f64 };
    AABB::of_size(position, x_size, y_size, z_size)
}

fn offset_for_painting_size(size: i32) -> f64 {
    if size % 2 == 0 { 0.5 } else { 0.0 }
}

impl Painting {
    pub fn create_empty() -> Painting {
        let mut data = entity_defaults();
        data.push(DataValue::Direction(hanging_entity::DEFAULT_DIRECTION));
        data.push(DataValue::PaintingVariant(0));
        let mut entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:painting"), DIMENSIONS, data);
        entity.block_attached = Some(BlockAttachedEntity::new(BlockAttachedKind::Painting, BlockPos::ZERO));
        Painting { entity }
    }

    fn at(pos: BlockPos) -> Painting {
        let mut painting = Painting::create_empty();
        painting.entity.block_attached = Some(BlockAttachedEntity::new(BlockAttachedKind::Painting, pos));
        painting
    }

    pub fn create(level: &mut ServerLevel, pos: BlockPos, direction: Direction) -> Option<Painting> {
        let mut candidate = Painting::at(pos);
        let mut potential_variants: Vec<i32> = wafer_data::tag(painting_variant::REGISTRY, painting_variant::PLACEABLE).to_vec();
        if potential_variants.is_empty() {
            return None;
        }
        hanging_entity::set_direction(&mut candidate.entity, direction);
        potential_variants.retain(|&variant| {
            candidate.set_variant(variant);
            let view = HangingView::of(&candidate.entity, candidate.entity.bounding_box());
            level.hanging_view_survives(&view)
        });
        if potential_variants.is_empty() {
            return None;
        }
        let largest_painting_area_size = potential_variants.iter().map(|&variant| painting_variant::get(variant).area()).max().unwrap_or(0);
        potential_variants.retain(|&variant| painting_variant::get(variant).area() >= largest_painting_area_size);
        let selected_variant = crate::util::get_random_safe(&potential_variants, &mut candidate.entity.random)?;
        candidate.set_variant(selected_variant);
        hanging_entity::set_direction(&mut candidate.entity, direction);
        Some(candidate)
    }

    pub fn set_variant(&mut self, variant: i32) {
        if get_variant(&self.entity) != variant {
            self.entity.data.set(DATA_PAINTING_VARIANT_ID, DataValue::PaintingVariant(variant));
            block_attached_entity::recalculate_bounding_box(&mut self.entity);
        }
    }

    pub fn apply_components_from_item_stack(&mut self, stack: &ItemStack) {
        if let Some(variant) = stack.get(dc::PAINTING_VARIANT).and_then(Nbt::as_str).and_then(|name| variant_utils::read_variant(painting_variant::REGISTRY, name)) {
            self.set_variant(variant);
        }
        self.entity.apply_components_from_item_stack(stack);
    }

    pub fn add_additional_save_data(&self, output: &mut Vec<(String, Nbt)>) {
        output.push(("facing".into(), Nbt::Byte(hanging_entity::get_direction(&self.entity).get_2d_data_value() as i8)));
        block_attached_entity::add_additional_save_data(&self.entity, output);
        output.push(("variant".into(), Nbt::String(variant_utils::variant_name(painting_variant::REGISTRY, get_variant(&self.entity)).into())));
    }

    pub fn read_additional_save_data(&mut self, input: &Nbt) {
        let direction = tag_value_input::read_or_error(input, "facing", |tag| codec::int(tag)).map_or(Direction::South, |value| Direction::from_2d_data_value(value as i8 as i32));
        block_attached_entity::read_additional_save_data(&mut self.entity, input);
        hanging_entity::set_direction(&mut self.entity, direction);
        if let Some(variant) = variant_utils::read_variant_field(input, "variant", painting_variant::REGISTRY) {
            self.set_variant(variant);
        }
    }

}

impl ServerLevel {
    pub fn painting_drop_item(&mut self, id: EntityId, caused_by: Option<EntityId>) {
        if self.game_rules.get_bool(game_rule::ENTITY_DROPS) {
            self.entity_play_sound(id, sound_event("minecraft:entity.painting.break"), 1.0, 1.0);
            if !caused_by.is_some_and(|player| self.has_infinite_materials(player)) {
                let mut item_stack = ItemStack::new(items::PAINTING, 1);
                item_stack.set(dc::CUSTOM_NAME, self.entity(id).custom_name().map(|name| name.to_nbt()));
                self.hanging_entity_spawn_at_location(id, item_stack, 0.0);
            }
        }
    }

    pub fn painting_play_placement_sound(&mut self, id: EntityId) {
        self.entity_play_sound(id, sound_event("minecraft:entity.painting.place"), 1.0, 1.0);
    }

    pub fn is_painting(&self, id: EntityId) -> bool {
        matches!(self.entities.get(&id), Some(LevelEntity::Painting(_)))
    }
}
