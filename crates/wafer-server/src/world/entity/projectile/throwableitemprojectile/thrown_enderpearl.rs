use wafer_core::random::RandomSource;
use wafer_core::{BlockPos, Vec3, mth};
use wafer_data::{BlockState, blocks, damage_types, game_rule};
use wafer_protocol::nbt::Nbt;

use super::throwable_item_projectile;
use crate::server::Server;
use crate::server::level::ticket::TicketType;
use crate::server::level::{OtherLevelPlayer, ServerLevel, SoundSource};
use crate::world::damagesource::DamageSource;
use crate::world::difficulty::Difficulty;
use crate::world::entity::projectile::projectile::{Projectile, ProjectileKind};
use crate::world::entity::relative::relative;
use crate::world::entity::{EntityId, LevelEntity, RemovalReason, sound_event};
use crate::world::item::ItemStack;
use crate::world::level::portal::teleport_transition::{PostTeleportTransition, TeleportTransition};
use crate::world::phys::hit_result::HitResult;

const ENTITY_TYPE: &str = "minecraft:ender_pearl";
pub const ENDER_PEARL_TICKET_RADIUS: i32 = 2;

#[derive(Default)]
pub struct ThrownEnderpearl {
    ticket_timer: i64,
}

#[derive(Clone, Copy)]
pub enum Owner {
    Entity(EntityId),
    OtherLevelPlayer(OtherLevelPlayer),
}

pub struct PendingEnderPearlTeleport {
    player: EntityId,
    set_portal_cooldown: bool,
    teleport_pos: Vec3,
}

impl ThrownEnderpearl {
    pub fn of_type() -> Projectile {
        throwable_item_projectile::new(ENTITY_TYPE, ProjectileKind::ThrownEnderpearl(ThrownEnderpearl::default()))
    }

    pub fn from_owner(level: &mut ServerLevel, mob: EntityId, item_stack: &ItemStack) -> Projectile {
        let projectile = throwable_item_projectile::from_owner(level, ENTITY_TYPE, mob, item_stack, ProjectileKind::ThrownEnderpearl(ThrownEnderpearl::default()));
        if level.is_player(mob) {
            level.register_ender_pearl(mob, projectile.entity.id);
        }
        projectile
    }
}

fn thrown_enderpearl(projectile: &mut Projectile) -> &mut ThrownEnderpearl {
    match &mut projectile.kind {
        ProjectileKind::ThrownEnderpearl(pearl) => pearl,
        _ => unreachable!("not an ender pearl"),
    }
}

pub fn is_thrown_enderpearl(entity: &LevelEntity) -> bool {
    matches!(entity, LevelEntity::Projectile(projectile) if matches!(projectile.kind, ProjectileKind::ThrownEnderpearl(_)))
}

impl ServerLevel {
    pub fn is_thrown_enderpearl(&self, id: EntityId) -> bool {
        self.entities.get(&id).is_some_and(is_thrown_enderpearl)
    }

    pub fn other_level_player(&self, uuid: u128) -> Option<OtherLevelPlayer> {
        self.other_level_players.iter().find(|player| player.uuid == uuid).copied()
    }

    pub fn thrown_enderpearl_get_owner(&self, id: EntityId) -> Option<Owner> {
        let owner = self.projectile(id).owner?;
        match owner.get_entity(self) {
            Some(entity) => Some(Owner::Entity(entity)),
            None => self.other_level_player(owner.get_uuid()).map(Owner::OtherLevelPlayer),
        }
    }

    pub fn thrown_enderpearl_deregister_from_current_owner(&mut self, id: EntityId) {
        let Some(owner) = self.projectile(id).owner else { return };
        match owner.get_entity(self) {
            Some(player) if self.is_player(player) => self.deregister_ender_pearl(player, id),
            Some(_) => {}
            None => self.other_level_ender_pearls.push((owner.get_uuid(), id, false)),
        }
    }

    pub fn thrown_enderpearl_register_to_current_owner(&mut self, id: EntityId) {
        let Some(owner) = self.projectile(id).owner else { return };
        self.register_to_owner(owner.get_uuid(), owner.get_entity(self), id);
    }

    pub fn thrown_enderpearl_register_detached(&mut self, entity: &LevelEntity) {
        if let LevelEntity::Projectile(projectile) = entity
            && let Some(owner) = projectile.owner
        {
            self.register_to_owner(owner.get_uuid(), owner.get_entity(self), projectile.entity.id);
        }
    }

