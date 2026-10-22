use wafer_core::random::RandomSource;
use wafer_core::{BlockPos, Vec3, mth};
use wafer_data::{BlockState, block_tags, damage_type_tags, entity_type_tags, game_rule, items, mob_effects, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use crate::network::chat::{Component, HoverEvent};
use crate::server::level::ServerLevel;
use crate::server::level::server_boss_event::{BossBarColor, BossBarOverlay, ServerBossEvent};
use crate::world::damagesource::DamageSource;
use crate::world::difficulty::Difficulty;
use crate::world::effect::mob_effect_instance::MobEffectInstance;
use crate::world::entity::ai::MobAi;
use crate::world::entity::ai::control::flying_move_control::FlyingMoveControl;
use crate::world::entity::ai::control::move_control::MoveControlKind;
use crate::world::entity::ai::goal::goal::Goal;
use crate::world::entity::ai::goal::look_at_player_goal::LookAtPlayerGoal;
use crate::world::entity::ai::goal::random_look_around_goal::RandomLookAroundGoal;
use crate::world::entity::ai::goal::ranged_attack_goal::RangedAttackGoal;
use crate::world::entity::ai::goal::target::hurt_by_target_goal::HurtByTargetGoal;
use crate::world::entity::ai::goal::target::nearest_attackable_target_goal::{NearestAttackableTargetGoal, TargetType};
use crate::world::entity::ai::goal::water_avoiding_random_flying_goal::WaterAvoidingRandomFlyingGoal;
use crate::world::entity::ai::navigation::path_navigation::{PathNavigation, PathNavigationKind};
use crate::world::entity::ai::targeting::targeting_conditions::TargetingConditions;
use crate::world::entity::attributes::{Attribute, WITHER_ATTRIBUTES};
use crate::world::entity::data::{DataValue, mob_defaults};
use crate::world::entity::living_entity::LivingEntity;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::projectile::hurtingprojectile::wither_skull::{self, WitherSkull};
use crate::world::entity::{Entity, EntityDimensions, EntityId, LevelEntity, create_insecure_uuid};
use crate::world::item::ItemStack;
use crate::world::level::BlockGetter;
use crate::world::level::server_explosion::ExplosionInteraction;
use crate::world::scores::player_team::PlayerTeam;

pub const DATA_TARGET_A: usize = 16;
pub const DATA_TARGET_B: usize = 17;
pub const DATA_TARGET_C: usize = 18;
const DATA_TARGETS: [usize; 3] = [DATA_TARGET_A, DATA_TARGET_B, DATA_TARGET_C];
pub const DATA_ID_INV: usize = 19;
const INVULNERABLE_TICKS: i32 = 220;
const DEFAULT_INVULNERABLE_TICKS: i32 = 0;
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.9, 3.5);
const MOVE_CONTROL: FlyingMoveControl = FlyingMoveControl::new(10, false);

pub struct WitherBoss {
    pub x_rot_heads: [f32; 2],
    pub y_rot_heads: [f32; 2],
    pub x_rot_o_heads: [f32; 2],
    pub y_rot_o_heads: [f32; 2],
    next_head_update: [i32; 2],
    idle_head_updates: [i32; 2],
    destroy_blocks_tick: i32,
    pub boss_event: Option<ServerBossEvent>,
}

fn living_entity_selector(_mob: EntityId, target: EntityId, level: &ServerLevel) -> bool {
    !entity_type_tags::WITHER_FRIENDS.contains(level.entity(target).entity_type as usize) && level.attackable(target)
}

fn targeting_conditions(selector: &dyn Fn(EntityId, &ServerLevel) -> bool) -> TargetingConditions<'_> {
    TargetingConditions::for_combat().range(20.0).selector(selector)
}

fn display_name(entity: &Entity) -> Component {
    let type_id = registry::ENTITY_TYPE[entity.entity_type as usize];
    let name = match entity.custom_name() {
        Some(name) => name,
        None => Component::translatable("entity.minecraft.wither"),
    };
    PlayerTeam::format_name_for_team(None, name.clone())
        .with_hover(HoverEvent::ShowEntity { id: type_id, uuid: entity.uuid, name: Some(Box::new(name)) })
        .with_insertion(&crate::world::level::storage::format_uuid(entity.uuid))
}

