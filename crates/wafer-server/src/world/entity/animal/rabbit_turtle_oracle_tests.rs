use std::collections::HashMap;

use wafer_core::random::LegacyRandomSource;
use wafer_core::{BlockPos, Vec3};
use wafer_data::{BlockState, Item, damage_types, game_rule, registry};
use wafer_protocol::nbt::Nbt;

use super::rabbit::rabbit::{self, rabbit};
use super::turtle::turtle::{self, turtle};
use crate::server::level::{ServerLevel, testing};
use crate::world::clock::ServerClockManager;
use crate::world::damagesource::DamageSource;
use crate::world::difficulty::Difficulty;
use crate::world::entity::mob::MobKind;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::item::component::custom::parse_snbt;
use crate::world::level::{BlockGetter, block};
use crate::world::level::levelgen::structure::templatesystem::structure_processor::parse_block_state_string;
use crate::world::level::update_flags::UPDATE_CLIENTS;
use crate::world::{InteractionHand, InteractionResult, SwingSource};

fn oracle<'a>(text: &'a str, label: &str) -> Vec<&'a str> {
    text.lines().skip_while(|line| *line != format!("== {label}")).skip(1).take_while(|line| !line.starts_with("==") && *line != "done").collect()
}

fn row(level: &ServerLevel, id: EntityId) -> String {
    let Some(mob) = level.mob(id).filter(|mob| !mob.entity.is_removed()) else { return "gone".into() };
    let e = &mob.entity;
    let mut row = format!("{:?} {:?} {:?} {:?} {:?} {} health={:?}", e.x(), e.y(), e.z(), e.y_rot, e.delta_movement.y, e.on_ground, mob.health);
    let love = mob.animal().is_some_and(|animal| animal.in_love > 0);
    let age = mob.ageable().map_or(0, |ageable| ageable.age);
    match &mob.kind {
        MobKind::Rabbit(_) => {
            let r = rabbit(mob);
            row += &format!(
                " jumping={} jumpTicks={} jumpDuration={} delay={} carrots={} variant={} love={love} age={age}",
                mob.living.jumping,
                r.jump_ticks,
                r.jump_duration,
                r.jump_delay_ticks,
                r.more_carrot_ticks,
                rabbit::get_variant(e).get_serialized_name()
            );
        }
        MobKind::Turtle(_) => {
            let t = turtle(mob);
            let travel = t.travel_pos.map_or("-".to_string(), |pos| format!("{},{},{}", pos.x, pos.y, pos.z));
            row += &format!(
                " egg={} laying={} counter={} home={},{},{} travel={travel} going={} love={love} age={age}",
                turtle::has_egg(e),
                turtle::is_laying_egg(e),
                t.lay_egg_counter,
                t.home_pos.x,
                t.home_pos.y,
                t.home_pos.z,
                t.going_home
            );
        }
        _ => {}
    }
    row
}

fn result_name(result: InteractionResult) -> String {
    match result {
        InteractionResult::Success { swing_source, was_item_interaction } => {
            let swing = match swing_source {
                SwingSource::None => "NONE",
                SwingSource::Predicted => "PREDICTED",
                SwingSource::ServerOnly => "SERVER_ONLY",
            };
            format!("Success[swingSource={swing}, itemContext=ItemContext[wasItemInteraction={was_item_interaction}, heldItemTransformedTo=null]]")
        }
        InteractionResult::Pass => "Pass[]".into(),
        InteractionResult::Fail => "Fail[]".into(),
        InteractionResult::TryWithEmptyHand => "TryEmptyHandInteraction[]".into(),
    }
}

fn set_time(level: &mut ServerLevel, ticks: i64) {
    let mut clocks = ServerClockManager::new();
    clocks.set_total_ticks(ServerClockManager::id_of("minecraft:overworld").unwrap(), ticks);
    level.sync_clocks(&clocks);
}

fn state_string(state: BlockState) -> String {
    let mut properties: Vec<(&str, &str)> = state.properties().collect();
    properties.sort();
    let name = state.block().name();
    if properties.is_empty() {
        format!("Block{{{name}}}")
    } else {
        format!("Block{{{name}}}[{}]", properties.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(","))
    }
}

