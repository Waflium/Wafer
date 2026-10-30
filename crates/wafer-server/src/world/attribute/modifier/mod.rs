pub mod boolean_modifier;
pub mod color_modifier;
pub mod float_modifier;
pub mod float_with_alpha;
pub mod integer_modifier;
pub mod list_modifier;
pub mod mob_spawn_settings_modifier;

pub use boolean_modifier::BooleanModifier;
pub use color_modifier::{BlendToGray, ColorModifier};
pub use float_modifier::FloatModifier;
pub use float_with_alpha::FloatWithAlpha;
pub use integer_modifier::IntegerModifier;

use serde_json::Value as Json;
use wafer_protocol::nbt::Nbt;

use super::environment_attribute::EnvironmentAttributeInfo;
use super::{LerpFunction, Value};
use crate::util::argb;
use crate::util::extra_codecs::hex_color;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum OperationId {
    Override,
    AlphaBlend,
    Add,
    Subtract,
    Multiply,
    BlendToGray,
    Minimum,
    Maximum,
    And,
    Nand,
    Or,
    Nor,
    Xor,
    Xnor,
    Append,
    Overlay,
}

impl OperationId {
    pub const VALUES: [OperationId; 16] = [
        OperationId::Override,
        OperationId::AlphaBlend,
        OperationId::Add,
        OperationId::Subtract,
        OperationId::Multiply,
        OperationId::BlendToGray,
        OperationId::Minimum,
        OperationId::Maximum,
        OperationId::And,
        OperationId::Nand,
        OperationId::Or,
        OperationId::Nor,
        OperationId::Xor,
        OperationId::Xnor,
        OperationId::Append,
        OperationId::Overlay,
    ];