impl WitherBoss {
    pub fn create() -> Mob {
        let mut data = mob_defaults();
        data.extend([DataValue::Int(0), DataValue::Int(0), DataValue::Int(0), DataValue::Int(DEFAULT_INVULNERABLE_TICKS)]);
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:wither"), DIMENSIONS, data);
        let wither = WitherBoss {
            x_rot_heads: [0.0; 2],
            y_rot_heads: [0.0; 2],
            x_rot_o_heads: [0.0; 2],
            y_rot_o_heads: [0.0; 2],
            next_head_update: [0; 2],
            idle_head_updates: [0; 2],
            destroy_blocks_tick: 0,
            boss_event: None,
        };
        let mut mob = Mob::new(entity, LivingEntity::new(&WITHER_ATTRIBUTES), MobKind::Wither(Box::new(wither)));
        let boss_event_id = create_insecure_uuid(&mut mob.entity.random);
        let mut boss_event = ServerBossEvent::new(boss_event_id, display_name(&mob.entity), BossBarColor::Purple, BossBarOverlay::Progress);
        boss_event.darken_screen = true;
        wither_mut(&mut mob).boss_event = Some(boss_event);
        mob.ai.move_control.kind = MoveControlKind::Flying(MOVE_CONTROL);
        let max_health = mob.living.attributes.value(Attribute::MaxHealth) as f32;
        mob.set_health(max_health);
        mob.xp_reward = 50;
        mob
    }

    pub fn create_navigation(follow_range_base: f64) -> PathNavigation {
        let mut flying_path_navigation = PathNavigation::of_kind(PathNavigationKind::Flying, follow_range_base);
        flying_path_navigation.path_finder.node_evaluator.node_evaluator_mut().can_open_doors = false;
        flying_path_navigation.set_can_float(true);
        flying_path_navigation
    }

    pub fn register_goals(ai: &mut MobAi) {
        let goal_selector = &mut ai.goal_selector;
        goal_selector.add_goal(0, Goal::WitherDoNothing);
        goal_selector.add_goal(2, Goal::RangedAttack(RangedAttackGoal::new(1.0, 40, 20.0)));
        goal_selector.add_goal(5, Goal::WaterAvoidingRandomFlying(WaterAvoidingRandomFlyingGoal::new(1.0)));
        goal_selector.add_goal(6, Goal::LookAtPlayer(LookAtPlayerGoal::new(8.0)));
        goal_selector.add_goal(7, Goal::RandomLookAround(RandomLookAroundGoal::default()));
        let target_selector = &mut ai.target_selector;
        target_selector.add_goal(1, Goal::HurtByTarget(HurtByTargetGoal::new(&[])));
        target_selector.add_goal(2, Goal::NearestAttackableTarget(NearestAttackableTargetGoal::with(TargetType::LivingEntity, 0, false, false, Some(living_entity_selector))));
    }

    pub fn make_invulnerable(mob: &mut Mob) {
        set_invulnerable_ticks(&mut mob.entity, INVULNERABLE_TICKS);
        wither_mut(mob).boss_event.as_mut().expect("a boss event").progress = 0.0;
        let max_health = mob.living.attributes.value(Attribute::MaxHealth) as f32;
        mob.set_health(max_health / 3.0);
    }

    pub fn add_additional_save_data(mob: &Mob, output: &mut Vec<(String, Nbt)>) {
        output.push(("Invul".into(), Nbt::Int(get_invulnerable_ticks(&mob.entity))));
    }

    pub fn read_additional_save_data(mob: &mut Mob, input: &Nbt) {
        let invulnerable_ticks = input.get("Invul").and_then(crate::world::level::block::entity::int_value).unwrap_or(DEFAULT_INVULNERABLE_TICKS);
        set_invulnerable_ticks(&mut mob.entity, invulnerable_ticks);
        if mob.entity.has_custom_name() {
            let name = display_name(&mob.entity);
            wither_mut(mob).boss_event.as_mut().expect("a boss event").name = name;
        }
    }
}

pub fn wither(mob: &Mob) -> &WitherBoss {
    match &mob.kind {
        MobKind::Wither(wither) => wither,
        _ => unreachable!("not a wither"),
    }
}

pub fn wither_mut(mob: &mut Mob) -> &mut WitherBoss {
    match &mut mob.kind {
        MobKind::Wither(wither) => wither,
        _ => unreachable!("not a wither"),
    }
}

pub fn can_destroy(state: BlockState) -> bool {
    !state.is_air() && !state.is(&block_tags::WITHER_IMMUNE)
}

pub fn get_invulnerable_ticks(entity: &Entity) -> i32 {
    match entity.data.get(DATA_ID_INV) {
        DataValue::Int(ticks) => *ticks,
        _ => 0,
    }
}

