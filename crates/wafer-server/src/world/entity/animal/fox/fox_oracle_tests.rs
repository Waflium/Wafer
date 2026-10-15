use std::collections::HashMap;

use wafer_core::random::LegacyRandomSource;
use wafer_core::{BlockPos, Vec3};
use wafer_data::{EquipmentSlot, Item, game_rule, registry};
use wafer_protocol::nbt::Nbt;

use super::fox;
use crate::server::level::{ServerLevel, testing};
use crate::world::clock::ServerClockManager;
use crate::world::difficulty::Difficulty;
use crate::world::entity::ai::goal::goal::Goal;
use crate::world::entity::attributes::Attribute;
use crate::world::entity::item::ItemEntity;
use crate::world::entity::mob::RANDOM_SPAWN_BONUS_ID;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::item::component::custom::parse_snbt;
use crate::world::level::levelgen::structure::templatesystem::structure_processor::parse_block_state_string;
use crate::world::level::update_flags::UPDATE_ALL;

const SCRIPT: &str = include_str!("fox_oracle_script.txt");

fn oracle(label: &str) -> Vec<&'static str> {
    include_str!("fox_oracle.txt").lines().skip_while(|line| *line != format!("== {label}")).skip(1).take_while(|line| !line.starts_with("==") && *line != "done").collect()
}

fn item(stack: Option<ItemStack>) -> String {
    match stack.filter(|stack| !stack.is_empty()) {
        Some(stack) => format!("{}x{}", stack.get_item().name(), stack.count),
        None => "-".into(),
    }
}

fn flags(level: &ServerLevel, id: EntityId) -> i32 {
    match level.entity(id).data.get(fox::DATA_FLAGS_ID) {
        crate::world::entity::data::DataValue::Byte(flags) => *flags as u8 as i32,
        _ => 0,
    }
}

fn row(level: &ServerLevel, id: EntityId) -> String {
    let Some(mob) = level.mob(id).filter(|mob| !mob.entity.is_removed()) else { return "gone".into() };
    let e = &mob.entity;
    let mut row = format!("{:?} {:?} {:?} {:?} {:?} health={:?}", e.x(), e.y(), e.z(), e.y_rot, e.x_rot, mob.health);
    if level.is_fox(id) {
        let target = level.get_target(id).map_or("-".to_string(), |target| registry::ENTITY_TYPE[level.entity(target).entity_type as usize].trim_start_matches("minecraft:").to_string());
        row += &format!(" flags={} hand={} target={}", flags(level, id), item(level.item_by_slot(id, EquipmentSlot::Mainhand)), target);
    }
    row
}

fn same(actual: &str, expected: &str) -> bool {
    let (actual, expected): (Vec<&str>, Vec<&str>) = (actual.split(' ').collect(), expected.split(' ').collect());
    actual.len() == expected.len()
        && actual.iter().zip(&expected).all(|(a, e)| {
            let number = |token: &str| token.rsplit('=').next().and_then(|value| value.parse::<f64>().ok());
            a == e || matches!((number(a), number(e)), (Some(a), Some(e)) if a == e || a as f32 == e as f32)
        })
}

fn sorted(nbt: Nbt) -> Nbt {
    match nbt {
        Nbt::Compound(mut entries) => {
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            Nbt::Compound(entries.into_iter().map(|(k, v)| (k, sorted(v))).collect())
        }
        Nbt::List(list) => Nbt::List(list.into_iter().map(sorted).collect()),
        other => other,
    }
}

fn snbt_value(value: &str) -> Option<Nbt> {
    if value == "null" {
        return None;
    }
    let Nbt::Compound(mut fields) = parse_snbt(&format!("{{v:{value}}}")).unwrap() else { unreachable!() };
    Some(sorted(fields.remove(0).1))
}

fn set_time(level: &mut ServerLevel, ticks: i64) {
    let mut clocks = ServerClockManager::new();
    clocks.set_total_ticks(ServerClockManager::id_of("minecraft:overworld").unwrap(), ticks);
    level.sync_clocks(&clocks);
}

