use std::sync::{LazyLock, Mutex};

use wafer_core::random::{LegacyRandomSource, RandomSource};
use wafer_core::{AABB, Vec3, mth};
use wafer_data::registry;

use super::super::entity_argument::{ERROR_NOT_SINGLE_ENTITY, ERROR_SELECTORS_NOT_ALLOWED, NO_ENTITIES_FOUND, NO_PLAYERS_FOUND};
use crate::commands::arguments::nbt_path_argument::compare_nbt;
use crate::advancements::predicates::min_max_bounds::{Doubles, Ints};
use crate::commands::brigadier::CommandSyntaxException;
use crate::commands::source::CommandSourceStack;
use crate::network::chat::{Component, HoverEvent};
use super::entity_world::EntityWorld;
use crate::server::level::ServerLevel;
use crate::server::level::game_mode::GameType;
use crate::world::entity::{Entity, EntityId};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Order {
    Arbitrary,
    Nearest,
    Furthest,
    Random,
}

static SHUFFLE_RANDOM: LazyLock<Mutex<LegacyRandomSource>> = LazyLock::new(|| Mutex::new(LegacyRandomSource::create()));

impl Order {
    fn apply(self, world: &dyn EntityWorld, pos: Vec3, entities: &mut [EntityId]) {
        let distance = |id: &EntityId| entity(world, *id).position().distance_to_sqr(pos);
        match self {
            Order::Arbitrary => {}
            Order::Nearest => entities.sort_by(|a, b| distance(a).total_cmp(&distance(b))),
            Order::Furthest => entities.sort_by(|a, b| distance(b).total_cmp(&distance(a))),
            Order::Random => {
                let mut random = SHUFFLE_RANDOM.lock().unwrap();
                for i in (2..=entities.len()).rev() {
                    let j = random.next_int_bounded(i as i32) as usize;
                    entities.swap(i - 1, j);
                }
            }
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum EntityPredicate {
    Alive,
    Name { name: String, inverted: bool },
    GameMode { mode: GameType, inverted: bool },
    Team { name: String, inverted: bool },
    Type { entity_type: i32, inverted: bool },
    TypeTag { tag: String, inverted: bool },
    Tag { tag: String, inverted: bool },
    Nbt { tag: wafer_protocol::nbt::Nbt, inverted: bool },
    Scores(Vec<(String, Ints)>),
    Rotation { min: f32, max: f32, x: bool },
    Level(Ints),
    Advancements(Vec<(String, AdvancementSelectorPredicate)>),
}

#[derive(Clone, PartialEq, Debug)]
pub enum AdvancementSelectorPredicate {
    Done(bool),
    Criteria(Vec<(String, bool)>),
}

impl AdvancementSelectorPredicate {
    fn test(&self, progress: &crate::advancements::advancement_progress::AdvancementProgress) -> bool {
        match self {
            AdvancementSelectorPredicate::Done(value) => progress.is_done() == *value,
            AdvancementSelectorPredicate::Criteria(criteria) => {
                criteria.iter().all(|(name, value)| progress.get_criterion(name).is_some_and(|criterion| criterion.is_done() == *value))
            }
        }
    }
}

impl EntityPredicate {
    fn test(&self, world: &dyn EntityWorld, level: &ServerLevel, id: EntityId) -> bool {
        let e = level.entity(id);
        match self {
            EntityPredicate::Alive => level.is_alive(id),
            EntityPredicate::Name { name, inverted } => (world.entity_name(id) == *name) != *inverted,
            EntityPredicate::GameMode { mode, inverted } => match level.players.get(&id) {
                Some(player) => (player.game_mode.game_type == *mode) ^ *inverted,
                None => false,
            },
            EntityPredicate::Team { name, inverted } => (level.team_of(id).as_deref().map_or("", |t| t.name()) == name) != *inverted,
            EntityPredicate::Type { entity_type, inverted } => (e.entity_type == *entity_type) != *inverted,
            EntityPredicate::TypeTag { tag, inverted } => wafer_data::tag("minecraft:entity_type", tag).contains(&e.entity_type) != *inverted,
            EntityPredicate::Tag { tag, inverted } => {
                if tag.is_empty() { e.tags.is_empty() != *inverted } else { e.tags.contains(tag) != *inverted }
            }
            EntityPredicate::Nbt { tag, inverted } => compare_nbt(tag, Some(&level.entity_tag_to_compare(id))) != *inverted,
            EntityPredicate::Scores(expected) => {
                let scoreboard = world.scoreboard().borrow();
                let holder = level.scoreboard_name(id);
                expected.iter().all(|(objective, range)| {
                    scoreboard
                        .get_objective(objective)
                        .and_then(|o| scoreboard.get_player_score_info(&holder, o.id()))
                        .is_some_and(|score| range.matches(score.value))
                })
            }
            &EntityPredicate::Rotation { min, max, x } => {
                let rotation = mth::wrap_degrees(if x { e.x_rot } else { e.y_rot });
                if min > max { rotation >= min || rotation <= max } else { rotation >= min && rotation <= max }
            }
            EntityPredicate::Level(range) => level.players.get(&id).is_some_and(|p| range.matches(p.experience.level)),
            EntityPredicate::Advancements(expected) => {
                let Some(player) = level.players.get(&id) else { return false };
                let server_advancements = crate::server::server_advancement_manager::ServerAdvancementManager::get_bound();
                expected.iter().all(|(name, predicate)| {
                    server_advancements.get(name).is_some_and(|advancement| predicate.test(&player.advancements.get_progress(advancement)))
                })
            }
        }
    }
}

fn entity(world: &dyn EntityWorld, id: EntityId) -> &Entity {
    world.level_of_entity(id).expect("entity in a loaded level").entity(id)
}

#[derive(Clone, PartialEq, Debug)]
pub struct EntitySelector {
    pub max_results: i32,
    pub includes_entities: bool,
    pub world_limited: bool,
    pub predicates: Vec<EntityPredicate>,
    pub range: Option<Doubles>,
    pub position: (Option<f64>, Option<f64>, Option<f64>),
    pub aabb: Option<AABB>,
    pub order: Order,
    pub current_entity: bool,
    pub player_name: Option<String>,
    pub entity_uuid: Option<u128>,
    pub entity_type: Option<i32>,
    pub uses_selector: bool,
    pub unsupported: Option<&'static str>,
}

impl EntitySelector {
    pub fn is_self_selector(&self) -> bool {
        self.current_entity
    }

    fn check_permissions(&self, source: &CommandSourceStack) -> Result<(), CommandSyntaxException> {
        if self.uses_selector && !source.has_permission(2) {
            return Err(ERROR_SELECTORS_NOT_ALLOWED.create());
        }
        if let Some(option) = self.unsupported {
            return Err(CommandSyntaxException::new(Component::translatable("command.failed").with_hover(HoverEvent::ShowText(Box::new(
                Component::literal(format!("The {option} selector option is not implemented in Wafer yet")),
            )))));
        }
        Ok(())
    }

    pub fn find_single_entity(&self, world: &dyn EntityWorld, source: &CommandSourceStack) -> Result<EntityId, CommandSyntaxException> {
        self.check_permissions(source)?;
        let entities = self.find_entities(world, source)?;
        match entities[..] {
            [] => Err(NO_ENTITIES_FOUND.create()),
            [entity] => Ok(entity),
            _ => Err(ERROR_NOT_SINGLE_ENTITY.create()),
        }
    }

    pub fn find_entities(&self, world: &dyn EntityWorld, source: &CommandSourceStack) -> Result<Vec<EntityId>, CommandSyntaxException> {
        self.check_permissions(source)?;
        if !self.includes_entities {
            return self.find_players(world, source);
        }
        if let Some(name) = &self.player_name {
            return Ok(player_by_name(world, name).into_iter().collect());
        }
        if let Some(uuid) = self.entity_uuid {
            for level in world.levels() {
                if let Some(found) = level.all_entities().find(|&id| level.entity(id).uuid == uuid) {
                    return Ok(vec![found]);
                }
            }
            return Ok(Vec::new());
        }
        let pos = self.apply_position(source.position);
        let absolute_aabb = self.absolute_aabb(pos);
        if self.current_entity {
            return Ok(source
                .entity
                .filter(|&e| world.level_of_entity(e).is_some_and(|level| self.test(world, level, e, pos, absolute_aabb)))
                .into_iter()
                .collect());
        }
        let mut result = Vec::new();
        if self.world_limited {
            self.add_entities(world, world.level(source.level), &mut result, absolute_aabb, pos);
        } else {
            for level in world.levels() {
                self.add_entities(world, level, &mut result, absolute_aabb, pos);
            }
        }
        Ok(self.sort_and_limit(world, pos, result))
    }

    fn add_entities(&self, world: &dyn EntityWorld, level: &ServerLevel, result: &mut Vec<EntityId>, aabb: Option<AABB>, pos: Vec3) {
        let limit = self.result_limit();
        if result.len() >= limit {
            return;
        }
        let of_type = |id: &EntityId| self.entity_type.is_none_or(|t| level.entity(*id).entity_type == t);
        let candidates: Vec<EntityId> = match aabb {
            None => level.all_entities().filter(of_type).collect(),
            Some(bb) => {
                let mut found = Vec::new();
                level.entity_manager.section_storage.for_each_accessible_non_empty_section(&bb, |section| {
                    found.extend(section.entities().iter().copied().filter(|id| of_type(id) && level.entity(*id).bounding_box().intersects(&bb)));
                });
                found
            }
        };
        for id in candidates {
            if self.test(world, level, id, pos, aabb) {
                result.push(id);
                if result.len() >= limit {
                    return;
                }
            }
        }
    }

    fn result_limit(&self) -> usize {
        if self.order == Order::Arbitrary { self.max_results as usize } else { i32::MAX as usize }
    }

    pub fn find_single_player(&self, world: &dyn EntityWorld, source: &CommandSourceStack) -> Result<EntityId, CommandSyntaxException> {
        self.check_permissions(source)?;
        let players = self.find_players(world, source)?;
        match players[..] {
            [player] => Ok(player),
            _ => Err(NO_PLAYERS_FOUND.create()),
        }
    }

    pub fn find_players(&self, world: &dyn EntityWorld, source: &CommandSourceStack) -> Result<Vec<EntityId>, CommandSyntaxException> {
        self.check_permissions(source)?;
        if let Some(name) = &self.player_name {
            return Ok(player_by_name(world, name).into_iter().collect());
        }
        if let Some(uuid) = self.entity_uuid {
            return Ok(world.players().find(|p| p.profile.uuid == uuid).map(|p| p.entity.id).into_iter().collect());
        }
        let pos = self.apply_position(source.position);
        let absolute_aabb = self.absolute_aabb(pos);
        if self.current_entity {
            return Ok(source
                .entity
                .filter(|&e| world.has_player(e))
                .filter(|&p| world.level_of_entity(p).is_some_and(|level| self.test(world, level, p, pos, absolute_aabb)))
                .into_iter()
                .collect());
        }
        let limit = self.result_limit();
        let mut result = Vec::new();
        if self.world_limited {
            let level = world.level(source.level);
            for &id in level.players.keys() {
                if self.test(world, level, id, pos, absolute_aabb) {
                    result.push(id);
                    if result.len() >= limit {
                        break;
                    }
                }
            }
        } else {
            for id in world.players().map(|p| p.entity.id) {
                if world.level_of_entity(id).is_some_and(|level| self.test(world, level, id, pos, absolute_aabb)) {
                    result.push(id);
                    if result.len() >= limit {
                        return Ok(result);
                    }
                }
            }
        }
        Ok(self.sort_and_limit(world, pos, result))
    }

    fn apply_position(&self, origin: Vec3) -> Vec3 {
        let (x, y, z) = self.position;
        Vec3::new(x.unwrap_or(origin.x), y.unwrap_or(origin.y), z.unwrap_or(origin.z))
    }

    fn absolute_aabb(&self, pos: Vec3) -> Option<AABB> {
        self.aabb.map(|bb| bb.move_vec(pos))
    }

    fn test(&self, world: &dyn EntityWorld, level: &ServerLevel, id: EntityId, pos: Vec3, aabb: Option<AABB>) -> bool {
        self.predicates.iter().all(|p| p.test(world, level, id))
            && aabb.is_none_or(|bb| bb.intersects(&level.entity(id).bounding_box()))
            && self.range.is_none_or(|range| range.matches_sqr(level.entity(id).position().distance_to_sqr(pos)))
    }

    fn sort_and_limit(&self, world: &dyn EntityWorld, pos: Vec3, mut result: Vec<EntityId>) -> Vec<EntityId> {
        if result.len() > 1 {
            self.order.apply(world, pos, &mut result);
        }
        result.truncate(self.max_results as usize);
        result
    }
}

pub fn player_by_name(world: &dyn EntityWorld, name: &str) -> Option<EntityId> {
    world.players().find(|p| java_equals_ignore_case(&p.profile.name, name)).map(|p| p.entity.id)
}

pub fn java_equals_ignore_case(a: &str, b: &str) -> bool {
    let (a, b): (Vec<u16>, Vec<u16>) = (a.encode_utf16().collect(), b.encode_utf16().collect());
    let single = |c: u16, upper: bool| {
        let Some(ch) = char::from_u32(c as u32) else { return c as u32 };
        let mut mapped = if upper { ch.to_uppercase().collect::<Vec<_>>() } else { ch.to_lowercase().collect::<Vec<_>>() };
        if mapped.len() == 1 && (mapped[0] as u32) < 0x10000 { mapped.pop().unwrap() as u32 } else { c as u32 }
    };
    a.len() == b.len()
        && a.iter().zip(&b).all(|(&x, &y)| {
            x == y || {
                let (ux, uy) = (single(x, true), single(y, true));
                ux == uy || single(ux as u16, false) == single(uy as u16, false)
            }
        })
}

pub fn join_names(world: &dyn EntityWorld, entities: &[EntityId]) -> Component {
    crate::server::commands::list_players_command::format_list(entities.iter().map(|&e| world.entity_display_name(e)).collect())
}

pub fn player_type() -> i32 {
    wafer_data::registry_id(registry::ENTITY_TYPE, "minecraft:player")
}