pub fn set_invulnerable_ticks(entity: &mut Entity, invulnerable_ticks: i32) {
    entity.data.set(DATA_ID_INV, DataValue::Int(invulnerable_ticks));
}

pub fn get_alternative_target(entity: &Entity, head_index: usize) -> i32 {
    match entity.data.get(DATA_TARGETS[head_index]) {
        DataValue::Int(id) => *id,
        _ => 0,
    }
}

fn set_alternative_target(entity: &mut Entity, head_index: usize, entity_id: i32) {
    entity.data.set(DATA_TARGETS[head_index], DataValue::Int(entity_id));
}

pub fn is_powered(mob: &Mob) -> bool {
    mob.health <= mob.living.attributes.value(Attribute::MaxHealth) as f32 / 2.0
}

fn rotlerp(a: f32, b: f32, max: f32) -> f32 {
    let mut diff = mth::wrap_degrees(b - a);
    if diff > max {
        diff = max;
    }
    if diff < -max {
        diff = -max;
    }
    a + diff
}

pub fn get_ambient_sound() -> &'static str {
    "minecraft:entity.wither.ambient"
}

pub fn get_hurt_sound() -> &'static str {
    "minecraft:entity.wither.hurt"
}

pub fn get_death_sound() -> &'static str {
    "minecraft:entity.wither.death"
}

impl ServerLevel {
    pub fn is_wither(&self, id: EntityId) -> bool {
        matches!(self.mob(id).map(|mob| &mob.kind), Some(MobKind::Wither(_)))
    }

    fn with_wither_boss_event(&mut self, id: EntityId, f: impl FnOnce(&mut ServerBossEvent, &ServerLevel)) {
        let Some(mut boss_event) = self.mob_mut(id).and_then(|mob| wither_mut(mob).boss_event.take()) else { return };
        f(&mut boss_event, self);
        if let Some(mob) = self.mob_mut(id) {
            wither_mut(mob).boss_event = Some(boss_event);
        }
    }

    pub fn wither_start_seen_by_player(&mut self, id: EntityId, player: EntityId) {
        self.with_wither_boss_event(id, |boss_event, level| boss_event.add_player(std::slice::from_ref(level), player));
    }

    pub fn wither_stop_seen_by_player(&mut self, id: EntityId, player: EntityId) {
        self.with_wither_boss_event(id, |boss_event, level| boss_event.remove_player(std::slice::from_ref(level), player));
    }

    fn wither_set_boss_event_progress(&mut self, id: EntityId, progress: f32) {
        self.with_wither_boss_event(id, |boss_event, level| {
            boss_event.set_progress(std::slice::from_ref(level), progress);
        });
    }

    fn wither_update_boss_event_name(&mut self, id: EntityId) {
        let name = display_name(self.entity(id));
        self.with_wither_boss_event(id, |boss_event, level| {
            boss_event.set_name(std::slice::from_ref(level), name);
        });
    }

    fn wither_get_head_x(&self, id: EntityId, index: usize) -> f64 {
        let e = self.entity(id);
        if index == 0 {
            return e.x();
        }
        let head_angle = (self.living(id).y_body_rot + (180 * (index as i32 - 1)) as f32) * (std::f64::consts::PI / 180.0) as f32;
        let cos = mth::cos(head_angle as f64);
        e.x() + cos as f64 * 1.3 * (self.attribute_value(id, Attribute::Scale) as f32) as f64
    }

    fn wither_get_head_y(&self, id: EntityId, index: usize) -> f64 {
        let height: f32 = if index == 0 { 3.0 } else { 2.2 };
        self.entity(id).y() + (height * (self.attribute_value(id, Attribute::Scale) as f32)) as f64
    }

    fn wither_get_head_z(&self, id: EntityId, index: usize) -> f64 {
        let e = self.entity(id);
        if index == 0 {
            return e.z();
        }
        let head_angle = (self.living(id).y_body_rot + (180 * (index as i32 - 1)) as f32) * (std::f64::consts::PI / 180.0) as f32;
        let sin = mth::sin(head_angle as f64);
        e.z() + sin as f64 * 1.3 * (self.attribute_value(id, Attribute::Scale) as f32) as f64
    }