fn same(actual: &str, expected: &str) -> bool {
    let (actual, expected): (Vec<&str>, Vec<&str>) = (actual.split(' ').collect(), expected.split(' ').collect());
    actual.len() == expected.len()
        && actual.iter().zip(&expected).all(|(a, e)| {
            let number = |token: &str| token.rsplit('=').next().and_then(|value| value.parse::<f64>().ok());
            a == e || matches!((number(a), number(e)), (Some(a), Some(e)) if a == e || a as f32 == e as f32)
        })
}

struct Replay {
    oracle: &'static str,
    level: Option<ServerLevel>,
    player: EntityId,
    player_pos: Vec3,
    held: ItemStack,
    tags: HashMap<String, EntityId>,
    label: String,
    expected: Vec<&'static str>,
    parities: Vec<i32>,
    line: usize,
    nbt: Vec<(String, Nbt)>,
}

impl Replay {
    fn new(oracle: &'static str) -> Replay {
        Replay {
            oracle,
            level: None,
            player: 0,
            player_pos: Vec3::new(0.5, -60.0, 0.5),
            held: ItemStack::empty(),
            tags: HashMap::new(),
            label: String::new(),
            expected: Vec::new(),
            parities: Vec::new(),
            line: 0,
            nbt: Vec::new(),
        }
    }

    fn level(&mut self) -> &mut ServerLevel {
        if self.level.is_none() {
            let mut level = testing::level();
            level.difficulty = Difficulty::Normal;
            level.game_rules.set(game_rule::RANDOM_TICK_SPEED, 0);
            level.biome_zoom_seed = crate::world::level::biome::obfuscate_seed(0);
            set_time(&mut level, 1000);
            let player = testing::player(&mut level, self.player_pos, 0.0, 0.0);
            for _ in 0..61 {
                level.tick();
            }
            level.player_mut(player).set_item_in_hand(InteractionHand::MainHand, Some(self.held.clone()));
            self.player = player;
            self.level = Some(level);
        }
        self.level.as_mut().unwrap()
    }

    fn start(&mut self, label: &str) {
        self.level = None;
        self.tags.clear();
        self.label = label.into();
        self.expected = oracle(self.oracle, label);
        assert!(!self.expected.is_empty(), "no oracle rows for {label}");
        self.parities = self.expected.iter().filter(|line| line.starts_with("summoned ")).map(|line| line.split(' ').nth(2).unwrap().trim_start_matches("id=").parse::<i32>().unwrap() % 2).collect();
        self.expected.retain(|line| !line.starts_with("summoned "));
        self.line = 0;
    }

    fn check(&mut self, actual: String) {
        let expected = self.expected.get(self.line).unwrap_or_else(|| panic!("{}: more rows than the oracle", self.label));
        assert!(same(&actual, expected), "{} row {}:\n  wafer   {actual}\n  vanilla {expected}", self.label, self.line + 1);
        self.line += 1;
    }

    fn check_set(&mut self, prefix: &str, mut actual: Vec<String>) {
        let mut expected: Vec<&str> = self.expected[self.line..].iter().take(actual.len()).take_while(|line| line.starts_with(prefix)).copied().collect();
        self.line += expected.len();
        actual.sort();
        expected.sort();
        assert_eq!(actual.len(), expected.len(), "{}: {prefix} rows {actual:?} vs {expected:?}", self.label);
        for (a, e) in actual.iter().zip(&expected) {
            assert!(same(a, e), "{}:\n  wafer   {a}\n  vanilla {e}\nwafer:\n{}\nvanilla:\n{}", self.label, actual.join("\n"), expected.join("\n"));
        }
    }

    fn fill(&mut self, from: BlockPos, to: BlockPos, state: BlockState) {
        let level = self.level();
        for x in from.x..=to.x {
            for y in from.y..=to.y {
                for z in from.z..=to.z {
                    level.set_block(BlockPos::new(x, y, z), state, UPDATE_CLIENTS);
                }
            }
        }
    }

    fn finish(&self) {
        assert_eq!(self.line, self.expected.len(), "{}: fewer rows than the oracle", self.label);
    }

