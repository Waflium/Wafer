use std::collections::HashSet;

use wafer_core::random::{LegacyRandomSource, RandomSource};
use wafer_core::{BlockPos, Vec3};
use wafer_data::{EquipmentSlot, blocks, damage_types, game_rule};
use wafer_protocol::nbt::Nbt;

use super::blaze::{self, Blaze};
use super::ghast::{self, Ghast};
use super::zombie::zombified_piglin::ZombifiedPiglin;
use crate::nbt::{java_double_to_string, java_float_to_string};
use crate::server::level::{ServerLevel, testing};
use crate::world::clock::ServerClockManager;
use crate::world::damagesource::DamageSource;
use crate::world::difficulty::Difficulty;
use crate::world::entity::animal::pig::pig::Pig;
use crate::world::entity::attributes::Attribute;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::projectile::projectile::ProjectileKind;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::level::BlockGetter;
use crate::world::level::block;
use crate::world::level::block::entity::BlockEntityKind;
use crate::world::level::block::entity::spawner_block_entity::SpawnerBlockEntity;
use crate::world::level::update_flags::UPDATE_ALL;

const ORACLE: &str = include_str!("nether_mob_oracle.txt");

fn section(label: &str) -> Vec<&'static str> {
    ORACLE.lines().skip_while(|line| *line != format!("== {label}")).skip(1).take_while(|line| !line.starts_with("==")).collect()
}

fn rows(label: &str) -> Vec<&'static str> {
    section(label).into_iter().filter(|line| !line.starts_with("spawned") && !line.starts_with("advancement") && !line.starts_with("nbt ")).collect()
}

fn parity(label: &str, tag: &str) -> i32 {
    let prefix = format!("spawned {tag} id=");
    let line = section(label).into_iter().find(|line| line.starts_with(&prefix)).expect("a spawned line");
    let id: i32 = line[prefix.len()..].split(' ').next().unwrap().parse().unwrap();
    id.rem_euclid(2)
}

fn d(v: f64) -> String {
    java_double_to_string(v)
}

fn f(v: f32) -> String {
    java_float_to_string(v)
}

fn segments(row: &str) -> Vec<&str> {
    row.split(" | ").map(str::trim).filter(|segment| !segment.is_empty()).collect()
}

fn type_path(level: &ServerLevel, id: EntityId) -> &'static str {
    wafer_data::registry::ENTITY_TYPE[level.entity(id).entity_type as usize].trim_start_matches("minecraft:")
}

fn night_level(player_pos: Vec3) -> (ServerLevel, EntityId) {
    let mut level = testing::level();
    let mut clocks = ServerClockManager::new();
    clocks.set_total_ticks(ServerClockManager::id_of("minecraft:overworld").unwrap(), 18000);
    level.sync_clocks(&clocks);
    level.difficulty = Difficulty::Normal;
    level.game_rules.set(game_rule::MOB_GRIEFING, 0);
    level.game_rules.set(game_rule::NATURAL_HEALTH_REGENERATION, 0);
    let player = testing::player(&mut level, player_pos, 0.0, 0.0);
    level.living_mut(player).attributes.instance(Attribute::MaxHealth).unwrap().set_base_value(1024.0);
    level.set_health(player, 1024.0);
    let mut scoreboard = level.scoreboard.borrow_mut();
    scoreboard.add_player_team("t");
    scoreboard.set_collision_rule("t", crate::world::scores::team::CollisionRule::Never);
    scoreboard.add_player_to_team_base("Tester", "t");
    drop(scoreboard);
    for _ in 0..61 {
        level.tick();
    }
    (level, player)
}

fn spawn(level: &mut ServerLevel, create: fn() -> Mob, parity: i32, pos: Vec3, seed: i64) -> EntityId {
    let mut mob = loop {
        let mob = create();
        if mob.entity.id.rem_euclid(2) == parity {
            break mob;
        }
    };
    mob.entity.snap_to(pos.x, pos.y, pos.z, 0.0, 0.0);
    mob.entity.y_head_rot = 0.0;
    mob.living.y_body_rot = 0.0;
    mob.persistence_required = true;
    let id = mob.entity.id;
    level.add_fresh_entity(LevelEntity::Mob(Box::new(mob)));
    level.mob_mut(id).unwrap().entity.random = LegacyRandomSource::new(seed);
    id
}

