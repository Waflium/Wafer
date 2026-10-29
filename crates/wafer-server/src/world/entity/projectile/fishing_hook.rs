use wafer_core::random::{LegacyRandomSource, RandomSource};
use wafer_core::{BlockPos, Vec3, mth};
use wafer_data::{blocks, fluid_tags, item_tags, items};

use super::projectile::{Projectile, ProjectileKind};
use crate::advancements::triggers::criteria_triggers;
use crate::server::level::ServerLevel;
use crate::world::entity::data::{DataValue, entity_defaults};
use crate::world::entity::entity_reference::EntityReference;
use crate::world::entity::{EntityDimensions, EntityId, ItemEntity, LevelEntity, MoverType, RemovalReason, sound_event};
use crate::world::item::ItemStack;
use crate::world::level::BlockGetter;
use crate::world::level::block::BlockHitResult;
use crate::world::level::material;
use crate::world::level::storage::loot::loot_params::LootParams;
use crate::world::level::storage::loot::loot_table::LootTable;
use crate::world::phys::entity_hit_result::EntityHitResult;
use crate::world::InteractionHand;

const ENTITY_TYPE: &str = "minecraft:fishing_bobber";
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.25, 0.25);
const MAX_OUT_OF_WATER_TIME: i32 = 10;
const DATA_HOOKED_ENTITY: usize = 8;
const DATA_BITING: usize = 9;
pub const GRAVITY: f32 = 0.03;
const FISHING: &str = "minecraft:gameplay/fishing";

#[derive(Clone, Copy, PartialEq, Eq)]
enum FishHookState {
    Flying,
    HookedInEntity,
    Bobbing,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum OpenWaterType {
    AboveWater,
    InsideWater,
    Invalid,
}

pub struct FishingHook {
    syncronized_random: LegacyRandomSource,
    biting: bool,
    out_of_water_time: i32,
    life: i32,
    nibble: i32,
    time_until_lured: i32,
    time_until_hooked: i32,
    fish_angle: f32,
    open_water: bool,
    hooked_in: Option<EntityId>,
    current_state: FishHookState,
    luck: i32,
    lure_speed: i32,
}

fn hook(projectile: &Projectile) -> &FishingHook {
    match &projectile.kind {
        ProjectileKind::FishingHook(hook) => hook,
        _ => unreachable!("not a fishing hook"),
    }
}

fn hook_mut(projectile: &mut Projectile) -> &mut FishingHook {
    match &mut projectile.kind {
        ProjectileKind::FishingHook(hook) => hook,
        _ => unreachable!("not a fishing hook"),
    }
}

impl FishingHook {
    fn with(luck: i32, lure_speed: i32) -> Projectile {
        let mut data = entity_defaults();
        data.push(DataValue::Int(0));
        data.push(DataValue::Boolean(false));
        let hook = FishingHook {
            syncronized_random: LegacyRandomSource::create(),
            biting: false,
            out_of_water_time: 0,
            life: 0,
            nibble: 0,
            time_until_lured: 0,
            time_until_hooked: 0,
            fish_angle: 0.0,
            open_water: true,
            hooked_in: None,
            current_state: FishHookState::Flying,
            luck: luck.max(0),
            lure_speed: lure_speed.max(0),
        };
        Projectile::new(ENTITY_TYPE, DIMENSIONS, data, ProjectileKind::FishingHook(hook))
    }

    pub fn of_type() -> Projectile {
        FishingHook::with(0, 0)
    }