    fn run(&mut self, line: &str) {
        let words: Vec<&str> = line.split(' ').collect();
        let int = |i: usize| words[i].parse::<i32>().unwrap();
        match words[0] {
            "label" => {
                if !self.label.is_empty() {
                    self.finish();
                }
                self.start(words[1]);
            }
            "tp" => self.player_pos = Vec3::new(words[1].parse().unwrap(), words[2].parse().unwrap(), words[3].parse().unwrap()),
            "hold" => {
                self.held = ItemStack::new(Item::by_name(words[1]).unwrap(), words[2].parse().unwrap());
                let (held, player) = (self.held.clone(), self.player);
                if let Some(level) = self.level.as_mut() {
                    level.player_mut(player).set_item_in_hand(InteractionHand::MainHand, Some(held));
                }
            }
            "setblock" => {
                let pos = BlockPos::new(int(1), int(2), int(3));
                self.fill(pos, pos, parse_block_state_string(words[4]).unwrap());
            }
            "fill" => self.fill(BlockPos::new(int(1), int(2), int(3)), BlockPos::new(int(4), int(5), int(6)), parse_block_state_string(words[7]).unwrap()),
            "time" => set_time(self.level(), words[1].parse().unwrap()),
            "seedlevel" => self.level().random = LegacyRandomSource::new(words[1].parse().unwrap()),
            "summon" => {
                let parity = self.parities.remove(0);
                let (x, y, z): (f64, f64, f64) = (words[3].parse().unwrap(), words[4].parse().unwrap(), words[5].parse().unwrap());
                let seed: i64 = words[6].parse().unwrap();
                let extra = words.get(7).map_or(String::new(), |nbt| format!(",{nbt}"));
                let nbt = parse_snbt(&format!("{{id:\"{}\",PersistenceRequired:1b,Rotation:[0f,0f]{extra}}}", words[2])).unwrap();
                let level = self.level();
                let mut entity = loop {
                    let entity = level.load_entity(&nbt).unwrap();
                    if entity.entity().id.rem_euclid(2) == parity {
                        break entity;
                    }
                };
                let LevelEntity::Mob(mob) = &mut entity else { unreachable!() };
                mob.entity.snap_to(x, y, z, 0.0, 0.0);
                mob.entity.random = LegacyRandomSource::new(seed);
                mob.entity.y_head_rot = 0.0;
                mob.living.y_body_rot = 0.0;
                let id = mob.entity.id;
                level.add_fresh_entity(entity);
                self.tags.insert(words[1].into(), id);
            }
            "interact" => {
                let (id, player) = (self.tags[words[1]], self.player);
                let level = self.level();
                let location = level.entity(id).position();
                let result = level.interact_on(player, id, InteractionHand::MainHand, location);
                let held = level.player(player).item_in_hand(InteractionHand::MainHand).unwrap_or_else(ItemStack::empty);
                let actual = format!("interact {} held={} {}", result_name(result), held.count, row(level, id));
                self.held = held;
                self.check(actual);
            }
            "hurt" => {
                let (id, player) = (self.tags[words[1]], self.player);
                let level = self.level();
                let source = DamageSource { causing_entity: Some(player), direct_entity: Some(player), ..DamageSource::new(damage_types::PLAYER_ATTACK) };
                let hurt = level.hurt_server(id, source, words[2].parse().unwrap());
                let actual = format!("hurt {hurt} {}", row(level, id));
                self.check(actual);
            }
            "steps" => {
                let ids: Vec<EntityId> = words[2].split(',').map(|tag| self.tags[tag]).collect();
                for _ in 0..words[1].parse::<usize>().unwrap() {
                    let level = self.level();
                    level.tick();
                    let actual = ids.iter().map(|&id| row(level, id)).collect::<Vec<_>>().join(" | ");
                    self.check(actual);
                }
            }
            "run" => {
                for _ in 0..words[1].parse::<usize>().unwrap() {
                    self.level().tick();
                }
            }
            "blocks" => {
                let level = self.level();
                let mut actual = Vec::new();
                for x in int(1)..=int(4) {
                    for y in int(2)..=int(5) {
                        for z in int(3)..=int(6) {
                            let state = level.block_state(BlockPos::new(x, y, z));
                            if !state.is_air() {
                                actual.push(format!("block {x} {y} {z} {}", state_string(state)));
                            }
                        }
                    }
                }
                self.check_set("block ", actual);
            }
            "fillbiome" => {
                let biome = crate::world::level::chunk::Biome::by_name(words[7]).unwrap();
                let (from, to) = (BlockPos::new(int(1), int(2), int(3)), BlockPos::new(int(4), int(5), int(6)));
                crate::server::commands::fill_biome_command::fill(self.level(), from, to, biome, |_| true, |_| {}).ok().unwrap();
            }
            "chunkspawn" => {
                let level = self.level();
                let chunk_pos = wafer_core::ChunkPos::new(int(1), int(2));
                let source_pos = BlockPos::new(chunk_pos.x << 4, -60, chunk_pos.z << 4);
                for seed in words[3].parse::<i64>().unwrap()..=words[4].parse().unwrap() {
                    let mut random = crate::world::level::levelgen::worldgen_random::WorldgenRandom::legacy(0);
                    random.set_decoration_seed(seed, source_pos.x, source_pos.z);
                    crate::world::level::natural_spawner::spawn_mobs_for_chunk_generation(level, source_pos, chunk_pos, &mut random, true);
                }
            }
            "randomtick" => {
                let level = self.level();
                let pos = BlockPos::new(int(1), int(2), int(3));
                for _ in 0..int(4) {
                    let state = level.block_state(pos);
                    block::of(state).random_tick(state, level, pos);
                }
            }
            "entities" => {
                let level = self.level();
                let ids: Vec<EntityId> = level.entities.keys().copied().filter(|&id| level.entity_ref(id).is_some_and(|e| registry::ENTITY_TYPE[e.entity_type as usize] == words[1] && !e.is_removed())).collect();
                let actual = ids.iter().map(|&id| format!("entity {}", row(level, id))).collect();
                self.check_set("entity ", actual);
            }
            "variants" => {
                let level = self.level();
                let mut actual = Vec::new();
                for entity in level.entities.values() {
                    let LevelEntity::Mob(mob) = entity else { continue };
                    if mob.entity.is_removed() {
                        continue;
                    }
                    match mob.kind {
                        MobKind::Rabbit(_) => actual.push(format!("rabbit {} baby={}", rabbit::get_variant(&mob.entity).get_serialized_name(), mob.is_baby())),
                        MobKind::Turtle(_) => actual.push(format!("turtle egg={} baby={}", turtle::has_egg(&mob.entity), mob.is_baby())),
                        _ => {}
                    }
                }
                self.check_set("rabbit ", actual.iter().filter(|line| line.starts_with("rabbit ")).cloned().collect());
                self.check_set("turtle ", actual.into_iter().filter(|line| line.starts_with("turtle ")).collect());
            }
            "items" => {
                let level = self.level();
                let mut actual = Vec::new();
                for entity in level.entities.values() {
                    if let LevelEntity::Item(item) = entity
                        && !item.entity.is_removed()
                    {
                        actual.push(format!("item {} {}", item.item.item.name(), item.item.count));
                    }
                }
                actual.sort();
                let mut expected: Vec<String> =
                    self.expected[self.line..].iter().take(actual.len()).take_while(|line| line.starts_with("item ")).map(|line| line.split(' ').take(3).collect::<Vec<_>>().join(" ")).collect();
                expected.sort();
                self.line += expected.len();
                assert_eq!(actual, expected, "{}: items", self.label);
            }
            "nbt" => {
                let id = self.tags[words[1]];
                let level = self.level();
                let nbt = level.entity_tag_to_compare(id);
                self.nbt.push((words[1].to_string(), nbt));
                self.line += 1;
            }
            "discard" => {
                let level = self.level();
                let ids: Vec<EntityId> = level.entities.keys().copied().filter(|&id| !level.is_player(id)).collect();
                for id in ids {
                    level.remove(id, crate::world::entity::RemovalReason::Discarded);
                }
            }
            other => panic!("unknown script command {other}"),
        }
    }
}