fn is_gone(level: &ServerLevel, id: EntityId) -> bool {
    level.entity_ref(id).is_none_or(|e| e.is_removed())
}

fn mob_row(level: &ServerLevel, tag: &str, id: EntityId) -> String {
    if is_gone(level, id) {
        return format!("{tag} gone");
    }
    let e = level.entity(id);
    let target = level.get_target(id).map_or("-", |target| type_path(level, target));
    let mut row = format!("{tag} {} {} {} {} {} {target}", d(e.x()), d(e.y()), d(e.z()), d(e.delta_movement.y), f(level.health(id)));
    match level.mob(id).map(|mob| &mob.kind) {
        Some(MobKind::Blaze(_)) => row += &format!(" {}", level.is_on_fire(id)),
        Some(MobKind::Ghast(_)) => row += &format!(" {} {}", ghast::is_charging(e), f(e.y_rot)),
        Some(MobKind::ZombifiedPiglin(_)) => {
            let end = level.get_persistent_anger_end_time(id);
            row += &format!(" {}", if end > 0 { end - level.game_time } else { -1 });
        }
        _ => {}
    }
    row
}

fn canonical(nbt: &Nbt, game_time: i64) -> Nbt {
    match nbt {
        Nbt::Compound(fields) => {
            let mut fields: Vec<(String, Nbt)> = fields
                .iter()
                .filter(|(key, _)| key != "Tags")
                .map(|(key, value)| match (key.as_str(), value) {
                    ("UUID" | "Owner" | "angry_at", _) => (key.clone(), Nbt::String("uuid".into())),
                    ("anger_end_time", Nbt::Long(end)) if *end > 0 => (key.clone(), Nbt::Long(end - game_time)),
                    _ => (key.clone(), canonical(value, game_time)),
                })
                .collect();
            fields.sort_by(|a, b| a.0.cmp(&b.0));
            Nbt::Compound(fields)
        }
        Nbt::List(items) => Nbt::List(items.iter().map(|item| canonical(item, game_time)).collect()),
        other => other.clone(),
    }
}

fn assert_same_nbt_at(level: &ServerLevel, id: EntityId, expected: &str, expected_game_time: i64, context: &str) {
    let expected = canonical(&crate::nbt::parse_fully(expected).expect("oracle snbt"), expected_game_time);
    let actual = canonical(&level.entity_tag_to_compare(id), level.game_time);
    assert_eq!(crate::nbt::to_string(&actual), crate::nbt::to_string(&expected), "{context}");
}

fn assert_same_nbt(level: &ServerLevel, id: EntityId, expected: &str, context: &str) {
    assert_same_nbt_at(level, id, expected, 0, context);
}

fn spawned_game_time(label: &str) -> i64 {
    let line = section(label).into_iter().find(|line| line.starts_with("spawned ")).expect("a spawned line");
    line.rsplit("gameTime=").next().unwrap().parse().unwrap()
}

struct Duel {
    level: ServerLevel,
    player: EntityId,
    tags: Vec<(&'static str, EntityId)>,
    projectiles: Vec<EntityId>,
    seen_fire: HashSet<BlockPos>,
    fire_box: Option<(BlockPos, BlockPos)>,
    deflect_range: Option<f32>,
    nbt_on_spawn: bool,
}

impl Duel {
    fn new(level: ServerLevel, player: EntityId) -> Duel {
        Duel { level, player, tags: Vec::new(), projectiles: Vec::new(), seen_fire: HashSet::new(), fire_box: None, deflect_range: None, nbt_on_spawn: false }
    }