    fn register_to_owner(&mut self, uuid: u128, owner: Option<EntityId>, pearl: EntityId) {
        match owner {
            Some(player) if self.is_player(player) => self.register_ender_pearl(player, pearl),
            Some(_) => {}
            None => self.other_level_ender_pearls.push((uuid, pearl, true)),
        }
    }

    pub fn register_ender_pearl(&mut self, player: EntityId, pearl: EntityId) {
        let pearls = &mut self.player_mut(player).ender_pearls;
        if !pearls.contains(&pearl) {
            pearls.push(pearl);
        }
    }

    pub fn deregister_ender_pearl(&mut self, player: EntityId, pearl: EntityId) {
        self.player_mut(player).ender_pearls.retain(|&p| p != pearl);
    }

    fn register_and_update_ender_pearl_ticket(&mut self, player: Owner, pearl: EntityId) -> i64 {
        let chunk_pos = self.entity(pearl).chunk_position();
        match player {
            Owner::Entity(player) => self.register_ender_pearl(player, pearl),
            Owner::OtherLevelPlayer(player) => self.other_level_ender_pearls.push((player.uuid, pearl, true)),
        }
        self.reset_empty_time();
        self.place_ender_pearl_ticket(chunk_pos) - 1
    }

    pub fn place_ender_pearl_ticket(&mut self, chunk: wafer_core::ChunkPos) -> i64 {
        self.chunks.add_ticket_with_radius(TicketType::EnderPearl, chunk, ENDER_PEARL_TICKET_RADIUS);
        TicketType::EnderPearl.timeout()
    }

    fn find_owner_including_dead_player(&self, uuid: u128) -> Option<Owner> {
        if let Some(entity) = self.entity_manager.visible_entity_storage.get(uuid) {
            return Some(Owner::Entity(entity));
        }
        if let Some(player) = self.other_level_player(uuid) {
            return Some(Owner::OtherLevelPlayer(player));
        }
        self.players.values().find(|p| p.profile.uuid == uuid).map(|p| Owner::Entity(p.entity.id))
    }

    pub fn thrown_enderpearl_on_hit(&mut self, id: EntityId, hit_result: HitResult) {
        self.projectile_base_on_hit(id, hit_result);
        let random = &mut self.entity_mut(id).random;
        for _ in 0..32 {
            random.next_double();
            random.next_gaussian();
            random.next_gaussian();
        }
        if self.entity(id).is_removed() {
            return;
        }
        let owner = self.thrown_enderpearl_get_owner(id);
        match owner.filter(|&owner| self.is_allowed_to_teleport_owner(owner)) {
            Some(Owner::Entity(owner)) => {
                let teleport_pos = self.entity(id).old_position();
                if self.is_player(owner) {
                    if !self.player(owner).disconnected {
                        let e = self.entity(owner);
                        self.thrown_enderpearl_spawn_endermite(id, e.position(), e.y_rot, e.x_rot);
                        if self.is_on_portal_cooldown(id) {
                            self.set_portal_cooldown(owner);
                        }
                        let transition = TeleportTransition::with_relatives(self.index, teleport_pos, Vec3::ZERO, 0.0, 0.0, relative::ROTATION, PostTeleportTransition::DO_NOTHING);
                        self.server_player_teleport_same_dimension(owner, transition);
                        self.reset_fall_distance(owner);
                        self.living_mut(owner).reset_current_impulse_context();
                        self.hurt_server(owner, DamageSource::new(damage_types::ENDER_PEARL), 5.0);
                        self.thrown_enderpearl_play_sound(teleport_pos);
                    }
                } else {
                    let (y_rot, x_rot) = (self.entity(owner).y_rot, self.entity(owner).x_rot);
                    let transition = TeleportTransition::new(self.index, teleport_pos, Vec3::ZERO, y_rot, x_rot, PostTeleportTransition::DO_NOTHING);
                    self.teleport_same_dimension(owner, transition);
                    self.reset_fall_distance(owner);
                    if self.is_living(owner) {
                        self.living_mut(owner).reset_current_impulse_context();
                    }
                    self.thrown_enderpearl_play_sound(teleport_pos);
                }
                self.discard(id);
            }
            Some(Owner::OtherLevelPlayer(player)) => {
                let teleport_pos = self.entity(id).old_position();
                if player.accepting_messages {
                    self.thrown_enderpearl_spawn_endermite(id, player.position, player.y_rot, player.x_rot);
                    let set_portal_cooldown = self.is_on_portal_cooldown(id);
                    self.pending_ender_pearl_teleport = Some(PendingEnderPearlTeleport { player: player.id, set_portal_cooldown, teleport_pos });
                }
                self.discard(id);
            }
            None => self.discard(id),
        }
    }

