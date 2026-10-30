use wafer_core::hash_map::java_hash_map_order;
use wafer_core::random::RandomSource;
use wafer_core::{AABB, BlockPos, Vec3, mth};
use wafer_data::packets::play::clientbound;
use wafer_data::{BlockState, Fluid, damage_types, game_rule, registry, registry_id};

use crate::core::holder_set::HolderSet;
use crate::core::particles::particle_options::ParticleOptions;
use crate::core::particles::particle_types;
use crate::server::level::ServerLevel;
use crate::sounds::sound_event::SoundEvent;
use crate::world::damagesource::DamageSource;
use crate::world::entity::attributes::Attribute;
use crate::world::entity::sound_event;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::level::BlockGetter;
use crate::world::level::block::{self, base_fire_block};
use crate::world::level::clip::{ClipBlock, ClipContext, ClipFluid};
use crate::world::phys::shapes::collision_context::CollisionContext;
use crate::world::level::storage::loot::loot_params::LootParams;
use crate::world::level::update_flags::UPDATE_ALL;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ExplosionInteraction {
    None,
    Block,
    Mob,
    Tnt,
    Trigger,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BlockInteraction {
    Keep,
    Destroy,
    DestroyWithDecay,
    TriggerBlock,
}

const MAX_DROPS_PER_COMBINED_STACK: i32 = 16;
const LARGE_EXPLOSION_RADIUS: f32 = 2.0;

pub struct ServerExplosion {
    pub source: Option<EntityId>,
    pub indirect_source: Option<EntityId>,
    pub damage_source: DamageSource,
    pub center: Vec3,
    pub radius: f32,
    pub fire: bool,
    pub block_interaction: BlockInteraction,
    damage_calculator: ExplosionDamageCalculator,
    hit_players: Vec<(EntityId, Vec3)>,
}

#[derive(Clone)]
pub enum ExplosionDamageCalculator {
    Default,
    Simple { explodes_blocks: bool, damages_entities: bool, knockback_multiplier: Option<f32>, immune_blocks: Option<HolderSet> },
    RespawnAnchor { pos: BlockPos, in_water: bool },
    UsedPortal,
    TntMinecart { primed: bool },
    WitherSkull { dangerous: bool },
}

impl ExplosionDamageCalculator {
    fn get_block_explosion_resistance(&self, level: &mut ServerLevel, test_pos: BlockPos, state: wafer_data::BlockState, fluid: Fluid) -> Option<f32> {
        let resistance = self.get_base_block_explosion_resistance(test_pos, state, fluid);
        match self {
            ExplosionDamageCalculator::TntMinecart { primed: true } if resistance.is_some() && is_rail_or_below_rail(level, test_pos, state) => Some(0.0),
            ExplosionDamageCalculator::WitherSkull { dangerous } => resistance.map(|resistance| crate::world::entity::projectile::hurtingprojectile::wither_skull::get_block_explosion_resistance(*dangerous, state, resistance)),
            _ => resistance,
        }
    }

    fn get_base_block_explosion_resistance(&self, test_pos: BlockPos, state: wafer_data::BlockState, fluid: Fluid) -> Option<f32> {
        match *self {
            ExplosionDamageCalculator::Simple { immune_blocks: Some(ref immune_blocks), .. } => immune_blocks.contains(state.block().0 as i32).then_some(3600000.0),
            ExplosionDamageCalculator::RespawnAnchor { pos, in_water: true } if test_pos == pos => Some(wafer_data::blocks::WATER.info().explosion_resistance),
            ExplosionDamageCalculator::UsedPortal if state.is_block(wafer_data::blocks::NETHER_PORTAL) => None,
            _ if state.is_air() && fluid == Fluid::Empty => None,
            _ => Some(state.block().info().explosion_resistance.max(fluid_explosion_resistance(fluid))),
        }
    }

    fn should_block_explode(&self, level: &mut ServerLevel, pos: BlockPos, state: wafer_data::BlockState) -> bool {
        match *self {
            ExplosionDamageCalculator::Simple { explodes_blocks, .. } => explodes_blocks,
            ExplosionDamageCalculator::UsedPortal => !state.is_block(wafer_data::blocks::NETHER_PORTAL),
            ExplosionDamageCalculator::TntMinecart { primed: true } => !is_rail_or_below_rail(level, pos, state),
            _ => true,
        }
    }
}

impl ExplosionDamageCalculator {
    fn should_damage_entity(&self) -> bool {
        match *self {
            ExplosionDamageCalculator::Simple { damages_entities, .. } => damages_entities,
            _ => true,
        }
    }

    fn get_knockback_multiplier(&self, level: &ServerLevel, entity: EntityId) -> f32 {
        match *self {
            ExplosionDamageCalculator::Simple { knockback_multiplier, .. } => {
                let creative_flying = level.find_player(entity).is_some_and(|player| player.abilities.flying);
                if creative_flying { 0.0 } else { knockback_multiplier.unwrap_or(1.0) }
            }
            _ => 1.0,
        }
    }

    fn get_entity_damage_amount(&self, level: &ServerLevel, explosion: &ServerExplosion, entity: EntityId, exposure: f32) -> f32 {
        let double_radius = explosion.radius * 2.0;
        let dist = level.entity(entity).position().distance_to_sqr(explosion.center).sqrt() / double_radius as f64;
        let pow = (1.0 - dist) * exposure as f64;
        ((pow * pow + pow) / 2.0 * 7.0 * double_radius as f64 + 1.0) as f32
    }
}

pub struct ExplosionParticleInfo {
    pub particle: ParticleOptions,
    pub scaling: f32,
    pub speed: f32,
}

fn simple_particle(particle_type: &'static str) -> ParticleOptions {
    particle_types::parse(particle_type, &wafer_protocol::nbt::Nbt::Compound(Vec::new().into())).expect("a simple particle")
}

fn is_rail_or_below_rail(level: &mut ServerLevel, pos: BlockPos, state: wafer_data::BlockState) -> bool {
    state.is(&wafer_data::block_tags::RAILS) || level.block_state(pos.above()).is(&wafer_data::block_tags::RAILS)
}

fn fluid_explosion_resistance(fluid: Fluid) -> f32 {
    if fluid == Fluid::Empty { 0.0 } else { 100.0 }
}

impl ServerExplosion {
    pub fn is_small(&self) -> bool {
        self.radius < LARGE_EXPLOSION_RADIUS || !self.interacts_with_blocks()
    }

    pub fn can_trigger_blocks(&self, level: &ServerLevel) -> bool {
        if self.block_interaction != BlockInteraction::TriggerBlock {
            return false;
        }
        match self.source {
            Some(source) if level.entity(source).entity_type == registry_id(registry::ENTITY_TYPE, "minecraft:breeze_wind_charge") => level.game_rules.get_bool(game_rule::MOB_GRIEFING),
            _ => true,
        }
    }

    fn interacts_with_blocks(&self) -> bool {
        self.block_interaction != BlockInteraction::Keep
    }
}

impl BlockInteraction {
    pub fn should_affect_blocklike_entities(self) -> bool {
        matches!(self, BlockInteraction::Destroy | BlockInteraction::DestroyWithDecay)
    }
}

impl ServerLevel {
    fn should_affect_blocklike_entities(&self, explosion: &ServerExplosion) -> bool {
        let mob_griefing_enabled = self.game_rules.get_bool(game_rule::MOB_GRIEFING);
        let is_not_wind_charge = explosion.source.is_none_or(|source| {
            let entity_type = self.entity(source).entity_type;
            entity_type != registry_id(registry::ENTITY_TYPE, "minecraft:breeze_wind_charge") && entity_type != registry_id(registry::ENTITY_TYPE, "minecraft:wind_charge")
        });
        if mob_griefing_enabled { is_not_wind_charge } else { explosion.block_interaction.should_affect_blocklike_entities() && is_not_wind_charge }
    }

    fn ignore_explosion(&self, id: EntityId, explosion: &ServerExplosion) -> bool {
        match self.find_entity(id) {
            Some(LevelEntity::ArmorStand(_)) => self.armor_stand_ignore_explosion(id, self.should_affect_blocklike_entities(explosion)),
            Some(LevelEntity::Mob(mob)) if matches!(mob.kind, crate::world::entity::mob::MobKind::Warden(_)) => crate::world::entity::monster::warden::warden::is_digging_or_emerging(&mob.entity),
            Some(LevelEntity::Item(_)) => !self.should_affect_blocklike_entities(explosion),
            Some(LevelEntity::LeashKnot(_) | LevelEntity::Painting(_) | LevelEntity::ItemFrame(_) | LevelEntity::Cushion(_)) => {
                explosion.source.is_some_and(|direct_entity| self.entity(direct_entity).is_in_water()) || !self.should_affect_blocklike_entities(explosion)
            }
            Some(LevelEntity::Minecart(_) | LevelEntity::Boat(_)) => explosion.indirect_source.is_some_and(|indirect| self.is_mob(indirect)) && !self.game_rules.get_bool(game_rule::MOB_GRIEFING),
            _ => false,
        }
    }

    fn destroy_type(&self, rule: wafer_data::GameRule) -> BlockInteraction {
        if self.game_rules.get_bool(rule) { BlockInteraction::DestroyWithDecay } else { BlockInteraction::Destroy }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn explode(
        &mut self,
        source: Option<EntityId>,
        indirect_source: Option<EntityId>,
        damage_source: Option<DamageSource>,
        center: Vec3,
        radius: f32,
        fire: bool,
        interaction: ExplosionInteraction,
    ) {
        self.explode_with_calculator(source, indirect_source, damage_source, ExplosionDamageCalculator::Default, center, radius, fire, interaction);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn explode_with_calculator(
        &mut self,
        source: Option<EntityId>,
        indirect_source: Option<EntityId>,
        damage_source: Option<DamageSource>,
        damage_calculator: ExplosionDamageCalculator,
        center: Vec3,
        radius: f32,
        fire: bool,
        interaction: ExplosionInteraction,
    ) {
        let block_particles = vec![
            (ExplosionParticleInfo { particle: simple_particle("minecraft:poof"), scaling: 0.5, speed: 1.0 }, 1),
            (ExplosionParticleInfo { particle: simple_particle("minecraft:smoke"), scaling: 1.0, speed: 1.0 }, 1),
        ];
        self.explode_with_particles(
            source,
            indirect_source,
            damage_source,
            damage_calculator,
            center,
            radius,
            fire,
            interaction,
            &simple_particle("minecraft:explosion"),
            &simple_particle("minecraft:explosion_emitter"),
            &block_particles,
            &SoundEvent::Reference(sound_event("minecraft:entity.generic.explode")),
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub fn explode_with_particles(
        &mut self,
        source: Option<EntityId>,
        indirect_source: Option<EntityId>,
        damage_source: Option<DamageSource>,
        damage_calculator: ExplosionDamageCalculator,
        center: Vec3,
        radius: f32,
        fire: bool,
        interaction: ExplosionInteraction,
        small_explosion_particles: &ParticleOptions,
        large_explosion_particles: &ParticleOptions,
        block_particles: &[(ExplosionParticleInfo, i32)],
        explosion_sound: &SoundEvent,
    ) {
        let block_interaction = match interaction {
            ExplosionInteraction::None => BlockInteraction::Keep,
            ExplosionInteraction::Block => self.destroy_type(game_rule::BLOCK_EXPLOSION_DROP_DECAY),
            ExplosionInteraction::Mob => {
                if self.game_rules.get_bool(game_rule::MOB_GRIEFING) { self.destroy_type(game_rule::MOB_EXPLOSION_DROP_DECAY) } else { BlockInteraction::Keep }
            }
            ExplosionInteraction::Tnt => self.destroy_type(game_rule::TNT_EXPLOSION_DROP_DECAY),
            ExplosionInteraction::Trigger => BlockInteraction::TriggerBlock,
        };
        let damage_source = damage_source.unwrap_or_else(|| default_damage_source(source, indirect_source));
        let mut explosion = ServerExplosion { source, indirect_source, damage_source, center, radius, fire, block_interaction, damage_calculator, hit_players: Vec::new() };
        let block_count = self.run_explosion(&mut explosion);
        let explosion_particle = if explosion.is_small() { small_explosion_particles } else { large_explosion_particles };
        let play_sound = source.is_none_or(|s| !self.entity(s).is_silent());
        let ids: Vec<EntityId> = self.players.keys().copied().collect();
        for id in ids {
            if self.entity(id).position().distance_to_sqr(center) >= 4096.0 {
                continue;
            }
            let knockback = explosion.hit_players.iter().find(|(p, _)| *p == id).map(|(_, k)| *k);
            self.player(id).connection.send(clientbound::EXPLODE, |w| {
                w.f64(center.x);
                w.f64(center.y);
                w.f64(center.z);
                w.f32(radius);
                w.i32(block_count);
                w.bool(knockback.is_some());
                if let Some(k) = knockback {
                    w.f64(k.x);
                    w.f64(k.y);
                    w.f64(k.z);
                }
                explosion_particle.write(w);
                explosion_sound.write(w);
                w.varint(block_particles.len() as i32);
                for (info, weight) in block_particles {
                    info.particle.write(w);
                    w.f32(info.scaling);
                    w.f32(info.speed);
                    w.varint(*weight);
                }
                w.bool(play_sound);
            });
        }
    }

    fn run_explosion(&mut self, explosion: &mut ServerExplosion) -> i32 {
        self.game_event_from_entity(explosion.source, crate::world::level::gameevent::game_event::EXPLODE, explosion.center);
        let to_blow = self.calculate_exploded_positions(explosion);
        self.hurt_entities(explosion);
        if explosion.interacts_with_blocks() {
            self.interact_with_blocks(explosion, to_blow.clone());
        }
        if explosion.fire {
            self.create_explosion_fire(&to_blow);
        }
        to_blow.len() as i32
    }

    fn calculate_exploded_positions(&mut self, explosion: &ServerExplosion) -> Vec<BlockPos> {
        let mut to_blow: Vec<BlockPos> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for xx in 0..16 {
            for yy in 0..16 {
                for zz in 0..16 {
                    if xx != 0 && xx != 15 && yy != 0 && yy != 15 && zz != 0 && zz != 15 {
                        continue;
                    }
                    let mut xd = (xx as f32 / 15.0 * 2.0 - 1.0) as f64;
                    let mut yd = (yy as f32 / 15.0 * 2.0 - 1.0) as f64;
                    let mut zd = (zz as f32 / 15.0 * 2.0 - 1.0) as f64;
                    let d = (xd * xd + yd * yd + zd * zd).sqrt();
                    xd /= d;
                    yd /= d;
                    zd /= d;
                    let mut remaining_power = explosion.radius * (0.7 + self.random.next_float() * 0.6);
                    let (mut xp, mut yp, mut zp) = (explosion.center.x, explosion.center.y, explosion.center.z);
                    while remaining_power > 0.0 {
                        let pos = BlockPos::containing(xp, yp, zp);
                        let state = self.block_state(pos);
                        let fluid = state.fluid_state();
                        if !self.is_in_world_bounds(pos) {
                            break;
                        }
                        let damage_calculator = &explosion.damage_calculator;
                        if let Some(resistance) = damage_calculator.get_block_explosion_resistance(self, pos, state, fluid.fluid) {
                            remaining_power -= (resistance + 0.3) * 0.3;
                        }
                        if remaining_power > 0.0 && damage_calculator.should_block_explode(self, pos, state) && seen.insert(pos) {
                            to_blow.push(pos);
                        }
                        xp += xd * 0.3f32 as f64;
                        yp += yd * 0.3f32 as f64;
                        zp += zd * 0.3f32 as f64;
                        remaining_power -= 0.22500001;
                    }
                }
            }
        }
        let hashes: Vec<i32> = to_blow.iter().map(|p| p.y.wrapping_add(p.z.wrapping_mul(31)).wrapping_mul(31).wrapping_add(p.x)).collect();
        java_hash_map_order(&hashes).into_iter().map(|i| to_blow[i]).collect()
    }

    fn seen_percent(&mut self, center: Vec3, id: EntityId) -> f32 {
        let bb = self.entity(id).bounding_box();
        let xs = 1.0 / ((bb.max_x - bb.min_x) * 2.0 + 1.0);
        let ys = 1.0 / ((bb.max_y - bb.min_y) * 2.0 + 1.0);
        let zs = 1.0 / ((bb.max_z - bb.min_z) * 2.0 + 1.0);
        let x_offset = (1.0 - (1.0 / xs).floor() * xs) / 2.0;
        let z_offset = (1.0 - (1.0 / zs).floor() * zs) / 2.0;
        if xs < 0.0 || ys < 0.0 || zs < 0.0 {
            return 0.0;
        }
        let collision_context = CollisionContext::of(self, id);
        let (mut hits, mut count) = (0, 0);
        let mut xx = 0.0;
        while xx <= 1.0 {
            let mut yy = 0.0;
            while yy <= 1.0 {
                let mut zz = 0.0;
                while zz <= 1.0 {
                    let x = mth::lerp(xx, bb.min_x, bb.max_x);
                    let y = mth::lerp(yy, bb.min_y, bb.max_y);
                    let z = mth::lerp(zz, bb.min_z, bb.max_z);
                    let from = Vec3::new(x + x_offset, y, z + z_offset);
                    let ctx = ClipContext { from, to: center, block: ClipBlock::Collider, fluid: ClipFluid::None, collision_context };
                    if self.clip(&ctx).miss {
                        hits += 1;
                    }
                    count += 1;
                    zz += zs;
                }
                yy += ys;
            }
            xx += xs;
        }
        hits as f32 / count as f32
    }

    fn hurt_entities(&mut self, explosion: &mut ServerExplosion) {
        if explosion.radius < 1.0e-5 {
            return;
        }
        let double_radius = explosion.radius * 2.0;
        let c = explosion.center;
        let r = double_radius as f64 + 1.0;
        let area = AABB::new(
            mth::floor(c.x - r) as f64,
            mth::floor(c.y - r) as f64,
            mth::floor(c.z - r) as f64,
            mth::floor(c.x + r) as f64,
            mth::floor(c.y + r) as f64,
            mth::floor(c.z + r) as f64,
        );
        for id in self.get_entities(explosion.source, &area) {
            if self.entity_ref(id).is_none() || self.ignore_explosion(id, explosion) {
                continue;
            }
            let dist = self.entity(id).position().distance_to_sqr(c).sqrt() / double_radius as f64;
            if dist > 1.0 {
                continue;
            }
            let is_tnt = matches!(self.find_entity(id), Some(LevelEntity::PrimedTnt(_)));
            let origin = if is_tnt { self.entity(id).position() } else { self.entity(id).eye_position() };
            let direction = origin.subtract(c).normalize();
            let should_damage_entity = explosion.damage_calculator.should_damage_entity();
            let knockback_multiplier = explosion.damage_calculator.get_knockback_multiplier(self, id);
            let exposure = if !should_damage_entity && knockback_multiplier == 0.0 { 0.0 } else { self.seen_percent(c, id) };
            if should_damage_entity && explosion.source.is_none_or(|source| self.do_teams_allow_damage(source, id)) {
                let damage = explosion.damage_calculator.get_entity_damage_amount(self, explosion, id, exposure);
                self.hurt_server(id, explosion.damage_source.clone(), damage);
            }
            if self.entity_ref(id).is_none() {
                continue;
            }
            let knockback_resistance = if self.is_living(id) { self.attribute_value(id, Attribute::ExplosionKnockbackResistance) } else { 0.0 };
            let knockback_power = (1.0 - dist) * exposure as f64 * knockback_multiplier as f64 * (1.0 - knockback_resistance);
            let knockback = direction.scale(knockback_power);
            self.push_from_explosion(id, knockback);
            let redirectable = wafer_data::tag("minecraft:entity_type", "minecraft:redirectable_projectile").contains(&self.entity(id).entity_type);
            if redirectable && self.is_projectile(id) {
                let owner = explosion.damage_source.causing_entity.map(|owner| crate::world::entity::entity_reference::EntityReference::of(self.entity(owner).uuid));
                self.projectile_set_owner_reference(id, owner);
            } else if let Some(player) = self.find_player(id)
                && !self.is_spectator(id)
                && !(player.game_mode.game_type.is_creative() && player.abilities.flying)
            {
                explosion.hit_players.push((id, knockback));
            }
            self.entity_on_explosion_hit(id, explosion.source);
        }
    }

    fn interact_with_blocks(&mut self, explosion: &ServerExplosion, mut targets: Vec<BlockPos>) {
        let mut stacks: Vec<(BlockPos, ItemStack)> = Vec::new();
        crate::util::shuffle(&mut targets, &mut self.random);
        for pos in targets {
            let state = self.block_state(pos);
            self.on_explosion_hit(state, pos, explosion, &mut stacks);
        }
        for (pos, stack) in stacks {
            self.pop_resource(pos, stack);
        }
    }

    fn on_explosion_hit(&mut self, state: BlockState, pos: BlockPos, explosion: &ServerExplosion, stacks: &mut Vec<(BlockPos, ItemStack)>) {
        block::of(state).on_explosion_hit(state, self, pos, explosion, stacks);
    }

    pub fn block_on_explosion_hit(&mut self, state: BlockState, pos: BlockPos, explosion: &ServerExplosion, stacks: &mut Vec<(BlockPos, ItemStack)>) {
        if state.is_air() || explosion.block_interaction == BlockInteraction::TriggerBlock {
            return;
        }
        let behaviour = block::of(state);
        if behaviour.drop_from_explosion() {
            let drop_experience = explosion.indirect_source.is_some_and(|e| self.is_player(e));
            let block_entity = if state.has_block_entity() { self.get_block_entity(pos).cloned() } else { None };
            let tool = ItemStack::empty();
            behaviour.spawn_after_break(state, self, pos, None, drop_experience);
            let mut params = LootParams::new(self);
            params.origin = Some(Vec3::at_center_of(pos));
            params.tool = Some(&tool);
            params.block_entity = block_entity.as_ref();
            params.this_entity = explosion.source;
            if explosion.block_interaction == BlockInteraction::DestroyWithDecay {
                params.explosion_radius = Some(explosion.radius);
            }
            for stack in behaviour.get_drops(state, params) {
                add_or_append_stack(stacks, stack, pos);
            }
        }
        self.set_block(pos, wafer_data::blocks::AIR.default_state(), UPDATE_ALL);
        behaviour.was_exploded(self, pos, explosion);
    }

    fn create_explosion_fire(&mut self, targets: &[BlockPos]) {
        for &pos in targets {
            if self.random.next_int_bounded(3) == 0 && self.block_state(pos).is_air() && self.block_state(pos.below()).is_solid_render() {
                let fire = base_fire_block::get_state(self, pos);
                self.set_block(pos, fire, UPDATE_ALL);
            }
        }
    }

    pub fn is_in_world_bounds(&self, pos: BlockPos) -> bool {
        !self.chunks.is_outside_build_height(pos.y) && pos.x >= -30_000_000 && pos.z >= -30_000_000 && pos.x < 30_000_000 && pos.z < 30_000_000
    }
}

fn default_damage_source(source: Option<EntityId>, causing: Option<EntityId>) -> DamageSource {
    let ty = if source.is_some() && causing.is_some() { damage_types::PLAYER_EXPLOSION } else { damage_types::EXPLOSION };
    DamageSource { ty, causing_entity: causing, direct_entity: source, damage_source_position: None }
}

fn add_or_append_stack(stacks: &mut Vec<(BlockPos, ItemStack)>, mut stack: ItemStack, pos: BlockPos) {
    for (_, collected) in stacks.iter_mut() {
        if stack.count + collected.count <= collected.max_stack_size() && ItemStack::is_same_item_same_components(collected, &stack) {
            let delta = (collected.max_stack_size().min(MAX_DROPS_PER_COMBINED_STACK) - collected.count).min(stack.count);
            *collected = collected.copy_with_count(collected.count + delta);
            stack.count -= delta;
        }
        if stack.is_empty() {
            return;
        }
    }
    stacks.push((pos, stack));
}