const ORACLE: &str = include_str!("rabbit_turtle_oracle.txt");

fn replay(script: &str) -> Replay {
    let mut replay = Replay::new(ORACLE);
    for line in script.lines().map(str::trim).filter(|line| !line.is_empty()) {
        replay.run(line);
    }
    replay.finish();
    replay
}

fn vanilla_nbt(label: &str) -> Vec<Nbt> {
    oracle(ORACLE, label).iter().filter_map(|line| line.strip_prefix("nbt ")).map(|nbt| parse_snbt(nbt).unwrap()).collect()
}

fn assert_same_keys(actual: &Nbt, expected: &Nbt, keys: &[&str]) {
    for key in keys {
        assert_eq!(actual.get(key), expected.get(key), "{key}");
    }
}

fn attributes_touched_by_the_game(nbt: &Nbt) -> Vec<String> {
    let mut attributes: Vec<String> = nbt
        .get("attributes")
        .map(Nbt::as_list)
        .unwrap_or(&[])
        .iter()
        .filter(|attribute| attribute.get("id").and_then(Nbt::as_str) != Some("minecraft:follow_range"))
        .map(|attribute| {
            let mut fields = match attribute {
                Nbt::Compound(fields) => fields.clone(),
                _ => Vec::new(),
            };
            fields.sort_by(|a, b| a.0.cmp(&b.0));
            format!("{fields:?}")
        })
        .collect();
    attributes.sort();
    attributes
}

