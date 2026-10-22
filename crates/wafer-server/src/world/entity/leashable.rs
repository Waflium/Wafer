use wafer_core::{AABB, BlockPos, Vec3};
use wafer_data::{game_rule, items};
use wafer_protocol::nbt::Nbt;

use super::decoration::leash_fence_knot_entity::LeashFenceKnotEntity;
use super::entity_save::uuid_from_nbt;
use super::mob::MobKind;
use super::{EntityId, LevelEntity, sound_event};
use crate::network::connection::encode;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::ai::goal::goal::Flag;
use crate::world::item::ItemStack;
use crate::world::level::BlockGetter;
use crate::world::level::storage::uuid_ints;

pub const LEASH_TAG: &str = "leash";
pub const LEASH_TOO_FAR_DIST: f64 = 12.0;
pub const LEASH_ELASTIC_DIST: f64 = 6.0;
const AXIS_SPECIFIC_ELASTICITY: Vec3 = Vec3::new(0.8, 0.2, 0.8);
const ENTITY_ATTACHMENT_POINT: &[Vec3] = &[Vec3::new(0.0, 0.5, 0.5)];
const LEASHER_ATTACHMENT_POINT: &[Vec3] = &[Vec3::new(0.0, 0.5, 0.0)];
const SHARED_QUAD_ATTACHMENT_POINTS: &[Vec3] = &[Vec3::new(-0.5, 0.5, 0.5), Vec3::new(-0.5, 0.5, -0.5), Vec3::new(0.5, 0.5, -0.5), Vec3::new(0.5, 0.5, 0.5)];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LeashInfo {
    Uuid(u128),
    Pos(BlockPos),
}

impl LeashInfo {
    fn read(tag: &Nbt) -> Option<LeashInfo> {
        match tag {
            Nbt::Compound(_) => tag.get("UUID").and_then(uuid_from_nbt).map(LeashInfo::Uuid),
            Nbt::IntArray(v) if v.len() == 3 => Some(LeashInfo::Pos(BlockPos::new(v[0], v[1], v[2]))),
            _ => None,
        }
    }