    fn row(&mut self, expected: &str) -> Vec<String> {
        let mut out = Vec::new();
        for &(tag, id) in &self.tags {
            if tag == "player" {
                let p = self.level.entity(self.player);
                out.push(format!("player {} {} {} {} {}", f(self.level.health(self.player)), d(p.x()), d(p.y()), d(p.z()), d(p.delta_movement.y)));
            } else {
                out.push(mob_row(&self.level, tag, id));
            }
        }
        let mut ids: Vec<EntityId> = self
            .level
            .entities
            .iter()
            .filter(|(_, e)| matches!(e, LevelEntity::Projectile(p) if p.abstract_hurting_projectile().is_some() && !p.entity.is_removed()))
            .map(|(id, _)| *id)
            .collect();
        ids.sort();
        for id in ids {
            let first = !self.projectiles.contains(&id);
            if first {
                self.projectiles.push(id);
            }
            let seq = self.projectiles.iter().position(|&p| p == id).unwrap();
            let e = self.level.entity(id);
            let m = e.delta_movement;
            out.push(format!("f{seq} {} {} {} {} {} {}", d(e.x()), d(e.y()), d(e.z()), d(m.x), d(m.y), d(m.z)));
            if first && self.nbt_on_spawn {
                let expected_nbt = segments(expected).into_iter().find(|segment| segment.starts_with("nbt ")).expect("an nbt segment");
                assert_same_nbt(&self.level, id, &expected_nbt[4..], "fireball nbt");
                out.push(expected_nbt.to_string());
            }
        }
        if let Some((from, to)) = self.fire_box {
            for x in from.x..=to.x {
                for y in from.y..=to.y {
                    for z in from.z..=to.z {
                        let pos = BlockPos::new(x, y, z);
                        if self.level.block_state(pos).is_block(blocks::FIRE) && self.seen_fire.insert(pos) {
                            out.push(format!("fire {x} {y} {z}"));
                        }
                    }
                }
            }
        }
        if let Some(range) = self.deflect_range {
            let player = self.player;
            let fireball = self.level.entities.iter().find_map(|(&id, e)| match e {
                LevelEntity::Projectile(p) if matches!(p.kind, ProjectileKind::LargeFireball(_)) && !p.entity.is_removed() && (p.entity.distance_to_sqr(self.level.entity(player)).sqrt() as f32) < range => Some(id),
                _ => None,
            });
            if let Some(fireball) = fireball {
                let segment = segments(expected).into_iter().find(|segment| segment.starts_with("deflected")).unwrap_or("deflected ? ?");
                let fields: Vec<&str> = segment.split(' ').collect();
                let (y_rot, x_rot): (f32, f32) = (fields[1].parse().unwrap_or(0.0), fields[2].parse().unwrap_or(0.0));
                let p = self.level.entity_mut(player);
                p.set_y_rot(y_rot);
                p.set_x_rot(x_rot);
                self.level.player_attack(player, fireball);
                out.push(segment.to_string());
                self.deflect_range = None;
            }
        }
        out
    }