#[test]
fn rabbits_hop_like_vanilla() {
    replay(
        "label rabbit_hop
        summon s minecraft:rabbit 0.5 -60 -30.5 5
        steps 300 s",
    );
}

#[test]
fn tempted_rabbits_follow_vanillas_path() {
    replay(
        "label rabbit_tempt
        tp 0.5 -60 -20.5
        hold minecraft:carrot 64
        summon s minecraft:rabbit 0.5 -60 -27.5 7
        steps 150 s",
    );
}

#[test]
fn rabbits_flee_players_like_vanilla() {
    replay(
        "label rabbit_avoid
        tp 0.5 -60 -20.5
        hold minecraft:air 1
        summon s minecraft:rabbit 0.5 -60 -24.5 11
        steps 120 s",
    );
}

#[test]
fn hurt_rabbits_panic_like_vanilla() {
    replay(
        "label rabbit_panic
        summon s minecraft:rabbit 0.5 -60 -20.5 13
        steps 5 s
        hurt s 1
        steps 100 s",
    );
}

#[test]
fn rabbits_look_for_carrots_like_vanilla() {
    replay(
        "label rabbit_raid
        fill 3 -61 -33 7 -61 -29 minecraft:farmland
        fill 3 -60 -33 7 -60 -29 minecraft:carrots[age=7]
        summon s minecraft:rabbit 0.5 -60 -31.5 17
        steps 600 s
        blocks 3 -60 -33 7 -60 -29",
    );
}

#[test]
fn rabbits_raid_the_carrots_they_stand_on() {
    replay(
        "label rabbit_raid_on_carrot
        fill 3 -61 -33 7 -61 -29 minecraft:farmland
        fill 3 -60 -33 7 -60 -29 minecraft:carrots[age=7]
        summon s minecraft:rabbit 5.5 -60 -31.5 97
        steps 200 s
        blocks 3 -60 -33 7 -60 -29",
    );
}

#[test]
fn the_killer_bunny_attacks_wolves_like_vanilla() {
    let replay = replay(
        "label killer_bunny
        summon w minecraft:wolf 0.5 -60 -24.5 19 NoAI:1b
        summon s minecraft:rabbit 0.5 -60 -28.5 23 RabbitType:99
        steps 150 s,w
        nbt s",
    );
    let expected = &vanilla_nbt("killer_bunny")[0];
    assert_same_keys(&replay.nbt[0].1, expected, &["CustomName", "RabbitType", "MoreCarrotTicks", "Health", "Age", "InLove"]);
    assert_eq!(attributes_touched_by_the_game(&replay.nbt[0].1), attributes_touched_by_the_game(expected));
}

#[test]
fn rabbits_breed_babies_of_their_variants() {
    replay(
        "label rabbit_breed
        seedlevel 101
        summon a minecraft:rabbit 0.5 -60 -30.5 29 InLove:600
        summon b minecraft:rabbit 2.5 -60 -30.5 31 InLove:600,RabbitType:2
        steps 60 a,b
        variants",
    );
}

#[test]
fn turtles_swim_like_vanilla() {
    replay(
        "label turtle_swim
        fill -10 -63 -50 20 -61 -20 minecraft:water
        summon s minecraft:turtle 5.5 -62 -35.5 37
        steps 300 s",
    );
}

#[test]
fn turtles_with_eggs_go_home_like_vanilla() {
    replay(
        "label turtle_home
        fill 20 -61 -40 40 -61 -20 minecraft:sand
        summon s minecraft:turtle 30.5 -60 -38.5 41 has_egg:1b,home_pos:[I;30,-60,-25]
        steps 900 s
        blocks 20 -60 -40 40 -60 -20",
    );
}

