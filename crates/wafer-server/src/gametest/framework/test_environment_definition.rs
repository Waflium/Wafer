use std::sync::OnceLock;

use serde_json::Value;
use wafer_data::{GameRule, GameRuleType};

use super::game_test_instance::resource;
use crate::commands::source::CommandSourceStack;
use crate::server::Server;
use crate::server::server_function_manager::ServerFunctionManager;
use crate::world::clock::ServerClockManager;
use crate::world::difficulty::Difficulty;

pub const REGISTRY: &str = "minecraft:test_environment";

pub enum EnvironmentHolder {
    Reference(String),
    Direct(Box<TestEnvironmentDefinition>),
}

impl EnvironmentHolder {
    pub fn parse(json: &Value) -> Result<EnvironmentHolder, String> {
        match json {
            Value::String(name) => Ok(EnvironmentHolder::Reference(resource(name))),
            other => Ok(EnvironmentHolder::Direct(Box::new(TestEnvironmentDefinition::parse(other)?))),
        }
    }

    pub fn registered_name(&self) -> &str {
        match self {
            EnvironmentHolder::Reference(name) => name,
            EnvironmentHolder::Direct(_) => "[unregistered]",
        }
    }

    pub fn batch_key(&self) -> String {
        match self {
            EnvironmentHolder::Reference(name) => name.clone(),
            EnvironmentHolder::Direct(definition) => format!("{:p}", &**definition),
        }
    }

