use wafer_core::shapes::{BooleanOp, VoxelShape, join_unoptimized};
use wafer_core::{AABB, Axis, BlockPos, Vec3};
use wafer_data::{Block, block_tags, blocks, damage_types, game_rule, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use super::block_attached_entity::{self, BlockAttachedEntity, BlockAttachedKind};
use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::entity::data::{DataValue, entity_defaults};
use crate::world::entity::{Entity, EntityDimensions, EntityId, LevelEntity, sound_event};
use crate::world::item::component as dc;
use crate::world::item::cushion_item;
use crate::world::item::dye_color::DyeColor;
use crate::world::item::ItemStack;
use crate::world::level::BlockGetter;
use crate::world::phys::shapes::collision_context::CollisionContext;
use crate::world::{InteractionHand, InteractionResult};

const DEFAULT_COLOR: DyeColor = DyeColor::VALUES[0];
const LIGHTNING_DROP_INVULNERABLE_TICKS: i32 = 20;
const DATA_COLOR: usize = 8;
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(1.0, 0.25);

const WOOL: [Block; 16] = [
    blocks::WHITE_WOOL,
    blocks::ORANGE_WOOL,
    blocks::MAGENTA_WOOL,
    blocks::LIGHT_BLUE_WOOL,
    blocks::YELLOW_WOOL,
    blocks::LIME_WOOL,
    blocks::PINK_WOOL,
    blocks::GRAY_WOOL,
    blocks::LIGHT_GRAY_WOOL,
    blocks::CYAN_WOOL,
    blocks::PURPLE_WOOL,
    blocks::BLUE_WOOL,
    blocks::BROWN_WOOL,
    blocks::GREEN_WOOL,
    blocks::RED_WOOL,
    blocks::BLACK_WOOL,
];

pub struct Cushion {
    pub entity: Entity,
}

impl Cushion {
    pub fn of_type() -> Cushion {
        let mut data = entity_defaults();
        data.push(DataValue::DyeColor(DEFAULT_COLOR.id));
        let mut entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:cushion"), DIMENSIONS, data);
        entity.block_attached = Some(BlockAttachedEntity::new(BlockAttachedKind::Cushion, BlockPos::ZERO));
        set_pos(&mut entity, 0.0, 0.0, 0.0);
        Cushion { entity }
    }

    pub fn get_color(&self) -> DyeColor {
        get_color(&self.entity)
    }

    pub fn set_color(&mut self, color: DyeColor) {
        self.entity.data.set(DATA_COLOR, DataValue::DyeColor(color.id));
    }

    pub fn get_pick_result(&self) -> ItemStack {
        ItemStack::new(cushion_item::CUSHION[self.get_color().id as usize], 1)
    }

    pub fn add_additional_save_data(&self, output: &mut Vec<(String, Nbt)>) {
        block_attached_entity::add_additional_save_data(&self.entity, output);
        output.push(("color".into(), Nbt::String(self.get_color().name.into())));
    }

    pub fn read_additional_save_data(&mut self, input: &Nbt) {
        block_attached_entity::read_additional_save_data(&mut self.entity, input);
        self.set_color(input.get("color").and_then(Nbt::as_str).and_then(DyeColor::by_name).unwrap_or(DEFAULT_COLOR));
    }

    pub fn apply_components_from_item_stack(&mut self, stack: &ItemStack) {
        if let Some(color) = stack.get(dc::CUSHION_COLOR).and_then(Nbt::as_str).and_then(DyeColor::by_name) {
            self.set_color(color);
        }
        self.entity.apply_components_from_item_stack(stack);
    }

    fn get_cushion_item_stack_with_data(&self) -> ItemStack {
        let mut item_stack = self.get_pick_result();
        item_stack.set(dc::CUSTOM_NAME, self.entity.custom_name().map(|name| name.to_nbt()));
        item_stack
    }
}

fn get_color(entity: &Entity) -> DyeColor {
    match entity.data.get(DATA_COLOR) {
        DataValue::DyeColor(id) => DyeColor::by_id(*id),
        _ => DEFAULT_COLOR,
    }
}

pub fn set_pos(entity: &mut Entity, x: f64, y: f64, z: f64) {
    entity.set_pos_raw(Vec3::new(x, y, z));
    block_attached_entity::block_attached_entity_set_pos(entity, x, y, z);
}

pub fn recalculate_bounding_box(entity: &mut Entity) {
    let bounding_box = entity.make_bounding_box(entity.position());
    entity.set_bounding_box(bounding_box);
}

pub fn can_be_placed_at(level: &mut ServerLevel, bounding_box: &AABB) -> bool {
    would_survive_at(level, bounding_box) && !is_anchor_buried(level, bounding_box)
}

pub fn would_survive_at(level: &mut ServerLevel, bounding_box: &AABB) -> bool {
    has_anchor_below(level, bounding_box) && !is_covered_by_suffocating_blocks(level, bounding_box)
}

fn has_anchor_below(level: &mut ServerLevel, bounding_box: &AABB) -> bool {
    let anchor_box = AABB::new(bounding_box.min_x, bounding_box.min_y - 0.015625, bounding_box.min_z, bounding_box.max_x.next_down(), bounding_box.min_y, bounding_box.max_z.next_down());
    let search_box = anchor_box.expand_towards(0.0, -0.125, 0.0);
    let from = BlockPos::containing(search_box.min_x, search_box.min_y, search_box.min_z);
    let to = BlockPos::containing(search_box.max_x, search_box.max_y, search_box.max_z);
    level.find_blocks_in(from, to).for_each_until(level, |_, block_pos, block_state| {
        let shape = block_state.get_shape(block_pos);
        !shape.is_empty() && bounds(&shape).move_to_block(block_pos).intersects(&anchor_box)
    })
}

fn bounds(shape: &VoxelShape) -> AABB {
    AABB::new(shape.min(Axis::X), shape.min(Axis::Y), shape.min(Axis::Z), shape.max(Axis::X), shape.max(Axis::Y), shape.max(Axis::Z))
}

fn is_anchor_buried(level: &ServerLevel, bounding_box: &AABB) -> bool {
    let resting_slice = AABB::new(bounding_box.min_x, bounding_box.min_y, bounding_box.min_z, bounding_box.max_x, bounding_box.min_y + 0.015625, bounding_box.max_z).next_deflated();
    let mut exposed_surface = VoxelShape::from_boxes(&[resting_slice]);
    for (_, collider) in level.block_collisions(&resting_slice, &CollisionContext::EMPTY, false) {
        exposed_surface = join_unoptimized(&exposed_surface, &collider.moved(), BooleanOp::OnlyFirst);
        if exposed_surface.is_empty() {
            return true;
        }
    }
    false
}

fn is_covered_by_suffocating_blocks(level: &mut ServerLevel, bounding_box: &AABB) -> bool {
    let deflated = bounding_box.next_deflated();
    let from = BlockPos::containing(deflated.min_x, deflated.min_y, deflated.min_z);
    let to = BlockPos::containing(deflated.max_x, deflated.max_y, deflated.max_z);
    BlockPos::between_closed(from, to).all(|block_pos| {
        let state = level.block_state(block_pos);
        level.is_suffocating_at(block_pos, state)
    })
}

fn cushion(level: &ServerLevel, id: EntityId) -> &Cushion {
    match level.find_entity(id) {
        Some(LevelEntity::Cushion(cushion)) => cushion,
        _ => unreachable!("a cushion"),
    }
}

impl ServerLevel {
    pub fn is_cushion(&self, id: EntityId) -> bool {
        matches!(self.find_entity(id), Some(LevelEntity::Cushion(_)))
    }

    pub fn cushion_drop_item(&mut self, id: EntityId, caused_by: Option<EntityId>) {
        self.entity_play_sound(id, sound_event("minecraft:entity.cushion.break"), 1.0, 1.0);
        self.cushion_show_breaking_particles(id);
        if self.game_rules.get_bool(game_rule::ENTITY_DROPS) && !caused_by.is_some_and(|player| self.has_infinite_materials(player)) {
            let item_stack = cushion(self, id).get_cushion_item_stack_with_data();
            if let Some(item_entity) = self.spawn_at_location(id, item_stack, 0.0)
                && caused_by.is_some_and(|entity| matches!(self.find_entity(entity), Some(LevelEntity::LightningBolt(_))))
            {
                self.entity_mut(item_entity).invulnerable_time = LIGHTNING_DROP_INVULNERABLE_TICKS;
            }
        }
    }

    fn cushion_is_breaking_denied_for(&self, source: &DamageSource) -> bool {
        source.causing_entity.is_some_and(|player| self.is_player(player) && !self.may_build(player))
    }

    fn cushion_is_breaking_denied_at_pos_for(&self, id: EntityId, source: &DamageSource) -> bool {
        source.causing_entity.is_some_and(|player| self.is_player(player) && !self.may_interact(player, self.entity(id).block_attached_pos()))
    }

    pub fn cushion_hurt_server(&mut self, id: EntityId, source: DamageSource) -> bool {
        if !self.cushion_is_breaking_denied_for(&source) && !self.cushion_is_breaking_denied_at_pos_for(id, &source) {
            self.block_attached_entity_hurt_server(id, source)
        } else {
            false
        }
    }

    pub fn cushion_interact(&mut self, id: EntityId, player: EntityId, _hand: InteractionHand, _location: Vec3) -> InteractionResult {
        if self.player(player).is_secondary_use_active() || self.is_vehicle(id) {
            InteractionResult::Pass
        } else if self.start_riding(player, id, false, true) {
            self.entity_play_sound(id, sound_event("minecraft:entity.cushion.sit"), 1.0, 1.0);
            InteractionResult::SUCCESS_SERVER
        } else {
            InteractionResult::CONSUME
        }
    }

    pub fn cushion_remove_passenger(&mut self, id: EntityId) {
        if !self.entity(id).is_removed() {
            self.entity_play_sound(id, sound_event("minecraft:entity.cushion.get_up"), 1.0, 1.0);
        }
    }

    pub fn cushion_tick_at_check_interval(&mut self, id: EntityId) {
        let block_pos = self.entity(id).block_position();
        let fluid_state = self.block_state(block_pos).fluid_state();
        let position = self.entity(id).position();
        if self.collided_with_fluid(id, fluid_state, block_pos, position, position) {
            crate::world::level::material::entity_inside(fluid_state, &mut self.entity_mut(id).inside_effects);
            self.apply_inside_effects(id);
        }
        self.cushion_destroy_if_in_fire(id);
    }

    pub fn cushion_destroy_if_in_fire(&mut self, id: EntityId) {
        if self.entity(id).is_removed() {
            return;
        }
        let deflated = self.entity(id).bounding_box().next_deflated();
        let from = BlockPos::containing(deflated.min_x, deflated.min_y, deflated.min_z);
        let to = BlockPos::containing(deflated.max_x, deflated.max_y, deflated.max_z);
        self.find_blocks_in(from, to).filter_state(|state| state.is(&block_tags::FIRE)).for_each_until(self, |level, _, _| {
            level.hurt_server(id, DamageSource::new(damage_types::IN_FIRE), 1.0);
            true
        });
    }

    pub fn cushion_thunder_hit(&mut self, id: EntityId, lightning_bolt: EntityId) {
        if !self.entity(id).is_removed() {
            self.block_attached_entity_kill(id, Some(lightning_bolt));
            self.cushion_drop_item(id, Some(lightning_bolt));
        }
    }

    pub fn cushion_survives(&mut self, id: EntityId) -> bool {
        let bounding_box = self.entity(id).bounding_box();
        would_survive_at(self, &bounding_box)
    }

    fn cushion_show_breaking_particles(&mut self, id: EntityId) {
        let e = self.entity(id);
        let state = WOOL[get_color(e).id as usize].default_state();
        let (width, height) = (e.dimensions.width, e.dimensions.height);
        let (x, y, z) = (e.x(), e.get_y(0.6666666666666666), e.z());
        self.send_block_particles(state, x, y, z, 10, ((width / 4.0) as f64, (height / 4.0) as f64, (width / 4.0) as f64), 0.05);
    }
}