    fn thrown_enderpearl_spawn_endermite(&mut self, id: EntityId, position: Vec3, y_rot: f32, x_rot: f32) {
        if self.entity_mut(id).random.next_float() < 0.05
            && self.is_spawning_monsters()
            && self.difficulty != Difficulty::Peaceful
            && let Some(mut endermite) = ServerLevel::create_entity("minecraft:endermite")
        {
            endermite.entity_mut().snap_to(position.x, position.y, position.z, y_rot, x_rot);
            self.add_fresh_entity(endermite);
        }
    }

    fn is_allowed_to_teleport_owner(&self, owner: Owner) -> bool {
        match owner {
            Owner::Entity(owner) if self.is_living(owner) => self.is_alive(owner) && !self.is_sleeping(owner),
            Owner::Entity(owner) => self.is_alive(owner),
            Owner::OtherLevelPlayer(player) => player.can_use_portal,
        }
    }

    pub fn thrown_enderpearl_tick(&mut self, id: EntityId) {
        let position = self.entity(id).position();
        let (chunk_x, chunk_z) = (mth::floor(position.x) >> 4, mth::floor(position.z) >> 4);
        let owner = self.projectile(id).owner.and_then(|owner| self.find_owner_including_dead_player(owner.get_uuid()));
        let owner_player = owner.filter(|&owner| match owner {
            Owner::Entity(owner) => self.is_player(owner),
            Owner::OtherLevelPlayer(_) => true,
        });
        let dead = match owner_player {
            Some(Owner::Entity(player)) => !self.is_alive(player) && !self.player(player).won_game,
            Some(Owner::OtherLevelPlayer(player)) => !player.alive && !player.won_game,
            None => false,
        };
        if dead && self.game_rules.get_bool(game_rule::ENDER_PEARLS_VANISH_ON_DEATH) {
            self.discard(id);
        } else {
            self.throwable_projectile_tick(id);
        }
        if self.is_alive(id) {
            let current_pos = BlockPos::containing_vec(self.entity(id).position());
            let pearl = thrown_enderpearl(self.projectile_mut(id));
            pearl.ticket_timer -= 1;
            let expired = pearl.ticket_timer <= 0;
            if (expired || chunk_x != current_pos.x >> 4 || chunk_z != current_pos.z >> 4)
                && let Some(player) = owner_player
            {
                let ticket_timer = self.register_and_update_ender_pearl_ticket(player, id);
                thrown_enderpearl(self.projectile_mut(id)).ticket_timer = ticket_timer;
            }
        }
    }

    fn thrown_enderpearl_play_sound(&mut self, position: Vec3) {
        self.play_sound_at(None, position.x, position.y, position.z, sound_event("minecraft:entity.player.teleport"), SoundSource::Players, 1.0, 1.0);
    }

    pub fn thrown_enderpearl_teleported(&mut self, id: EntityId) {
        let position = self.entity(id).position();
        self.place_portal_ticket(BlockPos::containing_vec(position));
    }

    pub fn thrown_enderpearl_can_teleport(&self, id: EntityId) -> Option<bool> {
        match self.thrown_enderpearl_get_owner(id)? {
            Owner::Entity(owner) => self.players.get(&owner).map(|player| player.seen_credits),
            Owner::OtherLevelPlayer(player) => Some(player.seen_credits),
        }
    }

    pub fn thrown_enderpearl_on_inside_block(&mut self, id: EntityId, state: BlockState) {
        if state.is_block(blocks::END_GATEWAY)
            && let Some(player) = self.projectile_get_owner(id).filter(|&owner| self.is_player(owner))
        {
            crate::advancements::triggers::criteria_triggers::ENTER_BLOCK.trigger(self, player, state);
        }
    }

    pub fn thrown_enderpearl_on_removal(&mut self, id: EntityId, reason: Option<RemovalReason>) {
        if reason != Some(RemovalReason::UnloadedWithPlayer) {
            self.thrown_enderpearl_deregister_from_current_owner(id);
        }
    }