    pub fn value(&self) -> Option<&TestEnvironmentDefinition> {
        match self {
            EnvironmentHolder::Reference(name) => TestEnvironmentDefinition::get(name),
            EnvironmentHolder::Direct(definition) => Some(definition),
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum WeatherType {
    Clear,
    Rain,
    Thunder,
}

impl WeatherType {
    fn by_name(name: &str) -> Option<WeatherType> {
        match name {
            "clear" => Some(WeatherType::Clear),
            "rain" => Some(WeatherType::Rain),
            "thunder" => Some(WeatherType::Thunder),
            _ => None,
        }
    }

    fn apply(self, server: &mut Server) {
        match self {
            WeatherType::Clear => server.set_weather_parameters(100000, 0, false, false),
            WeatherType::Rain => server.set_weather_parameters(0, 100000, true, false),
            WeatherType::Thunder => server.set_weather_parameters(0, 100000, true, true),
        }
    }
}

pub enum TestEnvironmentDefinition {
    AllOf(Vec<EnvironmentHolder>),
    ClockTime { clock: String, time: i32 },
    SetDifficulty(Difficulty),
    Functions { setup_function: Option<String>, teardown_function: Option<String> },
    SetGameRules(Vec<(GameRule, i32)>),
    Timelines(Vec<String>),
    Weather(WeatherType),
}

pub enum Activation {
    AllOf(Vec<Activation>),
    ClockTime { clock: Option<usize>, previous: i64 },
    SetDifficulty(Difficulty),
    Functions(Option<String>),
    SetGameRules(Vec<(GameRule, i32)>),
    Timelines,
    Weather(WeatherType),
}

impl TestEnvironmentDefinition {
    fn parse(json: &Value) -> Result<TestEnvironmentDefinition, String> {
        let identifiers = |key: &str| -> Vec<String> { json.get(key).and_then(Value::as_array).map_or_else(Vec::new, |list| list.iter().filter_map(Value::as_str).map(resource).collect()) };
        match json.get("type").and_then(Value::as_str).map(resource).as_deref() {
            Some("minecraft:all_of") => Ok(TestEnvironmentDefinition::AllOf(
                json.get("definitions").and_then(Value::as_array).map_or(Ok(Vec::new()), |list| list.iter().map(EnvironmentHolder::parse).collect())?,
            )),
            Some("minecraft:clock_time") => Ok(TestEnvironmentDefinition::ClockTime {
                clock: json.get("clock").and_then(Value::as_str).map(resource).ok_or("No key clock")?,
                time: json.get("time").and_then(Value::as_i64).ok_or("No key time")? as i32,
            }),
            Some("minecraft:difficulty") => Ok(TestEnvironmentDefinition::SetDifficulty(
                json.get("difficulty").and_then(Value::as_str).and_then(Difficulty::by_name).ok_or("No key difficulty")?,
            )),
            Some("minecraft:function") => Ok(TestEnvironmentDefinition::Functions {
                setup_function: json.get("setup").and_then(Value::as_str).map(resource),
                teardown_function: json.get("teardown").and_then(Value::as_str).map(resource),
            }),
            Some("minecraft:game_rules") => {
                let rules = json.get("rules").and_then(Value::as_object).ok_or("No key rules")?;
                let mut values = Vec::new();
                for (name, value) in rules {
                    let rule = GameRule::by_name(name).ok_or_else(|| format!("Unknown game rule {name}"))?;
                    let value = match rule.info().kind {
                        GameRuleType::Bool => value.as_bool().ok_or("Not a boolean")? as i32,
                        GameRuleType::Int => value.as_i64().ok_or("Not a number")? as i32,
                    };
                    values.push((rule, value));
                }
                Ok(TestEnvironmentDefinition::SetGameRules(values))
            }
            Some("minecraft:timeline_attributes") => Ok(TestEnvironmentDefinition::Timelines(identifiers("timelines"))),
            Some("minecraft:weather") => {
                Ok(TestEnvironmentDefinition::Weather(json.get("weather").and_then(Value::as_str).and_then(WeatherType::by_name).ok_or("No key weather")?))
            }
            other => Err(format!("Unknown test environment type {other:?}")),
        }
    }

    pub fn get(key: &str) -> Option<&'static TestEnvironmentDefinition> {
        static DEFINITIONS: OnceLock<Vec<(&'static str, Option<TestEnvironmentDefinition>)>> = OnceLock::new();
        DEFINITIONS
            .get_or_init(|| {
                crate::core::data_registries::json_entries(REGISTRY)
                    .iter()
                    .map(|&(name, json)| (name, serde_json::from_str(json).ok().and_then(|json: Value| TestEnvironmentDefinition::parse(&json).ok())))
                    .collect()
            })
            .iter()
            .find(|(name, _)| *name == key)
            .and_then(|(_, definition)| definition.as_ref())
    }

    pub fn activate(&self, server: &mut Server, level: usize) -> Activation {
        match self {
            TestEnvironmentDefinition::AllOf(definitions) => {
                Activation::AllOf(definitions.iter().filter_map(|definition| definition.value()).map(|definition| definition.activate(server, level)).collect())
            }
            TestEnvironmentDefinition::ClockTime { clock, time } => {
                let clock = ServerClockManager::id_of(clock);
                let previous = clock.map_or(0, |clock| server.clocks.clocks[clock].total_ticks);
                if let Some(clock) = clock {
                    server.clocks.set_total_ticks(clock, *time as i64);
                }
                Activation::ClockTime { clock, previous }
            }
            TestEnvironmentDefinition::SetDifficulty(difficulty) => {
                let old_difficulty = server.difficulty;
                server.set_difficulty(*difficulty, true);
                Activation::SetDifficulty(old_difficulty)
            }
            TestEnvironmentDefinition::Functions { setup_function, teardown_function } => {
                if let Some(function) = setup_function {
                    run(server, level, function);
                }
                Activation::Functions(teardown_function.clone())
            }
            TestEnvironmentDefinition::SetGameRules(rules) => {
                let original_state = rules.iter().map(|&(rule, _)| (rule, server.game_rules.get(rule))).collect();
                for &(rule, value) in rules {
                    server.set_game_rule(rule, value);
                }
                Activation::SetGameRules(original_state)
            }
            TestEnvironmentDefinition::Timelines(_) => Activation::Timelines,
            TestEnvironmentDefinition::Weather(weather) => {
                let level_ref = &server.levels[level];
                let previous = if level_ref.is_thundering() {
                    WeatherType::Thunder
                } else if level_ref.is_raining() {
                    WeatherType::Rain
                } else {
                    WeatherType::Clear
                };
                weather.apply(server);
                Activation::Weather(previous)
            }
        }
    }
}

impl Activation {
    pub fn teardown(self, server: &mut Server, level: usize) {
        match self {
            Activation::AllOf(activations) => {
                for activation in activations.into_iter().rev() {
                    activation.teardown(server, level);
                }
            }
            Activation::ClockTime { clock, previous } => {
                if let Some(clock) = clock {
                    server.clocks.set_total_ticks(clock, previous);
                }
            }
            Activation::SetDifficulty(difficulty) => server.set_difficulty(difficulty, true),
            Activation::Functions(teardown_function) => {
                if let Some(function) = teardown_function {
                    run(server, level, &function);
                }
            }
            Activation::SetGameRules(rules) => {
                for (rule, value) in rules {
                    server.set_game_rule(rule, value);
                }
            }
            Activation::Timelines => {}
            Activation::Weather(previous) => {
                server.levels[level].reset_weather_cycle();
                previous.apply(server);
            }
        }
    }
}

fn run(server: &mut Server, level: usize, function_id: &str) {
    match server.functions.get(function_id) {
        Some(function) => {
            let source = CommandSourceStack::server(server).with_permission(2).with_suppressed_output().with_level(server, level);
            ServerFunctionManager::execute(server, &function, &source);
        }
        None => error!("Test Batch failed for non-existent function {function_id}"),
    }
}