    fn write(self) -> Nbt {
        match self {
            LeashInfo::Uuid(uuid) => Nbt::Compound(vec![("UUID".into(), uuid_ints(uuid))]),
            LeashInfo::Pos(pos) => Nbt::IntArray(vec![pos.x, pos.y, pos.z]),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct LeashData {
    pub leash_holder: Option<EntityId>,
    leash_holder_info: Option<LeashInfo>,
    pub delayed_leash_info: Option<LeashInfo>,
    pub angular_momentum: f64,
}

impl LeashData {
    fn holder(leash_holder: EntityId, leash_holder_info: LeashInfo) -> LeashData {
        LeashData { leash_holder: Some(leash_holder), leash_holder_info: Some(leash_holder_info), delayed_leash_info: None, angular_momentum: 0.0 }
    }

    fn set_leash_holder(&mut self, leash_holder: EntityId, leash_holder_info: LeashInfo) {
        self.leash_holder = Some(leash_holder);
        self.leash_holder_info = Some(leash_holder_info);
        self.delayed_leash_info = None;
    }

    pub fn read(input: &Nbt) -> Option<LeashData> {
        input.get(LEASH_TAG).and_then(LeashInfo::read).map(|info| LeashData { delayed_leash_info: Some(info), ..LeashData::default() })
    }

    pub fn write(leash_data: Option<&LeashData>, output: &mut Vec<(String, Nbt)>) {
        if let Some(info) = leash_data.and_then(|data| if data.leash_holder.is_some() { data.leash_holder_info } else { data.delayed_leash_info }) {
            output.push((LEASH_TAG.into(), info.write()));
        }
    }
}

pub fn set_entity_link_packet(source: EntityId, dest: Option<EntityId>) -> Vec<u8> {
    encode(wafer_data::packets::play::clientbound::SET_ENTITY_LINK, |w| {
        w.i32(source);
        w.i32(dest.unwrap_or(0));
    })
}

struct Wrench {
    force: Vec3,
    torque: f64,
}

impl Wrench {
    fn torque_from_force(lever_arm: Vec3, force: Vec3) -> f64 {
        lever_arm.z * force.x - lever_arm.x * force.z
    }

    fn accumulate(wrenches: &[Wrench]) -> Wrench {
        let (mut x, mut y, mut z, mut t) = (0.0, 0.0, 0.0, 0.0);
        for wrench in wrenches {
            x += wrench.force.x;
            y += wrench.force.y;
            z += wrench.force.z;
            t += wrench.torque;
        }
        Wrench { force: Vec3::new(x, y, z), torque: t }
    }

    fn scale(self, scale: f64) -> Wrench {
        Wrench { force: self.force.scale(scale), torque: self.torque * scale }
    }
}

fn compute_dampened_spring_interaction(pivot_point: Vec3, object_position: Vec3, spring_slack: f64, object_motion: Vec3, lever_arm: Vec3) -> Option<Wrench> {
    let distance = object_position.distance_to(pivot_point);
    if distance < spring_slack {
        return None;
    }
    let mut displacement = pivot_point.subtract(object_position).normalize().scale(distance - spring_slack);
    let torque = Wrench::torque_from_force(lever_arm, displacement);
    let same_direction_to_movement = object_motion.dot(displacement) >= 0.0;
    if same_direction_to_movement {
        displacement = displacement.scale(0.3f32 as f64);
    }
    Some(Wrench { force: displacement, torque })
}

impl ServerLevel {
    pub fn is_leashable(&self, id: EntityId) -> bool {
        matches!(self.entities.get(&id), Some(LevelEntity::Mob(_) | LevelEntity::Boat(_)))
    }

    pub fn get_leash_data(&self, id: EntityId) -> Option<&LeashData> {
        match self.entities.get(&id) {
            Some(LevelEntity::Mob(mob)) => mob.leash_data.as_ref(),
            Some(LevelEntity::Boat(boat)) => boat.leash_data.as_ref(),
            _ => None,
        }
    }

    fn leash_data_slot(&mut self, id: EntityId) -> Option<&mut Option<LeashData>> {
        match self.entities.get_mut(&id) {
            Some(LevelEntity::Mob(mob)) => Some(&mut mob.leash_data),
            Some(LevelEntity::Boat(boat)) => Some(&mut boat.leash_data),
            _ => None,
        }
    }

    pub fn set_leash_data(&mut self, id: EntityId, leash_data: Option<LeashData>) {
        if let Some(slot) = self.leash_data_slot(id) {
            *slot = leash_data;
        }
    }

    pub fn is_leashed(&self, id: EntityId) -> bool {
        self.get_leash_data(id).is_some_and(|data| data.leash_holder.is_some())
    }

    pub fn may_be_leashed(&self, id: EntityId) -> bool {
        self.get_leash_data(id).is_some()
    }

    pub fn can_have_a_leash_attached_to(&self, id: EntityId, entity: EntityId) -> bool {
        if id == entity {
            return false;
        }
        if self.leash_distance_to(id, entity) > self.leash_snap_distance(id) { false } else { self.can_be_leashed(id) }
    }

    pub fn leash_distance_to(&self, id: EntityId, entity: EntityId) -> f64 {
        self.entity(entity).bounding_box().center().distance_to(self.entity(id).bounding_box().center())
    }

    pub fn can_be_leashed(&self, id: EntityId) -> bool {
        let Some(mob) = self.mob(id) else { return true };
        match mob.kind {
            MobKind::Bat(_) => false,
            MobKind::Squid(_) | MobKind::GlowSquid(_) | MobKind::Axolotl(_) | MobKind::Dolphin(_) => true,
            MobKind::Cod(_) | MobKind::Salmon(_) | MobKind::Pufferfish(_) | MobKind::TropicalFish(_) | MobKind::Tadpole(_) => false,
            MobKind::Turtle(_) => false,
            MobKind::Wolf(_) => !self.is_angry(id),
            MobKind::Cat(_) => true,
            MobKind::Hoglin(_) | MobKind::Zoglin(_) => true,
            MobKind::Panda(_) => false,
            MobKind::Villager(_) | MobKind::WanderingTrader(_) => false,
            MobKind::ZombieHorse(_) => self.is_tamed(id) || !self.is_mob_controlled(id),
            _ => !self.is_enemy(id),
        }
    }

    pub fn can_interact_with_level(&self, id: EntityId) -> bool {
        self.entity_ref(id).is_some_and(|e| !e.is_removed()) && self.is_alive(id) && !self.is_spectator(id)
    }

    pub fn read_leash_data(&mut self, id: EntityId, input: &Nbt) {
        let new_leash_data = LeashData::read(input);
        if self.get_leash_data(id).is_some() && new_leash_data.is_none() {
            self.remove_leash(id);
        }
        self.set_leash_data(id, new_leash_data);
    }

    fn restore_leash_from_save(&mut self, id: EntityId) {
        let Some(info) = self.get_leash_data(id).and_then(|data| data.delayed_leash_info) else { return };
        match info {
            LeashInfo::Uuid(uuid) => {
                if let Some(leasher) = self.entity_manager.visible_entity_storage.get(uuid) {
                    self.set_leashed_to_internal(id, leasher, true);
                    return;
                }
            }
            LeashInfo::Pos(pos) => {
                let knot = LeashFenceKnotEntity::get_or_create_knot(self, pos);
                self.set_leashed_to_internal(id, knot, true);
                return;
            }
        }
        if self.entity(id).tick_count > 100 {
            self.spawn_at_location(id, ItemStack::new(items::LEAD, 1), 0.0);
            self.set_leash_data(id, None);
        }
    }

    pub fn drop_leash(&mut self, id: EntityId) {
        self.drop_leash_with(id, true, true);
    }

    pub fn remove_leash(&mut self, id: EntityId) {
        self.drop_leash_with(id, true, false);
    }

    fn on_leash_removed(&mut self, id: EntityId) {
        if self.get_leash_data(id).is_none()
            && let Some(mob) = self.mob_mut(id)
        {
            mob.clear_home();
        }
    }

    fn drop_leash_with(&mut self, id: EntityId, send_packet: bool, drop_lead: bool) {
        let Some(leash_holder) = self.get_leash_data(id).and_then(|data| data.leash_holder) else { return };
        self.set_leash_data(id, None);
        self.on_leash_removed(id);
        if drop_lead {
            self.spawn_at_location(id, ItemStack::new(items::LEAD, 1), 0.0);
        }
        if send_packet {
            self.send_to_tracking_players(id, &set_entity_link_packet(id, None));
        }
        self.notify_leashee_removed(leash_holder);
    }

    pub fn tick_leash(&mut self, id: EntityId) {
        if self.get_leash_data(id).is_some_and(|data| data.delayed_leash_info.is_some()) {
            self.restore_leash_from_save(id);
        }
        let Some(leash_holder) = self.get_leash_data(id).and_then(|data| data.leash_holder) else { return };
        if !self.can_interact_with_level(id) || !self.can_interact_with_level(leash_holder) {
            if self.game_rules.get_bool(game_rule::ENTITY_DROPS) {
                self.drop_leash(id);
            } else {
                self.remove_leash(id);
            }
        }
        let initial_momentum = self.get_leash_data(id).map_or(0.0, |data| data.angular_momentum);
        let Some(leash_holder) = self.get_leash_holder(id).filter(|&holder| self.entity_ref(holder).is_some()) else { return };
        let distance_to = self.leash_distance_to(id, leash_holder);
        self.when_leashed_to(id, leash_holder);
        if distance_to > self.leash_snap_distance(id) {
            let h = self.entity(leash_holder);
            let (x, y, z) = (h.x(), h.y(), h.z());
            self.play_sound_at(None, x, y, z, sound_event("minecraft:item.lead.break"), SoundSource::Neutral, 1.0, 1.0);
            self.leash_too_far_behaviour(id);
        } else if distance_to > self.leash_elastic_distance(id) - self.entity(leash_holder).dimensions.width as f64 - self.entity(id).dimensions.width as f64
            && self.check_elastic_interactions(id, leash_holder)
        {
            self.on_elastic_leash_pull(id);
        } else {
            self.close_range_leash_behaviour(id, leash_holder);
        }
        let momentum = self.get_leash_data(id).map_or(initial_momentum, |data| data.angular_momentum);
        let y_rot = self.entity(id).y_rot;
        self.entity_mut(id).set_y_rot((y_rot as f64 - momentum) as f32);
        let friction = self.angular_friction(id);
        if let Some(Some(data)) = self.leash_data_slot(id) {
            data.angular_momentum *= friction as f64;
        }
    }

    pub fn on_elastic_leash_pull(&mut self, id: EntityId) {
        self.entity_mut(id).check_fall_distance_accumulation();
        if self.is_abstract_horse(id) && self.is_eating(id) {
            self.set_eating(id, false);
        }
    }

    pub fn leash_snap_distance(&self, _id: EntityId) -> f64 {
        LEASH_TOO_FAR_DIST
    }

    pub fn leash_elastic_distance(&self, _id: EntityId) -> f64 {
        LEASH_ELASTIC_DIST
    }

    fn angular_friction(&mut self, id: EntityId) -> f32 {
        if self.entity(id).on_ground {
            let pos = self.block_pos_below_that_affects_my_movement(id);
            self.block_state(pos).block().info().friction * 0.91
        } else if self.entity(id).is_in_liquid() {
            0.8
        } else {
            0.91
        }
    }

    fn when_leashed_to(&mut self, id: EntityId, leash_holder: EntityId) {
        if self.is_pathfinder_mob(id) {
            let home = self.entity(leash_holder).block_position();
            let radius = self.leash_elastic_distance(id) as i32 - 1;
            self.mob_mut(id).expect("a pathfinder mob").set_home_to(home, radius);
        }
    }

    fn leash_too_far_behaviour(&mut self, id: EntityId) {
        self.drop_leash(id);
        if let Some(mob) = self.mob_mut(id) {
            mob.ai.goal_selector.disable_control_flag(Flag::Move);
        }
    }

    fn close_range_leash_behaviour(&mut self, id: EntityId, leash_holder: EntityId) {
        if self.is_pathfinder_mob(id) && self.should_stay_close_to_leash_holder(id) && !self.is_panicking(id) {
            self.mob_mut(id).expect("a pathfinder mob").ai.goal_selector.enable_control_flag(Flag::Move);
            let (e, h) = (self.entity(id), self.entity(leash_holder));
            let distance_to = e.distance_to(h);
            let delta = Vec3::new(h.x() - e.x(), h.y() - e.y(), h.z() - e.z()).normalize().scale((distance_to - 2.0).max(0.0) as f64);
            let (x, y, z) = (e.x() + delta.x, e.y() + delta.y, e.z() + delta.z);
            let speed = self.follow_leash_speed(id);
            self.get_navigation(id, |navigation, level| navigation.move_to(level, id, x, y, z, speed));
        }
    }

    fn should_stay_close_to_leash_holder(&self, _id: EntityId) -> bool {
        true
    }

    fn follow_leash_speed(&self, id: EntityId) -> f64 {
        if self.is_llama(id) { crate::world::entity::animal::equine::llama::follow_leash_speed() } else { 1.0 }
    }

    pub fn support_quad_leash(&self, id: EntityId) -> bool {
        !self.is_llama(id) && self.is_abstract_horse(id) || self.is_boat(id)
    }

    pub fn support_quad_leash_as_holder(&self, _id: EntityId) -> bool {
        false
    }

    fn get_holder_movement(&self, leash_holder: EntityId) -> Vec3 {
        if self.mob(leash_holder).is_some_and(|mob| mob.is_no_ai()) { Vec3::ZERO } else { self.known_movement(leash_holder) }
    }

    fn check_elastic_interactions(&mut self, id: EntityId, leash_holder: EntityId) -> bool {
        let quad_connection = self.support_quad_leash_as_holder(leash_holder) && self.support_quad_leash(id);
        let (entity_points, leasher_points) = if quad_connection { (SHARED_QUAD_ATTACHMENT_POINTS, SHARED_QUAD_ATTACHMENT_POINTS) } else { (ENTITY_ATTACHMENT_POINT, LEASHER_ATTACHMENT_POINT) };
        let wrenches = self.compute_elastic_interaction(id, leash_holder, entity_points, leasher_points);
        if wrenches.is_empty() {
            return false;
        }
        let result = Wrench::accumulate(&wrenches).scale(if quad_connection { 0.25 } else { 1.0 });
        if let Some(Some(data)) = self.leash_data_slot(id) {
            data.angular_momentum += 10.0 * result.torque;
        }
        let relative_velocity_to_leasher = self.get_holder_movement(leash_holder).subtract(self.known_movement(id));
        let push = result.force.multiply(AXIS_SPECIFIC_ELASTICITY.x, AXIS_SPECIFIC_ELASTICITY.y, AXIS_SPECIFIC_ELASTICITY.z).add_vec(relative_velocity_to_leasher.scale(0.11));
        self.entity_mut(id).add_delta_movement(push);
        true
    }

    fn compute_elastic_interaction(&self, id: EntityId, leash_holder: EntityId, entity_attachment_points: &[Vec3], leasher_attachment_points: &[Vec3]) -> Vec<Wrench> {
        let slack_distance = self.leash_elastic_distance(id);
        let current_movement = self.get_holder_movement(id);
        let e = self.entity(id);
        let h = self.entity(leash_holder);
        let entity_y_rot = e.y_rot * (std::f64::consts::PI / 180.0) as f32;
        let entity_dimensions = Vec3::new(e.dimensions.width as f64, e.dimensions.height as f64, e.dimensions.width as f64);
        let leash_holder_y_rot = h.y_rot * (std::f64::consts::PI / 180.0) as f32;
        let leasher_dimensions = Vec3::new(h.dimensions.width as f64, h.dimensions.height as f64, h.dimensions.width as f64);
        let mut wrenches = Vec::new();
        for i in 0..entity_attachment_points.len() {
            let entity_attach_vector = entity_attachment_points[i].multiply(entity_dimensions.x, entity_dimensions.y, entity_dimensions.z).y_rot(-entity_y_rot);
            let entity_attach_pos = e.position().add_vec(entity_attach_vector);
            let leasher_attach_vector = leasher_attachment_points[i].multiply(leasher_dimensions.x, leasher_dimensions.y, leasher_dimensions.z).y_rot(-leash_holder_y_rot);
            let leasher_attach_pos = h.position().add_vec(leasher_attach_vector);
            if let Some(wrench) = compute_dampened_spring_interaction(leasher_attach_pos, entity_attach_pos, slack_distance, current_movement, entity_attach_vector) {
                wrenches.push(wrench);
            }
        }
        wrenches
    }

    pub fn set_leashed_to(&mut self, id: EntityId, holder: EntityId, synch: bool) {
        if id != holder {
            self.set_leashed_to_internal(id, holder, synch);
        }
    }

    fn leash_holder_info(&self, holder: EntityId) -> LeashInfo {
        match self.entities.get(&holder) {
            Some(LevelEntity::LeashKnot(knot)) => LeashInfo::Pos(knot.entity.block_attached_pos()),
            _ => LeashInfo::Uuid(self.entity(holder).uuid),
        }
    }

    fn set_leashed_to_internal(&mut self, id: EntityId, holder: EntityId, synch: bool) {
        let info = self.leash_holder_info(holder);
        let old_holder = match self.leash_data_slot(id) {
            Some(slot @ None) => {
                *slot = Some(LeashData::holder(holder, info));
                None
            }
            Some(Some(data)) => {
                let old_holder = data.leash_holder;
                data.set_leash_holder(holder, info);
                old_holder
            }
            None => return,
        };
        if let Some(old_holder) = old_holder
            && old_holder != holder
        {
            self.notify_leashee_removed(old_holder);
        }
        if synch {
            self.send_to_tracking_players(id, &set_entity_link_packet(id, Some(holder)));
        }
        if self.is_passenger(id) {
            self.stop_riding(id);
        }
    }

    pub fn get_leash_holder(&self, id: EntityId) -> Option<EntityId> {
        self.get_leash_data(id).and_then(|data| data.leash_holder)
    }

    pub fn leashable_leashed_to(&self, entity: EntityId) -> Vec<EntityId> {
        let center = self.entity(entity).bounding_box().center();
        self.leashable_in_area(center, |level, leashable| level.get_leash_holder(leashable) == Some(entity))
    }

    pub fn leashable_in_area(&self, pos: Vec3, test: impl Fn(&ServerLevel, EntityId) -> bool) -> Vec<EntityId> {
        let scan_area = AABB::of_size(pos, 32.0, 32.0, 32.0);
        self.get_entities_matching(None, &scan_area, |level, e| level.is_leashable(e) && test(level, e))
    }

    fn notify_leashee_removed(&mut self, holder: EntityId) {
        if matches!(self.entities.get(&holder), Some(LevelEntity::LeashKnot(_))) && self.leashable_leashed_to(holder).is_empty() {
            self.discard(holder);
        }
    }

    pub fn shear_off_all_leash_connections(&mut self, id: EntityId, player: Option<EntityId>) -> bool {
        let dropped = self.drop_all_leash_connections(id);
        if dropped {
            let pos = self.entity(id).block_position();
            let source = match player {
                Some(player) => self.sound_source(player),
                None => self.sound_source(id),
            };
            let (x, y, z) = (pos.x as f64 + 0.5, pos.y as f64 + 0.5, pos.z as f64 + 0.5);
            self.play_sound_at(None, x, y, z, sound_event("minecraft:item.shears.snip"), source, 1.0, 1.0);
        }
        dropped
    }

    pub fn drop_all_leash_connections(&mut self, id: EntityId) -> bool {
        let leashables = self.leashable_leashed_to(id);
        let mut dropped = !leashables.is_empty();
        if self.is_leashable(id) && self.is_leashed(id) {
            self.drop_leash(id);
            dropped = true;
        }
        for leashable in leashables {
            self.drop_leash(leashable);
        }
        dropped
    }
}