    pub fn wither_ai_step(&mut self, id: EntityId) {
        let mut delta_movement = self.entity(id).delta_movement.multiply(1.0, 0.6, 1.0);
        let target_a = get_alternative_target(self.entity(id), 0);
        if target_a > 0
            && let Some(entity) = self.entity_ref(target_a)
        {
            let (ex, ey, ez) = (entity.x(), entity.y(), entity.z());
            let e = self.entity(id);
            let powered = is_powered(self.mob(id).expect("a wither"));
            let mut yd = delta_movement.y;
            if e.y() < ey || !powered && e.y() < ey + 5.0 {
                yd = yd.max(0.0);
                yd += 0.3 - yd * 0.6f32 as f64;
            }
            delta_movement = Vec3::new(delta_movement.x, yd, delta_movement.z);
            let delta = Vec3::new(ex - e.x(), 0.0, ez - e.z());
            if delta.horizontal_distance_sqr() > 9.0 {
                let scale = delta.normalize();
                delta_movement = delta_movement.add(scale.x * 0.3 - delta_movement.x * 0.6, 0.0, scale.z * 0.3 - delta_movement.z * 0.6);
            }
        }
        self.entity_mut(id).set_delta_movement(delta_movement);
        if delta_movement.horizontal_distance_sqr() > 0.05 {
            let y_rot = mth::atan2(delta_movement.z, delta_movement.x) as f32 * mth::RAD_TO_DEG - 90.0;
            self.entity_mut(id).set_y_rot(y_rot);
        }
        self.monster_ai_step(id);
        let Some(mob) = self.mob_mut(id) else { return };
        let wither = wither_mut(mob);
        for i in 0..2 {
            wither.y_rot_o_heads[i] = wither.y_rot_heads[i];
            wither.x_rot_o_heads[i] = wither.x_rot_heads[i];
        }
        for i in 0..2 {
            let entity_id = get_alternative_target(self.entity(id), i + 1);
            let entity = if entity_id > 0 { self.entity_ref(entity_id).map(|e| (e.x(), e.eye_y(), e.z())) } else { None };
            let y_body_rot = self.living(id).y_body_rot;
            match entity {
                Some((ex, eye_y, ez)) => {
                    let hx = self.wither_get_head_x(id, i + 1);
                    let hy = self.wither_get_head_y(id, i + 1);
                    let hz = self.wither_get_head_z(id, i + 1);
                    let xd = ex - hx;
                    let yd = eye_y - hy;
                    let zd = ez - hz;
                    let sd = (xd * xd + zd * zd).sqrt();
                    let y_rot_d = (mth::atan2(zd, xd) * mth::RAD_TO_DEG as f64) as f32 - 90.0;
                    let x_rot_d = (-(mth::atan2(yd, sd) * mth::RAD_TO_DEG as f64)) as f32;
                    let wither = wither_mut(self.mob_mut(id).expect("a wither"));
                    wither.x_rot_heads[i] = rotlerp(wither.x_rot_heads[i], x_rot_d, 40.0);
                    wither.y_rot_heads[i] = rotlerp(wither.y_rot_heads[i], y_rot_d, 10.0);
                }
                None => {
                    let wither = wither_mut(self.mob_mut(id).expect("a wither"));
                    wither.y_rot_heads[i] = rotlerp(wither.y_rot_heads[i], y_body_rot, 10.0);
                }
            }
        }
        let powered = is_powered(self.mob(id).expect("a wither"));
        for _ in 0..3 {
            let random = &mut self.entity_mut(id).random;
            random.next_gaussian();
            random.next_gaussian();
            random.next_gaussian();
            if powered && self.random.next_int_bounded(4) == 0 {
                let random = &mut self.entity_mut(id).random;
                random.next_gaussian();
                random.next_gaussian();
                random.next_gaussian();
            }
        }
        if get_invulnerable_ticks(self.entity(id)) > 0 {
            let random = &mut self.entity_mut(id).random;
            for _ in 0..3 {
                random.next_gaussian();
                random.next_float();
                random.next_gaussian();
            }
        }
    }

