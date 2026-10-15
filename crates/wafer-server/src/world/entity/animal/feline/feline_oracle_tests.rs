use std::collections::HashMap;

use wafer_core::random::LegacyRandomSource;
use wafer_core::{BlockPos, Vec3};
use wafer_data::{Block, Item};

use super::cat;
use super::ocelot;
use crate::server::level::{ServerLevel, testing};
use crate::world::difficulty::Difficulty;
use crate::world::entity::attributes::Attribute;
use crate::world::entity::mob::{MobKind, RANDOM_SPAWN_BONUS_ID};
use crate::world::entity::tamable_animal;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::level::update_flags::UPDATE_ALL;
use crate::world::{InteractionHand, InteractionResult, SwingSource};

const SCRIPT: &str = "
label creeper_flee
summon c minecraft:cat 0.5 -60 -30.5 1 NoAI
summon s minecraft:creeper 3.5 -60 -30.5 42
steps 80 s
label cat_tempt
tp 0.5 -60 -20.5
hold minecraft:cod 64
summon s minecraft:cat 0.5 -60 -26.5 7
steps 120 s
label cat_avoid
hold minecraft:air 1
summon s minecraft:cat 0.5 -60 -24.5 11
steps 100 s
label cat_chest
setblock 3 -60 -23 minecraft:chest
summon s minecraft:cat 0.5 -60 -23.5 5
tame s
steps 150 s
label ocelot_trust
hold minecraft:cod 64
summon s minecraft:ocelot 0.5 -60 -26.5 3
steps 40 s
interact s
steps 20 s
interact s
steps 20 s
interact s
steps 20 s
interact s
steps 20 s
label cat_tame
summon s minecraft:cat 0.5 -60 -18.5 13 NoAI
interact s
steps 1 s
interact s
steps 1 s
interact s
steps 1 s
interact s
steps 1 s
interact s
steps 1 s
interact s
steps 1 s
interact s
steps 1 s
interact s
steps 1 s
interact s
steps 1 s
interact s
steps 1 s
interact s
steps 1 s
interact s
steps 1 s
hold minecraft:stick 1
interact s
steps 1 s
interact s
steps 1 s
label ocelot_hunt
tp 0.5 -60 0.5
hold minecraft:air 1
summon s minecraft:ocelot 10.5 -60 -30.5 17
summon k minecraft:chicken 14.5 -60 -30.5 19
steps 160 s,k
";

fn oracle(label: &str) -> Vec<&'static str> {
    include_str!("feline_oracle.txt").lines().skip_while(|line| *line != format!("== {label}")).skip(1).take_while(|line| !line.starts_with("==") && *line != "done").collect()
}

fn row(level: &ServerLevel, id: EntityId) -> String {
    let Some(mob) = level.mob(id).filter(|mob| !mob.entity.is_removed()) else { return "gone".into() };
    let e = &mob.entity;
    let mut row = format!("{:?} {:?} {:?} {:?} pose={} sprint={} health={:?}", e.x(), e.y(), e.z(), e.y_rot, format!("{:?}", e.pose()).to_lowercase(), e.is_sprinting(), mob.health);
    if mob.tamable_animal().is_some() {
        row += &format!(" tame={} sit={} ordered={}", tamable_animal::is_tame(e), tamable_animal::is_in_sitting_pose(e), level.is_ordered_to_sit(id));
    }
    if matches!(mob.kind, MobKind::Cat(_)) {
        row += &format!(" lying={}", cat::is_lying(e));
    }
    if matches!(mob.kind, MobKind::Ocelot(_)) {
        row += &format!(" trusting={}", ocelot::is_trusting(e));
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

fn same(actual: &str, expected: &str) -> bool {
    let (actual, expected): (Vec<&str>, Vec<&str>) = (actual.split(' ').collect(), expected.split(' ').collect());
    actual.len() == expected.len()
        && actual.iter().zip(&expected).all(|(a, e)| {
            let number = |token: &str| token.rsplit('=').next().and_then(|value| value.parse::<f64>().ok());
            a == e || matches!((number(a), number(e)), (Some(a), Some(e)) if a == e || a as f32 == e as f32)
        })
}

struct Replay {
    level: Option<ServerLevel>,
    player: EntityId,
    player_pos: Vec3,
    held: ItemStack,
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

    fn run(&mut self, line: &str) {
        let words: Vec<&str> = line.split(' ').collect();
        match words[0] {
            "label" => self.start(words[1]),
            "tp" => self.player_pos = Vec3::new(words[1].parse().unwrap(), words[2].parse().unwrap(), words[3].parse().unwrap()),
            "hold" => {
                self.held = ItemStack::new(Item::by_name(words[1]).unwrap(), words[2].parse().unwrap());
                let held = self.held.clone();
                let player = self.player;
                if let Some(level) = self.level.as_mut() {
                    level.player_mut(player).set_item_in_hand(InteractionHand::MainHand, Some(held));
                }
            }
            "setblock" => {
                let pos = BlockPos::new(words[1].parse().unwrap(), words[2].parse().unwrap(), words[3].parse().unwrap());
                self.level().set_block(pos, Block::by_name(words[4]).unwrap().default_state(), UPDATE_ALL);
            }
            "summon" => {
                let parity = self.parities.remove(0);
                let (x, y, z): (f64, f64, f64) = (words[3].parse().unwrap(), words[4].parse().unwrap(), words[5].parse().unwrap());
                let seed: i64 = words[6].parse().unwrap();
                let level = self.level();
                let mut mob = loop {
                    let Some(LevelEntity::Mob(mob)) = ServerLevel::create_entity(words[2]) else { unreachable!() };
                    if mob.entity.id.rem_euclid(2) == parity {
                        break mob;
                    }
                };
                mob.persistence_required = true;
                mob.set_no_ai(words.get(7) == Some(&"NoAI"));
                mob.entity.snap_to(x, y, z, 0.0, 0.0);
                mob.entity.y_head_rot = 0.0;
                mob.living.y_body_rot = 0.0;
                mob.entity.random = LegacyRandomSource::new(seed);
                assert!(!mob.living.attributes.has_modifier(Attribute::FollowRange, RANDOM_SPAWN_BONUS_ID));
                let id = mob.entity.id;
                level.add_fresh_entity(LevelEntity::Mob(mob));
                self.tags.insert(words[1].into(), id);
            }
            "tame" => {
                let (id, player) = (self.tags[words[1]], self.player);
                self.level().tame(id, player);
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
            "steps" => {
                let ids: Vec<EntityId> = words[2].split(',').map(|tag| self.tags[tag]).collect();
                for _ in 0..words[1].parse::<usize>().unwrap() {
                    let level = self.level();
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
fn cats_and_ocelots_behave_like_vanilla() {
    let mut replay = Replay {
        level: None,
        player: 0,
        player_pos: Vec3::new(0.5, -60.0, 0.5),
        held: ItemStack::empty(),
        tags: HashMap::new(),
        label: String::new(),
        expected: Vec::new(),
        parities: Vec::new(),
        line: 0,
    };
    for line in SCRIPT.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if line.starts_with("label ") && !replay.label.is_empty() {
            assert_eq!(replay.line, replay.expected.len(), "{}: fewer rows than the oracle", replay.label);
        }
        replay.run(line);
    }
    assert_eq!(replay.line, replay.expected.len(), "{}: fewer rows than the oracle", replay.label);
}