    pub fn from_player(level: &ServerLevel, player: EntityId, luck: i32, lure_speed: i32) -> Projectile {
        let mut projectile = FishingHook::with(luck, lure_speed);
        let p = level.entity(player);
        projectile.owner = Some(EntityReference::of(p.uuid));
        let (x_rot1, y_rot1) = (p.x_rot, p.y_rot);
        let y_cos = mth::cos((-y_rot1 * mth::DEG_TO_RAD - mth::PI) as f64);
        let y_sin = mth::sin((-y_rot1 * mth::DEG_TO_RAD - mth::PI) as f64);
        let x_cos = -mth::cos((-x_rot1 * mth::DEG_TO_RAD) as f64);
        let x_sin = mth::sin((-x_rot1 * mth::DEG_TO_RAD) as f64);
        let x1 = p.x() - y_sin as f64 * 0.3;
        let y1 = p.eye_y();
        let z1 = p.z() - y_cos as f64 * 0.3;
        let e = &mut projectile.entity;
        e.snap_to(x1, y1, z1, y_rot1, x_rot1);
        let mut new_movement = Vec3::new(-y_sin as f64, (-(x_sin / x_cos)).clamp(-5.0, 5.0) as f64, -y_cos as f64);
        let dist = new_movement.length();
        let random = &mut e.random;
        let mx = 0.6 / dist + random.triangle(0.5, 0.0103365);
        let my = 0.6 / dist + random.triangle(0.5, 0.0103365);
        let mz = 0.6 / dist + random.triangle(0.5, 0.0103365);
        new_movement = new_movement.multiply(mx, my, mz);
        e.set_delta_movement(new_movement);
        e.set_y_rot((mth::atan2(new_movement.x, new_movement.z) * mth::RAD_TO_DEG as f64) as f32);
        e.set_x_rot((mth::atan2(new_movement.y, new_movement.horizontal_distance()) * mth::RAD_TO_DEG as f64) as f32);
        e.y_rot_o = e.y_rot;
        e.x_rot_o = e.x_rot;
        projectile
    }

    pub fn is_open_water_fishing(&self) -> bool {
        self.open_water
    }
}

impl ServerLevel {
    pub fn is_fishing_hook(&self, id: EntityId) -> bool {
        self.as_projectile(id).is_some_and(|projectile| matches!(projectile.kind, ProjectileKind::FishingHook(_)))
    }

    pub fn fishing_hook_open_water(&self, id: EntityId) -> Option<bool> {
        match &self.as_projectile(id)?.kind {
            ProjectileKind::FishingHook(hook) => Some(hook.is_open_water_fishing()),
            _ => None,
        }
    }

    fn fishing_hook_on_synced_data_updated(&mut self, id: EntityId, accessor: usize) {
        if accessor == DATA_HOOKED_ENTITY {
            let DataValue::Int(hooked) = *self.entity(id).data.get(DATA_HOOKED_ENTITY) else { unreachable!() };
            let hooked_in = if hooked > 0 { Some(hooked - 1).filter(|&entity| self.entity_ref(entity).is_some()) } else { None };
            hook_mut(self.projectile_mut(id)).hooked_in = hooked_in;
        }
        if accessor == DATA_BITING {
            let biting = matches!(self.entity(id).data.get(DATA_BITING), DataValue::Boolean(true));
            let projectile = self.projectile_mut(id);
            hook_mut(projectile).biting = biting;
            if biting {
                let y = -0.4 * mth::next_float(&mut hook_mut(projectile).syncronized_random, 0.6, 1.0);
                let movement = projectile.entity.delta_movement;
                projectile.entity.set_delta_movement(Vec3::new(movement.x, y as f64, movement.z));
            }
        }
    }

    fn fishing_hook_set_data(&mut self, id: EntityId, accessor: usize, value: DataValue) {
        if *self.entity(id).data.get(accessor) != value {
            self.entity_mut(id).data.set(accessor, value);
            self.fishing_hook_on_synced_data_updated(id, accessor);
        }
    }