    pub fn wither_custom_server_ai_step(&mut self, id: EntityId) {
        self.wither_update_boss_event_name(id);
        let invulnerable_ticks = get_invulnerable_ticks(self.entity(id));
        if invulnerable_ticks > 0 {
            let new_count = invulnerable_ticks - 1;
            self.wither_set_boss_event_progress(id, 1.0 - new_count as f32 / 220.0);
            if new_count <= 0 {
                let e = self.entity(id);
                let position = Vec3::new(e.x(), e.eye_y(), e.z());
                self.explode(Some(id), Some(id), None, position, 7.0, false, ExplosionInteraction::Mob);
                if !self.entity(id).is_silent() {
                    let block_position = self.entity(id).block_position();
                    self.global_level_event(crate::server::level::events::SOUND_WITHER_BOSS_SPAWN, block_position, 0);
                }
            }
            set_invulnerable_ticks(self.entity_mut(id), new_count);
            if self.entity(id).tick_count % 10 == 0 {
                self.heal(id, 10.0);
            }
            return;
        }
        for i in 1..3 {
            let tick_count = self.entity(id).tick_count;
            if tick_count < wither(self.mob(id).expect("a wither")).next_head_update[i - 1] {
                continue;
            }
            let mob = self.mob_mut(id).expect("a wither");
            let next_head_update = tick_count + 10 + mob.entity.random.next_int_bounded(10);
            wither_mut(mob).next_head_update[i - 1] = next_head_update;
            if matches!(self.difficulty, Difficulty::Normal | Difficulty::Hard) {
                let wither = wither_mut(self.mob_mut(id).expect("a wither"));
                let idle = wither.idle_head_updates[i - 1];
                wither.idle_head_updates[i - 1] += 1;
                if idle > 15 {
                    let e = self.entity_mut(id);
                    let (x, y, z) = (e.x(), e.y(), e.z());
                    let xt = mth::next_double(&mut e.random, x - 10.0, x + 10.0);
                    let yt = mth::next_double(&mut e.random, y - 5.0, y + 5.0);
                    let zt = mth::next_double(&mut e.random, z - 10.0, z + 10.0);
                    self.wither_perform_ranged_attack_at(id, i + 1, xt, yt, zt, true);
                    wither_mut(self.mob_mut(id).expect("a wither")).idle_head_updates[i - 1] = 0;
                }
            }
            let head_target = get_alternative_target(self.entity(id), i);
            if head_target > 0 {
                let current = self.entity_ref(head_target).map(|e| e.id).filter(|&current| self.is_living(current));
                let valid = current.is_some_and(|current| {
                    self.can_attack(id, current) && !(self.entity(id).distance_to_sqr(self.entity(current)) > 900.0) && self.has_line_of_sight(id, current)
                });
                if valid {
                    self.wither_perform_ranged_attack_at_target(id, i + 1, current.expect("a head target"));
                    let mob = self.mob_mut(id).expect("a wither");
                    let tick_count = mob.entity.tick_count;
                    let next_head_update = tick_count + 40 + mob.entity.random.next_int_bounded(20);
                    let wither = wither_mut(mob);
                    wither.next_head_update[i - 1] = next_head_update;
                    wither.idle_head_updates[i - 1] = 0;
                } else {
                    set_alternative_target(self.entity_mut(id), i, 0);
                }
            } else {
                let area = self.entity(id).bounding_box().inflate(20.0, 8.0, 20.0);
                let candidates = self.get_entities_matching(None, &area, |level, entity| level.is_living(entity));
                let selector = |target: EntityId, level: &ServerLevel| living_entity_selector(id, target, level);
                let conditions = targeting_conditions(&selector);
                let entities: Vec<EntityId> = candidates.into_iter().filter(|&entity| conditions.test(self, Some(id), entity)).collect();
                if !entities.is_empty() {
                    let selected = entities[self.entity_mut(id).random.next_int_bounded(entities.len() as i32) as usize];
                    set_alternative_target(self.entity_mut(id), i, selected);
                }
            }
        }
        let target = self.get_target(id).unwrap_or(0);
        set_alternative_target(self.entity_mut(id), 0, target);
        let wither = wither_mut(self.mob_mut(id).expect("a wither"));
        if wither.destroy_blocks_tick > 0 {
            wither.destroy_blocks_tick -= 1;
            if wither.destroy_blocks_tick == 0 && self.game_rules.get_bool(game_rule::MOB_GRIEFING) {
                let mut destroyed = false;
                let e = self.entity(id);
                let width = mth::floor_f(e.dimensions.width / 2.0 + 1.0);
                let height = mth::floor_f(e.dimensions.height);
                let block_position = e.block_position();
                let from = BlockPos::new(block_position.x - width, block_position.y, block_position.z - width);
                let to = BlockPos::new(block_position.x + width, block_position.y + height, block_position.z + width);
                for pos in BlockPos::between_closed(from, to) {
                    let state = self.block_state(pos);
                    if can_destroy(state) {
                        destroyed = self.destroy_block(pos, true, Some(id), crate::world::level::update_flags::UPDATE_LIMIT) || destroyed;
                    }
                }
                if destroyed {
                    let block_position = self.entity(id).block_position();
                    self.level_event(None, crate::server::level::events::SOUND_WITHER_BLOCK_BREAK, block_position, 0);
                }
            }
        }
        if self.entity(id).tick_count % 20 == 0 {
            self.heal(id, 1.0);
        }
        let mob = self.mob(id).expect("a wither");
        let progress = mob.health / mob.living.attributes.value(Attribute::MaxHealth) as f32;
        self.wither_set_boss_event_progress(id, progress);
    }