#[test]
fn turtles_lay_eggs_like_vanilla() {
    replay(
        "label turtle_lay
        fill 20 -61 -40 40 -61 -20 minecraft:sand
        summon s minecraft:turtle 30.5 -60 -30.5 101 has_egg:1b,home_pos:[I;30,-60,-30]
        steps 450 s
        blocks 20 -60 -40 40 -60 -20",
    );
}

#[test]
fn trapped_turtles_lay_their_eggs_where_they_stand() {
    replay(
        "label turtle_lay_pit
        fill 29 -61 -32 32 -59 -29 minecraft:stone
        fill 30 -61 -31 31 -61 -30 minecraft:sand
        fill 30 -60 -31 31 -59 -30 minecraft:air
        summon s minecraft:turtle 31.0 -60 -30.0 103 has_egg:1b,home_pos:[I;30,-60,-30]
        steps 260 s
        blocks 30 -60 -31 31 -60 -30",
    );
}

#[test]
fn breeding_turtles_carry_an_egg() {
    replay(
        "label turtle_breed
        summon a minecraft:turtle 10.5 -60 -45.5 43 InLove:600
        summon b minecraft:turtle 12.5 -60 -45.5 47 InLove:600
        steps 100 a,b
        variants",
    );
}

#[test]
fn turtle_eggs_hatch_like_vanilla() {
    replay(
        "label turtle_hatch
        time 21500
        setblock 0 -61 -40 minecraft:sand
        setblock 0 -60 -40 minecraft:turtle_egg[eggs=3,hatch=2]
        seedlevel 53
        randomtick 0 -60 -40 1
        entities minecraft:turtle
        blocks 0 -60 -40 0 -60 -40
        setblock 0 -60 -41 minecraft:turtle_egg[eggs=2,hatch=0]
        randomtick 0 -60 -41 2
        blocks 0 -60 -41 0 -60 -41",
    );
}

#[test]
fn growing_turtles_drop_a_scute() {
    replay(
        "label turtle_grow
        summon s minecraft:turtle 0.5 -60 -45.5 59 Age:-3
        seedlevel 61
        steps 5 s
        items",
    );
}

#[test]
fn rabbits_and_turtles_drop_vanillas_loot() {
    replay(
        "label drops
        seedlevel 67
        summon r minecraft:rabbit 0.5 -60 -40.5 71
        hurt r 100
        items
        discard
        seedlevel 73
        summon t minecraft:turtle 0.5 -60 -40.5 79
        hurt t 100
        items",
    );
}

#[test]
fn chunk_generation_spawns_rabbits_and_turtles_like_vanilla() {
    replay(
        "label worldgen_spawns
        fill 32 -61 -64 47 -61 -49 minecraft:sand
        fillbiome 32 -64 -64 47 -49 -49 minecraft:beach
        fillbiome 64 -64 -64 79 -49 -49 minecraft:meadow
        fillbiome 80 -64 -64 95 -49 -49 minecraft:flower_forest
        seedlevel 109
        chunkspawn 2 -4 1 60
        chunkspawn 4 -4 1 60
        chunkspawn 5 -4 1 60
        entities minecraft:turtle
        entities minecraft:rabbit
        variants",
    );
}

#[test]
fn rabbits_and_turtles_save_like_vanilla() {
    let replay = replay(
        "label save
        summon s minecraft:rabbit 0.5 -60 -40.5 83 RabbitType:3,MoreCarrotTicks:17
        nbt s
        summon t minecraft:turtle 4.5 -60 -40.5 89 has_egg:1b,home_pos:[I;1,2,3]
        nbt t",
    );
    let expected = vanilla_nbt("save");
    assert_same_keys(&replay.nbt[0].1, &expected[0], &["RabbitType", "MoreCarrotTicks", "Health", "Age", "InLove"]);
    assert_same_keys(&replay.nbt[1].1, &expected[1], &["home_pos", "has_egg", "Health", "Age", "InLove"]);
    for (actual, expected) in replay.nbt.iter().zip(&expected) {
        assert_eq!(attributes_touched_by_the_game(&actual.1), attributes_touched_by_the_game(expected));
    }
}