    pub fn fishing_hook_tick(&mut self, id: EntityId) {
        let seed = (self.entity(id).uuid as u64 as i64) ^ self.game_time;
        hook_mut(self.projectile_mut(id)).syncronized_random.set_seed(seed);
        self.projectile_base_tick(id);
        let Some(owner) = self.fishing_hook_get_player_owner(id) else {
            self.discard(id);
            return;
        };
        if self.fishing_hook_should_stop_fishing(id, owner) {
            return;
        }
        if self.entity(id).on_ground {
            let hook = hook_mut(self.projectile_mut(id));
            hook.life += 1;
            if hook.life >= 1200 {
                self.discard(id);
                return;
            }
        } else {
            hook_mut(self.projectile_mut(id)).life = 0;
        }
        let mut liquid_height = 0.0;
        let block_pos = self.entity(id).block_position();
        let fluid_state = self.fluid_state(block_pos);
        if fluid_state.is(&fluid_tags::WATER) {
            liquid_height = material::height(self, fluid_state, block_pos);
        }
        let is_in_water = liquid_height > 0.0;
        let state = hook(self.projectile(id)).current_state;
        if state == FishHookState::Flying {
            if hook(self.projectile(id)).hooked_in.is_some() {
                self.entity_mut(id).set_delta_movement(Vec3::ZERO);
                hook_mut(self.projectile_mut(id)).current_state = FishHookState::HookedInEntity;
                return;
            }
            if is_in_water {
                let movement = self.entity(id).delta_movement;
                self.entity_mut(id).set_delta_movement(movement.multiply(0.3, 0.2, 0.3));
                hook_mut(self.projectile_mut(id)).current_state = FishHookState::Bobbing;
                return;
            }
            self.fishing_hook_check_collision(id);
        } else {
            if state == FishHookState::HookedInEntity {
                if let Some(hooked_in) = hook(self.projectile(id)).hooked_in {
                    if self.entity_ref(hooked_in).is_some_and(|e| !e.is_removed()) && self.can_interact_with_level(hooked_in) && self.is_in_level(hooked_in) {
                        let h = self.entity(hooked_in);
                        let pos = Vec3::new(h.x(), h.get_y(0.8), h.z());
                        self.entity_mut(id).set_pos(pos);
                        self.entity_moved(id);
                    } else {
                        self.fishing_hook_set_hooked_entity(id, None);
                        hook_mut(self.projectile_mut(id)).current_state = FishHookState::Flying;
                    }
                }
                return;
            }
            if state == FishHookState::Bobbing {
                let e = self.entity(id);
                let movement = e.delta_movement;
                let mut force = e.y() + movement.y - block_pos.y as f64 - liquid_height as f64;
                if force.abs() < 0.01 {
                    force += force.signum() * 0.1;
                }
                let r = self.entity_mut(id).random.next_float();
                self.entity_mut(id).set_delta_movement(Vec3::new(movement.x * 0.9, movement.y - force * r as f64 * 0.2, movement.z * 0.9));
                let hook = hook(self.projectile(id));
                let open_water = if hook.nibble <= 0 && hook.time_until_hooked <= 0 {
                    true
                } else {
                    hook.open_water && hook.out_of_water_time < MAX_OUT_OF_WATER_TIME && self.fishing_hook_calculate_open_water(block_pos)
                };
                hook_mut(self.projectile_mut(id)).open_water = open_water;
                if is_in_water {
                    let projectile = self.projectile_mut(id);
                    let hook = hook_mut(projectile);
                    hook.out_of_water_time = 0.max(hook.out_of_water_time - 1);
                    if hook.biting {
                        let a = hook.syncronized_random.next_float();
                        let b = hook.syncronized_random.next_float();
                        let movement = projectile.entity.delta_movement;
                        projectile.entity.set_delta_movement(movement.add(0.0, -0.1 * a as f64 * b as f64, 0.0));
                    }
                    self.fishing_hook_catching_fish(id, block_pos);
                } else {
                    let hook = hook_mut(self.projectile_mut(id));
                    hook.out_of_water_time = MAX_OUT_OF_WATER_TIME.min(hook.out_of_water_time + 1);
                }
            }
        }
        if !fluid_state.is(&fluid_tags::WATER) && !self.entity(id).on_ground && hook(self.projectile(id)).hooked_in.is_none() {
            let movement = self.entity(id).delta_movement;
            self.entity_mut(id).set_delta_movement(movement.add(0.0, -(GRAVITY as f64), 0.0));
        }
        let movement = self.entity(id).delta_movement;
        self.move_entity(id, MoverType::SelfMove, movement);
        self.apply_effects_from_blocks(id);
        self.update_rotation(id);
        let e = self.entity(id);
        if hook(self.projectile(id)).current_state == FishHookState::Flying && (e.on_ground || e.horizontal_collision) {
            self.entity_mut(id).set_delta_movement(Vec3::ZERO);
        }
        let e = self.entity_mut(id);
        e.set_delta_movement(e.delta_movement.scale(0.92));
        e.reapply_position();
    }

    fn fishing_hook_should_stop_fishing(&mut self, id: EntityId, owner: EntityId) -> bool {
        if self.can_interact_with_level(owner) {
            let player = self.player(owner);
            let main_hand_is_fishing = player.item_in_hand(InteractionHand::MainHand).is_some_and(|stack| stack.is(items::FISHING_ROD));
            let off_hand_is_fishing = player.item_in_hand(InteractionHand::OffHand).is_some_and(|stack| stack.is(items::FISHING_ROD));
            if (main_hand_is_fishing || off_hand_is_fishing) && self.entity(id).distance_to_sqr(self.entity(owner)) <= 1024.0 {
                return false;
            }
        }
        self.discard(id);
        true
    }