    fn wither_perform_ranged_attack_at_target(&mut self, id: EntityId, head: usize, target: EntityId) {
        let t = self.entity(target);
        let (tx, ty, tz) = (t.x(), t.y() + t.dimensions.eye_height as f64 * 0.5, t.z());
        let dangerous = head == 0 && self.entity_mut(id).random.next_float() < 0.001;
        self.wither_perform_ranged_attack_at(id, head, tx, ty, tz, dangerous);
    }

    fn wither_perform_ranged_attack_at(&mut self, id: EntityId, head: usize, tx: f64, ty: f64, tz: f64, dangerous: bool) {
        if !self.entity(id).is_silent() {
            let block_position = self.entity(id).block_position();
            self.level_event(None, crate::server::level::events::SOUND_WITHER_BOSS_SHOOT, block_position, 0);
        }
        let hx = self.wither_get_head_x(id, head);
        let hy = self.wither_get_head_y(id, head);
        let hz = self.wither_get_head_z(id, head);
        let direction = Vec3::new(tx - hx, ty - hy, tz - hz);
        let mut skull = WitherSkull::from_mob(self, id, direction.normalize());
        skull.owner = Some(crate::world::entity::entity_reference::EntityReference::of(self.entity(id).uuid));
        if dangerous {
            wither_skull::set_dangerous(&mut skull.entity, true);
        }
        skull.entity.set_pos(Vec3::new(hx, hy, hz));
        self.add_fresh_entity(LevelEntity::Projectile(Box::new(skull)));
    }

    pub fn wither_perform_ranged_attack(&mut self, id: EntityId, target: EntityId) {
        self.wither_perform_ranged_attack_at_target(id, 0, target);
    }

    pub fn wither_hurt_server(&mut self, id: EntityId, source: DamageSource, damage: f32) -> bool {
        if self.is_invulnerable_to(id, &source) {
            return false;
        }
        if source.is(&damage_type_tags::WITHER_IMMUNE_TO) || source.causing_entity.is_some_and(|entity| self.is_wither(entity)) {
            return false;
        }
        if get_invulnerable_ticks(self.entity(id)) > 0 && !source.is(&damage_type_tags::BYPASSES_INVULNERABILITY) {
            return false;
        }
        if is_powered(self.mob(id).expect("a wither"))
            && source.direct_entity.is_some_and(|direct| self.as_projectile(direct).is_some_and(|projectile| projectile.abstract_arrow().is_some() || matches!(projectile.kind, crate::world::entity::projectile::projectile::ProjectileKind::WindCharge(_))))
        {
            return false;
        }
        if source.causing_entity.is_some_and(|entity| entity_type_tags::WITHER_FRIENDS.contains(self.entity(entity).entity_type as usize)) {
            return false;
        }
        let wither = wither_mut(self.mob_mut(id).expect("a wither"));
        if wither.destroy_blocks_tick <= 0 {
            wither.destroy_blocks_tick = 20;
        }
        for idle in &mut wither.idle_head_updates {
            *idle += 3;
        }
        self.living_entity_hurt_server(id, source, damage)
    }

    pub fn wither_drop_custom_death_loot(&mut self, id: EntityId, source: &DamageSource, killed_by_player: bool) {
        self.mob_drop_custom_death_loot(id, source, killed_by_player);
        if let Some(nether_star) = self.spawn_at_location(id, ItemStack::new(items::NETHER_STAR, 1), 0.0)
            && let Some(LevelEntity::Item(item)) = self.entities.get_mut(&nether_star)
        {
            item.age = -6000;
        }
    }

    pub fn wither_check_despawn(&mut self, id: EntityId) {
        if self.difficulty == Difficulty::Peaceful {
            self.discard(id);
        } else {
            self.living_mut(id).no_action_time = 0;
        }
    }

    pub fn wither_can_be_affected(&self, id: EntityId, effect: &MobEffectInstance) -> bool {
        if effect.effect == mob_effects::WITHER { false } else { self.living_entity_can_be_affected(id, effect) }
    }
}