    pub fn save_ender_pearls<'a>(&'a self, player: EntityId, other_level_pearl: &dyn Fn(EntityId) -> Option<&'a ServerLevel>) -> Option<Nbt> {
        let ender_pearls = &self.player(player).ender_pearls;
        if ender_pearls.is_empty() {
            return None;
        }
        let mut pearls_output = Vec::new();
        for &ender_pearl in ender_pearls {
            let level = if self.entities.contains_key(&ender_pearl) { Some(self) } else { other_level_pearl(ender_pearl) };
            match level.filter(|level| !level.entity(ender_pearl).is_removed()) {
                None => warn!("Trying to save removed ender pearl, skipping"),
                Some(level) => {
                    let Nbt::Compound(mut pearl_tag) = level.save_entity(ender_pearl).unwrap_or(Nbt::Compound(Vec::new())) else { unreachable!("entities save as compounds") };
                    pearl_tag.push(("ender_pearl_dimension".into(), Nbt::String(level.dimension.into())));
                    pearls_output.push(Nbt::Compound(pearl_tag));
                }
            }
        }
        Some(Nbt::List(pearls_output))
    }
}

impl Server {
    pub fn update_other_level_players(&mut self, index: usize) {
        let mut players = std::mem::take(&mut self.levels[index].other_level_players);
        players.clear();
        for (other, level) in self.levels.iter().enumerate() {
            if other != index {
                players.extend(level.players.keys().map(|&id| OtherLevelPlayer::of(level, id)));
            }
        }
        self.levels[index].other_level_players = players;
    }

    pub fn flush_other_level_ender_pearls(&mut self) {
        for index in 0..self.levels.len() {
            for (uuid, pearl, registered) in std::mem::take(&mut self.levels[index].other_level_ender_pearls) {
                let Some(level) = self.levels.iter_mut().find(|level| level.players.values().any(|p| p.entity.uuid == uuid)) else { continue };
                let player = level.players.values().find(|p| p.entity.uuid == uuid).expect("the owner").entity.id;
                if registered {
                    level.register_ender_pearl(player, pearl);
                } else {
                    level.deregister_ender_pearl(player, pearl);
                }
            }
        }
    }

    pub fn handle_pending_ender_pearl_teleport(&mut self, level: usize) {
        let Some(pending) = self.levels[level].pending_ender_pearl_teleport.take() else { return };
        let Some(owner_level) = self.level_of(pending.player) else { return };
        if pending.set_portal_cooldown {
            self.levels[owner_level].set_portal_cooldown(pending.player);
        }
        let transition = TeleportTransition::with_relatives(level, pending.teleport_pos, Vec3::ZERO, 0.0, 0.0, relative::ROTATION, PostTeleportTransition::DO_NOTHING);
        if let Some(new_owner) = self.server_player_teleport(pending.player, transition) {
            let new_level = self.entity_level_mut(new_owner);
            new_level.reset_fall_distance(new_owner);
            new_level.living_mut(new_owner).reset_current_impulse_context();
            new_level.hurt_server(new_owner, DamageSource::new(damage_types::ENDER_PEARL), 5.0);
        }
        self.levels[level].thrown_enderpearl_play_sound(pending.teleport_pos);
    }

    pub fn remove_ender_pearls(&mut self, player: EntityId) {
        for ender_pearl in self.player(player).ender_pearls.clone() {
            if let Some(level) = self.level_of(ender_pearl) {
                let level = &mut self.levels[level];
                level.set_removed(ender_pearl, RemovalReason::UnloadedWithPlayer);
                level.remove_entity(ender_pearl);
            }
        }
    }

    pub fn load_and_spawn_ender_pearls(&mut self, player_input: &Nbt) {
        for pearl_input in player_input.get("ender_pearls").map_or(&[][..], Nbt::as_list) {
            self.load_and_spawn_ender_pearl(pearl_input);
        }
    }

    fn load_and_spawn_ender_pearl(&mut self, pearl_input: &Nbt) {
        let Some(pearl_level_key) = pearl_input.get("ender_pearl_dimension").and_then(Nbt::as_str).and_then(crate::commands::arguments::parse_identifier) else { return };
        let Some(pearl_level) = self.get_level(&pearl_level_key) else {
            warn!("Trying to load ender pearl without level (ResourceKey[minecraft:dimension / {pearl_level_key}]) being loaded, skipping");
            return;
        };
        let level = &mut self.levels[pearl_level];
        let mut loaded = level.load_entity_recursive(pearl_input, &mut |_| {}).into_iter();
        let pearl = loaded.next().and_then(|pearl| {
            let chunk_pos = pearl.entity().chunk_position();
            level.add_entity(pearl).then_some(chunk_pos)
        });
        match pearl {
            Some(chunk_pos) => {
                for passenger in loaded {
                    level.add_entity(passenger);
                }
                level.place_ender_pearl_ticket(chunk_pos);
            }
            None => warn!("Failed to spawn player ender pearl in level (ResourceKey[minecraft:dimension / {pearl_level_key}]), skipping"),
        }
    }
}