    fn fishing_hook_check_collision(&mut self, id: EntityId) {
        let hit_result = self.get_hit_result_on_move_vector(id, |level, entity| level.can_hit_entity(id, entity));
        self.hit_target_or_deflect_self(id, hit_result);
    }

    pub fn fishing_hook_can_hit_entity(&self, id: EntityId, entity: EntityId) -> bool {
        self.projectile_base_can_hit_entity(id, entity) || self.is_alive(entity) && matches!(self.find_entity(entity), Some(LevelEntity::Item(_)))
    }

    pub fn fishing_hook_on_hit_entity(&mut self, id: EntityId, hit_result: EntityHitResult) {
        self.fishing_hook_set_hooked_entity(id, Some(hit_result.entity));
    }

    pub fn fishing_hook_on_hit_block(&mut self, id: EntityId, hit_result: BlockHitResult) {
        self.projectile_base_on_hit_block(id, hit_result);
        let e = self.entity(id);
        let location = hit_result.location;
        let (xd, yd, zd) = (location.x - e.x(), location.y - e.y(), location.z - e.z());
        let distance = xd * xd + yd * yd + zd * zd;
        let movement = e.delta_movement.normalize().scale(distance);
        self.entity_mut(id).set_delta_movement(movement);
    }

    fn fishing_hook_set_hooked_entity(&mut self, id: EntityId, hooked_in: Option<EntityId>) {
        hook_mut(self.projectile_mut(id)).hooked_in = hooked_in;
        self.fishing_hook_set_data(id, DATA_HOOKED_ENTITY, DataValue::Int(hooked_in.map_or(0, |entity| entity + 1)));
    }