fn block_pos(words: &[&str]) -> BlockPos {
    BlockPos::new(words[0].parse().unwrap(), words[1].parse().unwrap(), words[2].parse().unwrap())
}

struct Replay {
    level: Option<ServerLevel>,
    player: EntityId,
    tags: HashMap<String, EntityId>,
    label: String,
    expected: Vec<&'static str>,
    parities: Vec<i32>,
    line: usize,
}

impl Replay {
    fn level(&mut self) -> &mut ServerLevel {
        if self.level.is_none() {
            let mut level = testing::level();
            level.difficulty = Difficulty::Normal;
            level.game_rules.set(game_rule::RANDOM_TICK_SPEED, 0);
            let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
            for _ in 0..61 {
                level.tick();
            }
            self.player = player;
            self.level = Some(level);
        }
        self.level.as_mut().unwrap()
    }

    fn start(&mut self, label: &str) {
        self.level = None;
        self.tags.clear();
        self.label = label.into();
        self.expected = oracle(label);
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

    fn check_nbt(&mut self, id: EntityId) {
        let expected: &'static str = *self.expected.get(self.line).unwrap_or_else(|| panic!("{}: more rows than the oracle", self.label));
        let nbt = self.level().entities[&id].save().unwrap();
        const KEYS: [&str; 8] = ["Type", "Sleeping", "Sitting", "Crouching", "Trusted", "equipment", "drop_chances", "CanPickUpLoot"];
        for (i, key) in KEYS.iter().enumerate() {
            let start = expected.find(&format!(" {key}=")).unwrap() + key.len() + 2;
            let end = KEYS.get(i + 1).map_or(expected.len(), |next| expected.find(&format!(" {next}=")).unwrap());
            assert_eq!(nbt.get(key).cloned().map(sorted), snbt_value(&expected[start..end]), "{}: {key}", self.label);
        }
        self.line += 1;
    }

    fn run(&mut self, line: &str) {
        let words: Vec<&str> = line.split(' ').collect();
        match words[0] {
            "cmd" | "sleep" | "discard" => {}
            "label" => self.start(words[1]),
            "gamerule" => {
                assert_eq!(words[1], "mob_drops");
                let value = (words[2] == "true") as i32;
                self.level().game_rules.set(game_rule::MOB_DROPS, value);
            }
            "time" => {
                let ticks = words[1].parse().unwrap();
                set_time(self.level(), ticks);
            }
            "setblock" => {
                let state = parse_block_state_string(words[4]).unwrap();
                self.level().set_block(block_pos(&words[1..4]), state, UPDATE_ALL);
            }
            "fill" => {
                let (from, to, state) = (block_pos(&words[1..4]), block_pos(&words[4..7]), parse_block_state_string(words[7]).unwrap());
                let level = self.level();
                for x in from.x..=to.x {
                    for y in from.y..=to.y {
                        for z in from.z..=to.z {
                            level.set_block(BlockPos::new(x, y, z), state, UPDATE_ALL);
                        }
                    }
                }
            }
            "summon" => {
                let parity = self.parities.remove(0);
                let (x, y, z): (f64, f64, f64) = (words[3].parse().unwrap(), words[4].parse().unwrap(), words[5].parse().unwrap());
                let seed: i64 = words[6].parse().unwrap();
                let extra = words.get(7..).map(|rest| rest.join(" ")).filter(|rest| !rest.is_empty()).map_or(String::new(), |rest| format!(",{rest}"));
                let nbt = parse_snbt(&format!("{{id:\"{}\",Pos:[{x}d,{y}d,{z}d],PersistenceRequired:1b,Rotation:[0f,0f],Tags:[\"{}\"]{extra}}}", words[2], words[1])).unwrap();
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
                assert!(!mob.living.attributes.has_modifier(Attribute::FollowRange, RANDOM_SPAWN_BONUS_ID));
                for goal in mob.ai.goal_selector.get_available_goals_mut() {
                    if let Goal::Sleep(goal) = goal.get_goal_mut() {
                        goal.set_countdown(0);
                    }
                }
                let id = entity.entity().id;
                level.add_fresh_entity(entity);
                self.tags.insert(words[1].into(), id);
            }
            "item" => {
                let pos = Vec3::new(words[1].parse().unwrap(), words[2].parse().unwrap(), words[3].parse().unwrap());
                let stack = ItemStack::new(Item::by_name(words[4]).unwrap(), words[5].parse().unwrap());
                let mut item = ItemEntity::with_movement(pos, stack, Vec3::ZERO);
                item.set_no_pickup_delay();
                self.level().add_fresh_entity(LevelEntity::Item(item));
            }
            "seedlevel" => self.level().random = LegacyRandomSource::new(words[1].parse().unwrap()),
            "love" => {
                let (id, player) = (self.tags[words[1]], self.player);
                self.level().set_in_love(id, Some(player));
            }
            "mouth" => {
                let id = self.tags[words[1]];
                let stack = ItemStack::new(Item::by_name(words[2]).unwrap(), words[3].parse().unwrap());
                self.level().living_set_item_slot(id, EquipmentSlot::Mainhand, stack);
            }
            "kill" => {
                let id = self.tags[words[1]];
                self.level().kill(id);
            }
            "nbt" => {
                let id = self.tags[words[1]];
                self.check_nbt(id);
            }
            "items" => {
                let level = self.level();
                let mut actual: Vec<(&'static str, i32)> = Vec::new();
                for item in level.entities.values().filter_map(LevelEntity::as_item).filter(|item| !item.entity.is_removed()) {
                    actual.push((item.item.get_item().name(), item.item.count));
                }
                let mut expected: Vec<(&'static str, i32)> = Vec::new();
                while let Some(line) = self.expected.get(self.line).and_then(|line| line.strip_prefix("item ")) {
                    let (name, count) = line.rsplit_once('x').unwrap();
                    expected.push((Item::by_name(name).unwrap().name(), count.parse().unwrap()));
                    self.line += 1;
                }
                let totals = |items: Vec<(&'static str, i32)>| {
                    let mut totals: Vec<(&'static str, i32)> = Vec::new();
                    for (name, count) in items {
                        match totals.iter_mut().find(|(n, _)| *n == name) {
                            Some((_, total)) => *total += count,
                            None => totals.push((name, count)),
                        }
                    }
                    totals.sort();
                    totals
                };
                assert_eq!(totals(actual), totals(expected), "{}: dropped items", self.label);
            }
            "babies" => {
                let player = self.player;
                let level = self.level();
                let babies: Vec<EntityId> = level.entities.keys().copied().filter(|&id| level.is_fox(id) && level.is_baby(id)).collect();
                let rows: Vec<String> = babies
                    .into_iter()
                    .map(|id| {
                        let age = level.mob(id).unwrap().ageable().unwrap().age;
                        format!("baby age={age} variant={} trusts={}", fox::get_variant(level.entity(id)).get_serialized_name(), level.trusts(id, player))
                    })
                    .collect();
                for actual in rows {
                    self.check(actual);
                }
            }
            "steps" => {
                let ids: Vec<EntityId> = words[2].split(',').map(|tag| self.tags[tag]).collect();
                for _ in 0..words[1].parse::<usize>().unwrap() {
                    let level = self.level();
                    level.chunks.run_light_updates();
                    level.tick();
                    let actual = ids.iter().map(|&id| row(level, id)).collect::<Vec<_>>().join(" | ");
                    self.check(actual);
                }
            }
            other => panic!("unknown script command {other}"),
        }
    }
}

#[test]
fn foxes_behave_like_vanilla() {
    let mut replay = Replay { level: None, player: 0, tags: HashMap::new(), label: String::new(), expected: Vec::new(), parities: Vec::new(), line: 0 };
    for line in SCRIPT.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if line.starts_with("label ") && !replay.label.is_empty() {
            assert_eq!(replay.line, replay.expected.len(), "{}: fewer rows than the oracle", replay.label);
        }
        replay.run(line);
    }
    assert_eq!(replay.line, replay.expected.len(), "{}: fewer rows than the oracle", replay.label);
}