    fn play(&mut self, label: &str, rows: &[&str]) {
        for (tick, expected) in rows.iter().enumerate() {
            self.level.tick();
            self.level.tick_player(self.player);
            let actual = self.row(expected);
            let expected_segments = segments(expected);
            assert_eq!(actual, expected_segments, "{label} tick {}", tick + 1);
        }
    }
}

fn first_player_health(rows: &[&str]) -> f32 {
    let segment = segments(rows[0]).into_iter().find(|segment| segment.starts_with("player ")).expect("a player segment");
    segment[7..].split(' ').next().unwrap().parse().unwrap()
}

#[test]
fn nether_mobs_have_vanillas_attributes() {
    let blaze = Blaze::create();
    let attributes = &blaze.living.attributes;
    assert_eq!(attributes.value(Attribute::MaxHealth), 20.0);
    assert_eq!(attributes.value(Attribute::AttackDamage), 6.0);
    assert_eq!(attributes.value(Attribute::MovementSpeed), 0.23f32 as f64);
    assert_eq!(attributes.value(Attribute::FollowRange), 48.0);
    assert_eq!(blaze.xp_reward, 10);
    assert_eq!(blaze.entity.dimensions.eye_height, 1.8 * 0.85);
    let ghast = Ghast::create();
    let attributes = &ghast.living.attributes;
    assert_eq!(attributes.value(Attribute::MaxHealth), 10.0);
    assert_eq!(attributes.value(Attribute::FollowRange), 100.0);
    assert_eq!(attributes.value(Attribute::FlyingSpeed), 0.06);
    assert_eq!(attributes.value(Attribute::CameraDistance), 8.0);
    assert_eq!(ghast.xp_reward, 5);
    assert_eq!(ghast.entity.dimensions.eye_height, 2.6);
    let zombified_piglin = ZombifiedPiglin::create();
    let attributes = &zombified_piglin.living.attributes;
    assert_eq!(attributes.value(Attribute::MovementSpeed), 0.23f32 as f64);
    assert_eq!(attributes.value(Attribute::AttackDamage), 5.0);
    assert_eq!(attributes.value(Attribute::SpawnReinforcements), 0.0);
    assert_eq!(zombified_piglin.entity.dimensions.eye_height, 1.79);
    assert_eq!(zombified_piglin.ai.goal_selector.get_available_goals().len(), 6);
    assert_eq!(zombified_piglin.ai.target_selector.get_available_goals().len(), 3);
}

#[test]
fn blazes_shoot_volleys_of_small_fireballs_like_vanilla() {
    let rows = rows("blaze");
    let (mut level, player) = night_level(Vec3::new(0.5, -60.0, 0.5));
    level.set_health(player, first_player_health(&rows));
    let blaze = spawn(&mut level, Blaze::create, parity("blaze", "b"), Vec3::new(0.5, -60.0, 10.5), 42);
    let mut duel = Duel::new(level, player);
    duel.tags = vec![("b", blaze), ("player", player)];
    duel.fire_box = Some((BlockPos::new(-12, -62, -12), BlockPos::new(12, -50, 16)));
    assert_eq!(rows.len(), 240);
    duel.play("blaze", &rows);
}

fn fill(level: &mut ServerLevel, from: BlockPos, to: BlockPos, state: wafer_data::BlockState) {
    for x in from.x..=to.x {
        for y in from.y..=to.y {
            for z in from.z..=to.z {
                level.set_block(BlockPos::new(x, y, z), state, UPDATE_ALL);
            }
        }
    }
}

#[test]
fn blaze_fireballs_set_fires_where_they_land_like_vanilla() {
    let rows = rows("blazefire");
    let (mut level, player) = night_level(Vec3::new(0.5, -60.0, 0.5));
    level.set_health(player, first_player_health(&rows));
    fill(&mut level, BlockPos::new(0, -60, 12), BlockPos::new(0, -51, 12), blocks::STONE.default_state());
    level.game_rules.set(game_rule::MOB_GRIEFING, 1);
    let blaze = spawn(&mut level, Blaze::create, parity("blazefire", "b"), Vec3::new(0.5, -50.0, 12.5), 3);
    let mut duel = Duel::new(level, player);
    duel.tags = vec![("b", blaze), ("player", player)];
    duel.fire_box = Some((BlockPos::new(-14, -62, -14), BlockPos::new(14, -50, 20)));
    assert_eq!(rows.len(), 200);
    assert!(rows.iter().any(|row| row.contains("| fire ")));
    duel.play("blazefire", &rows);
}

#[test]
fn blazes_burn_players_in_melee_range_like_vanilla() {
    let rows = rows("blazemelee");
    let (mut level, player) = night_level(Vec3::new(0.5, -60.0, 0.5));
    level.set_health(player, first_player_health(&rows));
    let blaze = spawn(&mut level, Blaze::create, parity("blazemelee", "b"), Vec3::new(0.5, -60.0, 1.7), 9);
    let mut duel = Duel::new(level, player);
    duel.tags = vec![("b", blaze), ("player", player)];
    assert_eq!(rows.len(), 120);
    duel.play("blazemelee", &rows);
}

#[test]
fn blazes_rise_toward_targets_above_them_like_vanilla() {
    let rows = rows("blazehover");
    let (mut level, player) = night_level(Vec3::new(0.5, -60.0, 0.5));
    fill(&mut level, BlockPos::new(-1, -60, -1), BlockPos::new(1, -56, 1), blocks::STONE.default_state());
    level.entity_mut(player).set_pos(Vec3::new(0.5, -55.0, 0.5));
    level.entity_moved(player);
    level.set_health(player, first_player_health(&rows));
    let blaze = spawn(&mut level, Blaze::create, parity("blazehover", "b"), Vec3::new(3.5, -60.0, 8.5), 5);
    let mut duel = Duel::new(level, player);
    duel.tags = vec![("b", blaze), ("player", player)];
    duel.fire_box = Some((BlockPos::new(-14, -62, -14), BlockPos::new(14, -50, 20)));
    assert_eq!(rows.len(), 200);
    assert!(rows.iter().any(|row| !row.contains(" -60.0 8.5 ")));
    duel.play("blazehover", &rows);
}

#[test]
fn ghasts_charge_and_shoot_fireballs_like_vanilla() {
    let rows = rows("ghast");
    let (mut level, player) = night_level(Vec3::new(0.5, -60.0, 0.5));
    level.set_health(player, first_player_health(&rows));
    let ghast = spawn(&mut level, Ghast::create, parity("ghast", "g"), Vec3::new(0.5, -58.0, 24.5), 7);
    let mut duel = Duel::new(level, player);
    duel.tags = vec![("g", ghast), ("player", player)];
    duel.nbt_on_spawn = true;
    assert_eq!(rows.len(), 160);
    duel.play("ghast", &rows);
}

fn deflect(label: &str, pos: Vec3, seed: i64) {
    let rows = rows(label);
    let (mut level, player) = night_level(Vec3::new(0.5, -60.0, 0.5));
    level.set_health(player, first_player_health(&rows));
    let ghast = spawn(&mut level, Ghast::create, parity(label, "g"), pos, seed);
    let mut duel = Duel::new(level, player);
    duel.tags = vec![("g", ghast), ("player", player)];
    duel.deflect_range = Some(3.0);
    duel.play(label, &rows);
    assert!(duel.deflect_range.is_none(), "{label}: deflected");
    let killed = rows.last().unwrap().starts_with("g gone");
    assert_eq!(is_gone(&duel.level, ghast), killed);
}

#[test]
fn killing_a_ghast_with_a_returned_fireball_awards_return_to_sender() {
    use crate::network::login::{Profile, offline_uuid};
    use crate::server::server_advancement_manager::ServerAdvancementManager;
    use crate::world::entity::entity_reference::EntityReference;
    use crate::world::entity::projectile::hurtingprojectile::large_fireball::LargeFireball;
    use crate::world::phys::entity_hit_result::EntityHitResult;
    let mut server = testing::server();
    let uuid = offline_uuid("Alice");
    let profile = Profile { uuid, name: "Alice".into(), properties: Vec::new() };
    let player = testing::server_player_with_profile(&mut server, profile, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    server.levels[0].player_mut(player).advancements = server.get_player_advancements(uuid);
    let level = server.overworld_mut();
    level.difficulty = Difficulty::Normal;
    let ghast = spawn(level, Ghast::create, 0, Vec3::new(0.5, -55.0, 10.5), 1);
    let mut fireball = LargeFireball::from_mob(level, ghast, Vec3::new(0.0, 0.0, -1.0), 1);
    fireball.owner = Some(EntityReference::of(uuid));
    let fireball_id = fireball.entity.id;
    level.add_fresh_entity(LevelEntity::Projectile(Box::new(fireball)));
    let location = level.entity(ghast).position();
    level.large_fireball_on_hit_entity(fireball_id, EntityHitResult::new(ghast, location));
    assert!(level.is_dead_or_dying(ghast));
    let advancement = ServerAdvancementManager::get_bound().get("minecraft:nether/return_to_sender").expect("the advancement");
    assert!(server.levels[0].player_mut(player).advancements.get_or_start_progress(advancement).is_done());
}

#[test]
fn ghasts_ignore_their_own_fireballs_unless_a_player_returned_them() {
    use crate::world::entity::projectile::hurtingprojectile::large_fireball::LargeFireball;
    use crate::world::phys::entity_hit_result::EntityHitResult;
    let (mut level, _) = night_level(Vec3::new(0.5, -60.0, 0.5));
    let ghast = spawn(&mut level, Ghast::create, 0, Vec3::new(0.5, -55.0, 10.5), 1);
    let fireball = LargeFireball::from_mob(&level, ghast, Vec3::new(0.0, 0.0, -1.0), 1);
    let fireball_id = fireball.entity.id;
    level.add_fresh_entity(LevelEntity::Projectile(Box::new(fireball)));
    let location = level.entity(ghast).position();
    level.large_fireball_on_hit_entity(fireball_id, EntityHitResult::new(ghast, location));
    assert_eq!(level.health(ghast), 10.0);
}

#[test]
fn players_hit_ghast_fireballs_back_like_vanilla() {
    deflect("deflect", Vec3::new(0.5, -58.0, 24.5), 7);
}

#[test]
fn reflected_fireballs_kill_ghasts_like_vanilla() {
    assert!(rows("deflect2").last().unwrap().starts_with("g gone"));
    deflect("deflect2", Vec3::new(0.5, -58.0, 14.5), 11);
}

#[test]
fn zombified_piglins_alert_each_other_like_vanilla() {
    let rows = rows("anger");
    let (mut level, player) = night_level(Vec3::new(0.5, -60.0, 60.5));
    level.set_health(player, first_player_health(&rows));
    let a = spawn(&mut level, ZombifiedPiglin::create, parity("anger", "a"), Vec3::new(3.5, -60.0, 60.5), 1);
    let b = spawn(&mut level, ZombifiedPiglin::create, parity("anger", "b"), Vec3::new(8.5, -60.0, 64.5), 2);
    let c = spawn(&mut level, ZombifiedPiglin::create, parity("anger", "c"), Vec3::new(-6.5, -60.0, 57.5), 3);
    let far = spawn(&mut level, ZombifiedPiglin::create, parity("anger", "d"), Vec3::new(40.5, -60.0, 60.5), 4);
    let mut duel = Duel::new(level, player);
    duel.tags = vec![("a", a), ("b", b), ("c", c), ("d", far), ("player", player)];
    assert_eq!(rows.len(), 123);
    duel.play("anger", &rows[..3]);
    let source = DamageSource { causing_entity: Some(player), direct_entity: Some(player), ..DamageSource::new(damage_types::PLAYER_ATTACK) };
    duel.level.hurt_server(a, source, 1.0);
    duel.play("anger", &rows[3..]);
    for (tag, id) in [("a", a), ("d", far)] {
        let prefix = format!("nbt {tag} ");
        let expected = section("anger").into_iter().find(|line| line.starts_with(&prefix)).expect("an nbt line");
        assert_same_nbt_at(&duel.level, id, &expected[prefix.len()..], spawned_game_time("anger") + rows.len() as i64, tag);
    }
}

fn item_string(stack: Option<ItemStack>) -> String {
    match stack.filter(|stack| !stack.is_empty()) {
        Some(stack) => format!("{} {}", stack.count, stack.get_item().name()),
        None => "0 minecraft:air".into(),
    }
}

fn describe(level: &ServerLevel, id: EntityId) -> String {
    let e = level.entity(id);
    let mob = level.mob(id).expect("a mob");
    let vehicle = level.vehicle(id).map_or("-", |vehicle| type_path(level, vehicle));
    format!(
        "{} {} {} {} {} cooldown={} baby={} main={} head={} loot={} left={} persistent={} health={} follow={} vehicle={vehicle}",
        type_path(level, id),
        d(e.x()),
        d(e.y()),
        d(e.z()),
        f(e.y_rot),
        e.portal_cooldown,
        mob.is_baby(),
        item_string(level.item_by_slot(id, EquipmentSlot::Mainhand)),
        item_string(level.item_by_slot(id, EquipmentSlot::Head)),
        mob.can_pick_up_loot,
        mob.is_left_handed(),
        mob.persistence_required,
        f(mob.health),
        d(level.attribute_value(id, Attribute::FollowRange)),
    )
}

fn without_knockback_resistance(line: &str) -> String {
    let start = line.find(" kb=").expect("a knockback resistance");
    let end = line[start + 1..].find(' ').map_or(line.len(), |end| start + 1 + end);
    format!("{}{}", &line[..start], &line[end..])
}

fn new_mobs(level: &ServerLevel, before: &HashSet<EntityId>) -> Vec<EntityId> {
    let mut ids: Vec<EntityId> = level.entities.keys().copied().filter(|id| !before.contains(id) && level.is_mob(*id)).collect();
    ids.sort();
    ids
}

#[test]
fn nether_portals_spawn_zombified_piglins_like_vanilla() {
    let (mut level, _) = night_level(Vec3::new(0.5, -60.0, 60.5));
    let portal = BlockPos::new(20, -59, 100);
    level.set_block(portal.below(), blocks::OBSIDIAN.default_state(), UPDATE_ALL);
    level.set_block(portal, blocks::NETHER_PORTAL.default_state(), UPDATE_ALL);
    level.game_rules.set(game_rule::SPAWN_MOBS, 1);
    level.random = LegacyRandomSource::new(12345);
    let before: HashSet<EntityId> = level.entities.keys().copied().collect();
    for _ in 0..6000 {
        let state = level.block_state(portal);
        block::of(state).random_tick(state, &mut level, portal);
    }
    let actual: Vec<String> = new_mobs(&level, &before).into_iter().map(|id| format!("portal {}", describe(&level, id))).collect();
    let expected: Vec<String> = section("portal").into_iter().filter(|line| line.starts_with("portal ")).map(without_knockback_resistance).collect();
    assert_eq!(actual, expected);
    let random_line = section("portal").into_iter().find(|line| line.starts_with("random ")).unwrap();
    assert_eq!(format!("random {}", level.random.next_int()), random_line);
}

#[test]
fn pigs_struck_by_lightning_become_zombified_piglins_like_vanilla() {
    let (mut level, player) = night_level(Vec3::new(60.5, -60.0, 92.5));
    let expected: Vec<&str> = section("thunder").into_iter().filter(|line| line.starts_with("converted ")).collect();
    for (seed, pos, expected) in [(99, Vec3::new(40.5, -60.0, 100.5), expected[0]), (5, Vec3::new(44.5, -60.0, 100.5), expected[1])] {
        let pig = spawn(&mut level, Pig::create, 0, pos, seed);
        let before: HashSet<EntityId> = level.entities.keys().copied().collect();
        level.pig_thunder_hit(pig, player);
        let converted = new_mobs(&level, &before);
        assert_eq!(converted.len(), 1);
        let actual = format!("converted {}", describe(&level, converted[0]));
        let expected = without_knockback_resistance(expected);
        assert_eq!(actual, expected.split(" drop=").next().unwrap());
        assert!(level.mob(converted[0]).unwrap().drop_chances.iter().all(|&chance| chance == 0.085));
        assert!(is_gone(&level, pig));
    }
}

#[test]
fn spawners_spawn_blazes_like_vanilla() {
    let (mut level, _) = night_level(Vec3::new(60.5, -60.0, 92.5));
    let spawner = BlockPos::new(60, -60, 100);
    level.set_block(spawner, blocks::SPAWNER.default_state(), UPDATE_ALL);
    let BlockEntityKind::Spawner(entity) = &mut level.get_block_entity(spawner).expect("the spawner").kind else { panic!("not a spawner") };
    entity.load_additional(&crate::nbt::parse_fully("{SpawnData:{entity:{id:\"minecraft:blaze\"}}}").unwrap());
    level.random = LegacyRandomSource::new(777);
    let mut seen: HashSet<EntityId> = level.entities.keys().copied().collect();
    let mut actual = Vec::new();
    for i in 0..120 {
        SpawnerBlockEntity::server_tick(&mut level, spawner);
        for id in new_mobs(&level, &seen) {
            actual.push(format!("spawner {i} {}", describe(&level, id)));
            seen.insert(id);
        }
    }
    let expected: Vec<String> = section("spawner").into_iter().filter(|line| line.starts_with("spawner ")).map(without_knockback_resistance).collect();
    assert_eq!(actual, expected);
    let random_line = section("spawner").into_iter().find(|line| line.starts_with("random ")).unwrap();
    assert_eq!(format!("random {}", level.random.next_int()), random_line);
}

#[test]
fn blazes_and_ghasts_save_like_vanilla() {
    let (mut level, _) = night_level(Vec3::new(60.5, -60.0, 92.5));
    let blaze = spawn(&mut level, Blaze::create, 0, Vec3::new(80.5, -60.0, 100.5), 1);
    let ghast = spawn(&mut level, Ghast::create, 0, Vec3::new(90.5, -60.0, 100.5), 1);
    level.entity_mut(blaze).tags.push("nb".into());
    level.entity_mut(ghast).tags.push("ng".into());
    for (tag, id) in [("nb", blaze), ("ng", ghast)] {
        let prefix = format!("nbt {tag} ");
        let expected = section("nbt").into_iter().find(|line| line.starts_with(&prefix)).expect("an nbt line");
        assert_same_nbt(&level, id, &expected[prefix.len()..], tag);
    }
}

fn drops(create: fn() -> Mob, by_player: bool) -> Vec<wafer_data::Item> {
    let mut level = testing::level();
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 4.5), 0.0, 0.0);
    for seed in 0..20 {
        let id = spawn(&mut level, create, 0, Vec3::new(0.5, -60.0, 0.5), seed);
        let source = if by_player {
            DamageSource { causing_entity: Some(player), direct_entity: Some(player), ..DamageSource::new(damage_types::PLAYER_ATTACK) }
        } else {
            DamageSource::new(damage_types::GENERIC_KILL)
        };
        level.hurt_server(id, source, 1000.0);
    }
    level.entities.values().filter_map(|e| e.as_item()).map(|item| item.item.get_item()).collect()
}