    fn fishing_hook_catching_fish(&mut self, id: EntityId, block_pos: BlockPos) {
        let mut fishing_speed = 1;
        let above = block_pos.above();
        if self.entity_mut(id).random.next_float() < 0.25 && self.is_raining_at(above) {
            fishing_speed += 1;
        }
        if self.entity_mut(id).random.next_float() < 0.5 && !self.can_see_sky(above) {
            fishing_speed -= 1;
        }
        let (nibble, time_until_hooked, time_until_lured) = {
            let hook = hook(self.projectile(id));
            (hook.nibble, hook.time_until_hooked, hook.time_until_lured)
        };
        if nibble > 0 {
            let hook = hook_mut(self.projectile_mut(id));
            hook.nibble -= 1;
            if hook.nibble <= 0 {
                hook.time_until_lured = 0;
                hook.time_until_hooked = 0;
                self.fishing_hook_set_data(id, DATA_BITING, DataValue::Boolean(false));
            }
        } else if time_until_hooked > 0 {
            hook_mut(self.projectile_mut(id)).time_until_hooked -= fishing_speed;
            let time_until_hooked = hook(self.projectile(id)).time_until_hooked;
            if time_until_hooked > 0 {
                let delta = self.entity_mut(id).random.triangle(0.0, 9.188) as f32;
                let hook = hook_mut(self.projectile_mut(id));
                hook.fish_angle += delta;
                let angle = hook.fish_angle * mth::DEG_TO_RAD;
                let angle_sin = mth::sin(angle as f64);
                let angle_cos = mth::cos(angle as f64);
                let e = self.entity(id);
                let fish_x = e.x() + (angle_sin * time_until_hooked as f32 * 0.1) as f64;
                let fish_y = (mth::floor(e.y()) as f32 + 1.0) as f64;
                let fish_z = e.z() + (angle_cos * time_until_hooked as f32 * 0.1) as f64;
                let splash_block_state = self.block_state(BlockPos::containing(fish_x, fish_y - 1.0, fish_z));
                if splash_block_state.is_block(blocks::WATER) {
                    if self.entity_mut(id).random.next_float() < 0.15 {
                        self.send_particles("minecraft:bubble", |_| {}, fish_x, fish_y - 0.1f32 as f64, fish_z, 1, (angle_sin as f64, 0.1, angle_cos as f64), 0.0);
                    }
                    let particle_x_movement = angle_sin * 0.04;
                    let particle_z_movement = angle_cos * 0.04;
                    self.send_particles("minecraft:fishing", |_| {}, fish_x, fish_y, fish_z, 0, (particle_z_movement as f64, 0.01, -particle_x_movement as f64), 1.0);
                    self.send_particles("minecraft:fishing", |_| {}, fish_x, fish_y, fish_z, 0, (-particle_z_movement as f64, 0.01, particle_x_movement as f64), 1.0);
                }
            } else {
                let random = &mut self.entity_mut(id).random;
                let pitch = 1.0 + (random.next_float() - random.next_float()) * 0.4;
                self.entity_play_sound(id, sound_event("minecraft:entity.fishing_bobber.splash"), 0.25, pitch);
                let e = self.entity(id);
                let (x, y, z) = (e.x(), e.y() + 0.5, e.z());
                let width = e.dimensions.width;
                let count = (1.0 + width * 20.0) as i32;
                self.send_particles("minecraft:bubble", |_| {}, x, y, z, count, (width as f64, 0.0, width as f64), 0.2f32 as f64);
                self.send_particles("minecraft:fishing", |_| {}, x, y, z, count, (width as f64, 0.0, width as f64), 0.2f32 as f64);
                let nibble = mth::next_int(&mut self.entity_mut(id).random, 20, 40);
                hook_mut(self.projectile_mut(id)).nibble = nibble;
                self.fishing_hook_set_data(id, DATA_BITING, DataValue::Boolean(true));
            }
        } else if time_until_lured > 0 {
            let hook = hook_mut(self.projectile_mut(id));
            hook.time_until_lured -= fishing_speed;
            let time_until_lured = hook.time_until_lured;
            let mut tease_chance = 0.15f32;
            if time_until_lured < 20 {
                tease_chance += (20 - time_until_lured) as f32 * 0.05;
            } else if time_until_lured < 40 {
                tease_chance += (40 - time_until_lured) as f32 * 0.02;
            } else if time_until_lured < 60 {
                tease_chance += (60 - time_until_lured) as f32 * 0.01;
            }
            if self.entity_mut(id).random.next_float() < tease_chance {
                let random = &mut self.entity_mut(id).random;
                let angle = mth::next_float(random, 0.0, 360.0) * mth::DEG_TO_RAD;
                let dist = mth::next_float(random, 25.0, 60.0);
                let e = self.entity(id);
                let fish_x = e.x() + (mth::sin(angle as f64) * dist) as f64 * 0.1;
                let fish_y = (mth::floor(e.y()) as f32 + 1.0) as f64;
                let fish_z = e.z() + (mth::cos(angle as f64) * dist) as f64 * 0.1;
                let splash_block_state = self.block_state(BlockPos::containing(fish_x, fish_y - 1.0, fish_z));
                if splash_block_state.is_block(blocks::WATER) {
                    let count = 2 + self.entity_mut(id).random.next_int_bounded(2);
                    self.send_particles("minecraft:splash", |_| {}, fish_x, fish_y, fish_z, count, (0.1f32 as f64, 0.0, 0.1f32 as f64), 0.0);
                }
            }
            if time_until_lured <= 0 {
                let random = &mut self.entity_mut(id).random;
                let fish_angle = mth::next_float(random, 0.0, 360.0);
                let time_until_hooked = mth::next_int(random, 20, 80);
                let hook = hook_mut(self.projectile_mut(id));
                hook.fish_angle = fish_angle;
                hook.time_until_hooked = time_until_hooked;
            }
        } else {
            let time_until_lured = mth::next_int(&mut self.entity_mut(id).random, 100, 600);
            let hook = hook_mut(self.projectile_mut(id));
            hook.time_until_lured = time_until_lured - hook.lure_speed;
        }
    }

    fn fishing_hook_calculate_open_water(&mut self, block_pos: BlockPos) -> bool {
        let mut previous_layer = OpenWaterType::Invalid;
        for y in -1..=2 {
            let layer = self.fishing_hook_get_open_water_type_for_area(block_pos.offset(-2, y, -2), block_pos.offset(2, y, 2));
            match layer {
                OpenWaterType::AboveWater => {
                    if previous_layer == OpenWaterType::Invalid {
                        return false;
                    }
                }
                OpenWaterType::InsideWater => {
                    if previous_layer == OpenWaterType::AboveWater {
                        return false;
                    }
                }
                OpenWaterType::Invalid => return false,
            }
            previous_layer = layer;
        }
        true
    }

    fn fishing_hook_get_open_water_type_for_area(&mut self, from: BlockPos, to: BlockPos) -> OpenWaterType {
        BlockPos::between_closed(from, to).map(|pos| self.fishing_hook_get_open_water_type_for_block(pos)).reduce(|a, b| if a == b { a } else { OpenWaterType::Invalid }).unwrap_or(OpenWaterType::Invalid)
    }