    pub fn name(self) -> &'static str {
        [
            "override",
            "alpha_blend",
            "add",
            "subtract",
            "multiply",
            "blend_to_gray",
            "minimum",
            "maximum",
            "and",
            "nand",
            "or",
            "nor",
            "xor",
            "xnor",
            "append",
            "overlay",
        ][self as usize]
    }

    pub fn by_name(name: &str) -> Option<OperationId> {
        Self::VALUES.into_iter().find(|o| o.name() == name)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum AttributeModifier {
    Override,
    Boolean(BooleanModifier),
    Float(FloatModifier),
    Integer(IntegerModifier),
    Color(ColorModifier),
    Append,
    Overlay,
}

impl AttributeModifier {
    pub fn apply(self, subject: &Value, argument: &Value) -> Value {
        match self {
            AttributeModifier::Override => argument.clone(),
            AttributeModifier::Boolean(m) => Value::Boolean(m.apply(subject.as_bool(), argument.as_bool())),
            AttributeModifier::Float(m) => m.apply(subject.as_float(), argument),
            AttributeModifier::Integer(m) => Value::Integer(m.apply(subject.as_int(), argument.as_int())),
            AttributeModifier::Color(m) => m.apply(subject, argument),
            AttributeModifier::Append => list_modifier::append(subject, argument),
            AttributeModifier::Overlay => mob_spawn_settings_modifier::overlay(subject, argument),
        }
    }

    pub fn operation_id(self) -> OperationId {
        match self {
            AttributeModifier::Override => OperationId::Override,
            AttributeModifier::Boolean(m) => match m {
                BooleanModifier::And => OperationId::And,
                BooleanModifier::Nand => OperationId::Nand,
                BooleanModifier::Or => OperationId::Or,
                BooleanModifier::Nor => OperationId::Nor,
                BooleanModifier::Xor => OperationId::Xor,
                BooleanModifier::Xnor => OperationId::Xnor,
            },
            AttributeModifier::Float(m) => match m {
                FloatModifier::AlphaBlend => OperationId::AlphaBlend,
                FloatModifier::Add => OperationId::Add,
                FloatModifier::Subtract => OperationId::Subtract,
                FloatModifier::Multiply => OperationId::Multiply,
                FloatModifier::Minimum => OperationId::Minimum,
                FloatModifier::Maximum => OperationId::Maximum,
            },
            AttributeModifier::Integer(m) => match m {
                IntegerModifier::Add => OperationId::Add,
                IntegerModifier::Subtract => OperationId::Subtract,
                IntegerModifier::Multiply => OperationId::Multiply,
                IntegerModifier::Minimum => OperationId::Minimum,
                IntegerModifier::Maximum => OperationId::Maximum,
            },
            AttributeModifier::Color(m) => match m {
                ColorModifier::AlphaBlendRgb | ColorModifier::AlphaBlendArgb => OperationId::AlphaBlend,
                ColorModifier::AddRgb | ColorModifier::AddArgb => OperationId::Add,
                ColorModifier::SubtractRgb | ColorModifier::SubtractArgb => OperationId::Subtract,
                ColorModifier::MultiplyRgb | ColorModifier::MultiplyArgb => OperationId::Multiply,
                ColorModifier::BlendToGrayRgb | ColorModifier::BlendToGrayArgb => OperationId::BlendToGray,
            },
            AttributeModifier::Append => OperationId::Append,
            AttributeModifier::Overlay => OperationId::Overlay,
        }
    }

    pub fn encode_argument(self, attribute: &EnvironmentAttributeInfo, argument: &Value) -> Result<Nbt, String> {
        Ok(match (self, argument) {
            (AttributeModifier::Override | AttributeModifier::Append | AttributeModifier::Overlay, argument) => attribute.ty.encode_value(argument)?,
            (AttributeModifier::Boolean(_), Value::Boolean(argument)) => Nbt::Byte(*argument as i8),
            (AttributeModifier::Integer(_), Value::Integer(argument)) => Nbt::Int(*argument),
            (AttributeModifier::Float(_), Value::Float(argument)) => Nbt::Float(*argument),
            (AttributeModifier::Float(_), Value::FloatWithAlpha(argument)) => {
                if argument.alpha == 1.0 {
                    Nbt::Float(argument.value)
                } else {
                    Nbt::Compound(vec![("value".into(), Nbt::Float(argument.value)), ("alpha".into(), Nbt::Float(argument.alpha))].into())
                }
            }
            (AttributeModifier::Color(_), Value::BlendToGray(argument)) => {
                Nbt::Compound(vec![("brightness".into(), Nbt::Float(argument.brightness)), ("factor".into(), Nbt::Float(argument.factor))].into())
            }
            (AttributeModifier::Color(_), Value::Argb(argument)) => Nbt::String(hex_color(argb::color_from_vector4f(*argument), 8)),
            (AttributeModifier::Color(_), Value::Rgb(argument)) => Nbt::String(hex_color(argb::transparent(argb::color_from_vector3f(*argument)), 6)),
            (modifier, argument) => return Err(format!("{modifier:?} cannot take {argument:?}")),
        })
    }

    pub fn parse_argument(self, attribute: &EnvironmentAttributeInfo, json: &Json) -> Result<Value, String> {
        match self {
            AttributeModifier::Override | AttributeModifier::Append | AttributeModifier::Overlay => attribute.parse_value(json),
            AttributeModifier::Boolean(_) => super::parse_bool(json).map(Value::Boolean),
            AttributeModifier::Float(m) => m.parse_argument(json),
            AttributeModifier::Integer(_) => super::parse_int(json).map(Value::Integer),
            AttributeModifier::Color(m) => m.parse_argument(json),
        }
    }

    pub fn argument_keyframe_lerp(self, attribute: &EnvironmentAttributeInfo) -> LerpFunction {
        match self {
            AttributeModifier::Override | AttributeModifier::Append | AttributeModifier::Overlay => attribute.ty.keyframe_lerp(),
            AttributeModifier::Boolean(_) => LerpFunction::CONSTANT,
            AttributeModifier::Float(m) => m.argument_keyframe_lerp(),
            AttributeModifier::Integer(_) => LerpFunction::Integer,
            AttributeModifier::Color(m) => m.argument_keyframe_lerp(),
        }
    }
}
