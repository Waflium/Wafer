use std::sync::OnceLock;

use serde_json::Value;

use super::test_environment_definition::{self, EnvironmentHolder};
use crate::network::chat::{ChatFormatting, Component};
use crate::world::item::component::codec::{Codec, Dispatch, Field, RangeError, Selector, Stream};
use crate::world::item::component::types::{self, Pair, def, req};
use crate::world::level::block::rotation::Rotation;

pub const REGISTRY: &str = "minecraft:test_instance";

pub struct TestData {
    pub environment: EnvironmentHolder,
    pub dimension: String,
    pub structure: String,
    pub max_ticks: i32,
    pub setup_ticks: i32,
    pub required: bool,
    pub rotation: Rotation,
    pub manual_only: bool,
    pub max_attempts: i32,
    pub required_successes: i32,
    pub sky_access: bool,
    pub padding: i32,
}

impl TestData {
    fn parse(json: &Value) -> Result<TestData, String> {
        let identifier = |key: &str| json.get(key).and_then(Value::as_str).map(resource);
        let int = |key: &str, default: i32| json.get(key).and_then(Value::as_i64).map_or(default, |v| v as i32);
        let boolean = |key: &str, default: bool| json.get(key).and_then(Value::as_bool).unwrap_or(default);
        Ok(TestData {
            environment: EnvironmentHolder::parse(json.get("environment").ok_or("No key environment")?)?,
            dimension: identifier("dimension").unwrap_or_else(|| "minecraft:overworld".to_string()),
            structure: identifier("structure").ok_or("No key structure")?,
            max_ticks: int("max_ticks", 0),
            setup_ticks: int("setup_ticks", 0),
            required: boolean("required", true),
            rotation: json.get("rotation").and_then(Value::as_str).and_then(Rotation::by_name).unwrap_or(Rotation::None),
            manual_only: boolean("manual_only", false),
            max_attempts: int("max_attempts", 1),
            required_successes: int("required_successes", 1),
            sky_access: boolean("sky_access", false),
            padding: int("padding", 0),
        })
    }
}

fn test_data_fields() -> Vec<Field> {
    vec![
        req("environment", test_environment_definition::holder_codec()),
        def("dimension", types::identifier(), types::s("minecraft:overworld")),
        req("structure", types::identifier()),
        req("max_ticks", types::var_int(types::positive_int())),
        def("setup_ticks", types::var_int(types::non_negative_int()), types::i(0)),
        def("required", types::bool(), types::b(true)),
        def("rotation", types::enumeration(&["none", "clockwise_90", "180", "counterclockwise_90"]), types::s("none")),
        def("manual_only", types::bool(), types::b(false)),
        def("max_attempts", types::var_int(types::positive_int()), types::i(1)),
        def("required_successes", types::var_int(types::positive_int()), types::i(1)),
        def("sky_access", types::bool(), types::b(false)),
        def("padding", types::var_int(Codec::Int(0, 128, RangeError::Outside)), types::i(0)),
    ]
}

pub fn direct_codec() -> Pair {
    let mut function = vec![req("function", types::identifier())];
    function.extend(test_data_fields());
    let cases = vec![("minecraft:block_based", test_data_fields()), ("minecraft:function", function)];
    (Codec::Dispatch(Box::new(Dispatch { key: "type", selector: Selector::Registry("minecraft:test_instance_type"), cases })), Stream::Dispatch)
}

pub enum GameTestInstanceType {
    Function(String),
    BlockBased,
}

pub struct GameTestInstance {
    pub kind: GameTestInstanceType,
    pub info: TestData,
}

pub fn resource(name: &str) -> String {
    if name.contains(':') { name.to_string() } else { format!("minecraft:{name}") }
}

impl GameTestInstance {
    fn parse(json: &Value) -> Result<GameTestInstance, String> {
        let kind = match json.get("type").and_then(Value::as_str).map(resource).as_deref() {
            Some("minecraft:function") => GameTestInstanceType::Function(resource(json.get("function").and_then(Value::as_str).ok_or("No key function")?)),
            Some("minecraft:block_based") => GameTestInstanceType::BlockBased,
            other => return Err(format!("Unknown test instance type {other:?}")),
        };
        Ok(GameTestInstance { kind, info: TestData::parse(json)? })
    }

    fn registered() -> &'static [(&'static str, Option<GameTestInstance>)] {
        static INSTANCES: OnceLock<Vec<(&'static str, Option<GameTestInstance>)>> = OnceLock::new();
        INSTANCES.get_or_init(|| {
            let entries = crate::core::data_registries::json_entries(REGISTRY);
            let order: Vec<usize> = match crate::core::data_registries::get(REGISTRY) {
                Some(registry) => registry.network_order().map(|id| id as usize).collect(),
                None => (0..entries.len()).collect(),
            };
            order
                .into_iter()
                .map(|index| {
                    let (name, json) = entries[index];
                    (name, serde_json::from_str(json).ok().and_then(|json: Value| GameTestInstance::parse(&json).ok()))
                })
                .collect()
        })
    }

    pub fn get(key: &str) -> Option<(&'static str, &'static GameTestInstance)> {
        GameTestInstance::registered().iter().find(|(name, _)| *name == key).and_then(|(name, test)| test.as_ref().map(|test| (*name, test)))
    }

    pub fn all() -> impl Iterator<Item = (&'static str, &'static GameTestInstance)> {
        GameTestInstance::registered().iter().filter_map(|(name, test)| test.as_ref().map(|test| (*name, test)))
    }

    pub fn batch(&self) -> &EnvironmentHolder {
        &self.info.environment
    }

    pub fn dimension(&self) -> &str {
        &self.info.dimension
    }

    pub fn structure(&self) -> &str {
        &self.info.structure
    }

    pub fn max_ticks(&self) -> i32 {
        self.info.max_ticks
    }

    pub fn setup_ticks(&self) -> i32 {
        self.info.setup_ticks
    }

    pub fn required(&self) -> bool {
        self.info.required
    }

    pub fn manual_only(&self) -> bool {
        self.info.manual_only
    }

    pub fn max_attempts(&self) -> i32 {
        self.info.max_attempts
    }

    pub fn required_successes(&self) -> i32 {
        self.info.required_successes
    }

    pub fn sky_access(&self) -> bool {
        self.info.sky_access
    }

    pub fn rotation(&self) -> Rotation {
        self.info.rotation
    }

    pub fn padding(&self) -> i32 {
        self.info.padding
    }

    fn type_description(&self) -> Component {
        match self.kind {
            GameTestInstanceType::Function(_) => Component::translatable("test_instance.type.function"),
            GameTestInstanceType::BlockBased => Component::translatable("test_instance.type.block_based"),
        }
    }

    pub fn describe(&self) -> Component {
        let mut description = self.describe_type();
        if let GameTestInstanceType::Function(function) = &self.kind {
            description = description.append(description_row("test_instance.description.function", Component::literal(function)));
        }
        description.append(self.describe_info())
    }

    fn describe_type(&self) -> Component {
        description_row("test_instance.description.type", self.type_description())
    }

    fn describe_info(&self) -> Component {
        description_row("test_instance.description.structure", Component::literal(&self.info.structure))
            .append(description_row("test_instance.description.batch", Component::literal(self.info.environment.registered_name())))
            .append(description_row("test_instance.description.dimension", Component::literal(&self.info.dimension)))
    }
}

fn description_row(translation_key: &str, value: Component) -> Component {
    Component::translatable_with(translation_key, vec![value.with_color(ChatFormatting::Blue).into()]).append(Component::literal("\n"))
}
