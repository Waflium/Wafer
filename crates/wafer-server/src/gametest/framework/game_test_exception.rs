use wafer_core::BlockPos;

use crate::network::chat::{Arg, Component};

#[derive(Clone)]
pub enum GameTestException {
    Assert { message: Component, tick: i32 },
    AssertPos { message: Component, absolute_pos: BlockPos, relative_pos: BlockPos, tick: i32 },
    Timeout(Component),
    Unknown(String),
}

impl GameTestException {
    pub fn get_description(&self) -> Component {
        match self {
            GameTestException::Assert { message, tick } => Component::translatable_with("test.error.tick", vec![message.clone().into(), Arg::Int(*tick)]),
            GameTestException::AssertPos { message, absolute_pos, relative_pos, tick } => Component::translatable_with(
                "test.error.position",
                vec![
                    message.clone().into(),
                    Arg::Int(absolute_pos.x),
                    Arg::Int(absolute_pos.y),
                    Arg::Int(absolute_pos.z),
                    Arg::Int(relative_pos.x),
                    Arg::Int(relative_pos.y),
                    Arg::Int(relative_pos.z),
                    Arg::Int(*tick),
                ],
            ),
            GameTestException::Timeout(message) => message.clone(),
            GameTestException::Unknown(reason) => Component::translatable_with("test.error.unknown", vec![Arg::String(reason.clone())]),
        }
    }

    pub fn get_message(&self) -> String {
        match self {
            GameTestException::Assert { .. } | GameTestException::AssertPos { .. } => self.get_description().get_string(),
            GameTestException::Timeout(message) => message.get_string(),
            GameTestException::Unknown(reason) => reason.clone(),
        }
    }

    pub fn is_assert(&self) -> bool {
        matches!(self, GameTestException::Assert { .. } | GameTestException::AssertPos { .. })
    }
}