#[test]
fn blazes_drop_rods_only_when_players_kill_them() {
    assert!(!drops(Blaze::create, false).contains(&wafer_data::items::BLAZE_ROD));
    assert!(drops(Blaze::create, true).contains(&wafer_data::items::BLAZE_ROD));
}

#[test]
fn ghasts_and_zombified_piglins_drop_their_loot() {
    let ghast = drops(Ghast::create, false);
    assert!(ghast.contains(&wafer_data::items::GHAST_TEAR));
    assert!(ghast.contains(&wafer_data::items::GUNPOWDER));
    assert!(!ghast.contains(&wafer_data::items::MUSIC_DISC_TEARS));
    let zombified_piglin = drops(ZombifiedPiglin::create, false);
    assert!(zombified_piglin.contains(&wafer_data::items::ROTTEN_FLESH));
    assert!(zombified_piglin.contains(&wafer_data::items::GOLD_NUGGET));
    assert!(!zombified_piglin.contains(&wafer_data::items::GOLD_INGOT));
}

#[test]
fn zombified_piglins_spawn_with_golden_swords_or_spears() {
    let mut level = testing::level();
    let mut spears = 0;
    for seed in 0..200 {
        let id = spawn(&mut level, ZombifiedPiglin::create, 0, Vec3::new(0.5, -60.0, 0.5), 1);
        let mut random = LegacyRandomSource::new(seed);
        level.zombified_piglin_populate_default_equipment_slots(id, &mut random);
        let item = level.item_by_slot(id, EquipmentSlot::Mainhand).unwrap().get_item();
        assert!(item == wafer_data::items::GOLDEN_SWORD || item == wafer_data::items::GOLDEN_SPEAR);
        spears += (item == wafer_data::items::GOLDEN_SPEAR) as i32;
        assert!(level.item_by_slot(id, EquipmentSlot::Head).is_none());
        level.entity_mut(id).discard();
    }
    assert!(spears > 0 && spears < 30);
}