    fn fishing_hook_get_open_water_type_for_block(&mut self, pos: BlockPos) -> OpenWaterType {
        let state = self.block_state(pos);
        if !state.is_air() && !state.is_block(blocks::LILY_PAD) {
            let fluid_state = state.fluid_state();
            if fluid_state.is(&fluid_tags::WATER) && fluid_state.is_source() && state.collision_shape().is_empty() { OpenWaterType::InsideWater } else { OpenWaterType::Invalid }
        } else {
            OpenWaterType::AboveWater
        }
    }

    pub fn fishing_hook_retrieve(&mut self, id: EntityId, rod: &ItemStack) -> i32 {
        let Some(owner) = self.fishing_hook_get_player_owner(id) else { return 0 };
        if self.fishing_hook_should_stop_fishing(id, owner) {
            return 0;
        }
        let mut dmg = 0;
        let hook = hook(self.projectile(id));
        if let Some(hooked_in) = hook.hooked_in {
            self.fishing_hook_pull_entity(id, hooked_in);
            criteria_triggers::FISHING_ROD_HOOKED.trigger(self, owner, rod, id, &[]);
            self.broadcast_entity_event(id, 31);
            dmg = if matches!(self.find_entity(hooked_in), Some(LevelEntity::Item(_))) { 3 } else { 5 };
        } else if hook.nibble > 0 {
            let luck = hook.luck as f32 + self.attribute_value(owner, crate::world::entity::attributes::Attribute::Luck) as f32;
            let position = self.entity(id).position();
            let loot_table = LootTable::get(FISHING);
            let mut params = LootParams::new(self);
            params.origin = Some(position);
            params.tool = Some(rod);
            params.this_entity = Some(id);
            params.luck = luck;
            let items = loot_table.get_random_items(params);
            criteria_triggers::FISHING_ROD_HOOKED.trigger(self, owner, rod, id, &items);
            for item_stack in items {
                let e = self.entity(id);
                let (x, y, z) = (e.x(), e.y(), e.z());
                let mut entity = ItemEntity::new(Vec3::new(x, y, z), item_stack.clone());
                let o = self.entity(owner);
                let xa = o.x() - x;
                let ya = o.y() - y;
                let za = o.z() - z;
                entity.entity.set_delta_movement(Vec3::new(xa * 0.1, ya * 0.1 + (xa * xa + ya * ya + za * za).sqrt().sqrt() * 0.08, za * 0.1));
                self.add_fresh_entity(LevelEntity::Item(entity));
                let o = self.entity(owner);
                let orb_pos = Vec3::new(o.x(), o.y() + 0.5, o.z() + 0.5);
                let value = self.entity_mut(id).random.next_int_bounded(6) + 1;
                let orb = self.new_experience_orb(orb_pos, Vec3::ZERO, value);
                self.add_fresh_entity(LevelEntity::ExperienceOrb(orb));
                if item_stack.get_item().is(&item_tags::FISHES) {
                    self.award_stat(owner, crate::stats::stats::FISH_CAUGHT, 1);
                }
            }
            dmg = 1;
        }
        if self.entity(id).on_ground {
            dmg = 2;
        }
        self.discard(id);
        dmg
    }

    fn fishing_hook_pull_entity(&mut self, id: EntityId, entity: EntityId) {
        let Some(owner) = self.projectile_get_owner(id) else { return };
        if !self.can_simulate_movement(entity) {
            return;
        }
        let (o, e) = (self.entity(owner), self.entity(id));
        let delta = Vec3::new(o.x() - e.x(), o.y() - e.y(), o.z() - e.z()).scale(0.1);
        let target = self.entity_mut(entity);
        target.set_delta_movement(target.delta_movement.add_vec(delta));
    }

    pub fn fishing_hook_remove(&mut self, id: EntityId, reason: RemovalReason) {
        self.fishing_hook_update_owner_info(id, None);
        self.set_removed(id, reason);
    }

    pub fn fishing_hook_update_owner_info(&mut self, id: EntityId, hook: Option<EntityId>) {
        if let Some(owner) = self.fishing_hook_get_player_owner(id) {
            self.player_mut(owner).fishing = hook;
        }
    }

    pub fn fishing_hook_get_player_owner(&self, id: EntityId) -> Option<EntityId> {
        self.projectile_get_owner(id).filter(|&owner| self.is_player(owner))
    }

    pub fn fishing_hook_add_data(&self, id: EntityId) -> i32 {
        self.projectile_get_owner(id).unwrap_or(id)
    }
}