#[test]
fn zombified_piglins_never_convert_in_water() {
    let (mut level, _) = night_level(Vec3::new(0.5, -60.0, 20.5));
    let id = spawn(&mut level, ZombifiedPiglin::create, 0, Vec3::new(0.5, -60.0, 0.5), 1);
    let zombie = spawn(&mut level, crate::world::entity::monster::zombie::zombie::Zombie::create, 0, Vec3::new(0.5, -60.0, 0.5), 1);
    for mob in [id, zombie] {
        level.add_effect(mob, crate::world::effect::MobEffectInstance::new(wafer_data::mob_effects::WATER_BREATHING, 10000, 0), None);
    }
    fill(&mut level, BlockPos::new(-2, -60, -2), BlockPos::new(2, -55, 2), blocks::GLASS.default_state());
    fill(&mut level, BlockPos::new(-1, -60, -1), BlockPos::new(1, -57, 1), blocks::WATER.default_state());
    for _ in 0..1000 {
        level.tick();
    }
    assert!(is_gone(&level, zombie));
    assert!(level.entities.values().any(|e| e.entity().entity_type == crate::world::entity::entity_types::DROWNED.id));
    assert!(!is_gone(&level, id));
    assert_eq!(type_path(&level, id), "zombified_piglin");
}

#[test]
fn ghasts_save_and_load_their_explosion_power() {
    let mut mob = Ghast::create();
    mob.load(&crate::nbt::parse_fully("{ExplosionPower:5b}").unwrap());
    let mut out = Vec::new();
    mob.save(&mut out);
    assert!(out.contains(&("ExplosionPower".to_string(), Nbt::Byte(5))));
    assert_eq!(Ghast::get_explosion_power(&mob), 5);
}

#[test]
fn blazes_are_charged_while_their_volley_lasts() {
    let mut mob = Blaze::create();
    assert!(!blaze::is_charged(&mob.entity));
    mob.entity.data.set(blaze::DATA_FLAGS_ID, crate::world::entity::data::DataValue::Byte(1));
    assert!(blaze::is_charged(&mob.entity));
    assert!(Blaze::is_on_fire(&mob.entity));
}
