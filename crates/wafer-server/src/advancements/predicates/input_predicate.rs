use wafer_protocol::nbt::Nbt;

use crate::server::network::movement::Input;
use crate::util::codec::{self, DataResult, DecodeResult};
use crate::world::item::component::types::{self, Pair, opt, record};

pub fn codec() -> Pair {
    record(
        ["forward", "backward", "left", "right", "jump", "sneak", "sprint"].into_iter().map(|key| opt(key, types::bool())).collect(),
    )
}

#[derive(Debug)]
pub struct InputPredicate {
    forward: Option<bool>,
    backward: Option<bool>,
    left: Option<bool>,
    right: Option<bool>,
    jump: Option<bool>,
    sneak: Option<bool>,
    sprint: Option<bool>,
}

impl InputPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<InputPredicate> {
        codec::as_codec(|| {
            if let Err(error) = codec::map(tag) {
                return DecodeResult::error(error);
            }
            let mut record = codec::Record::default();
            let mut flag = |key| record.optional(tag, key, codec::bool_value);
            let (forward, backward, left, right, jump, sneak, sprint) = (flag("forward"), flag("backward"), flag("left"), flag("right"), flag("jump"), flag("sneak"), flag("sprint"));
            record.build(|| Some(InputPredicate { forward, backward, left, right, jump, sneak, sprint }))
        })
        .into_result()
    }

    pub fn matches(&self, input: Input) -> bool {
        let matches = |expected: Option<bool>, value: bool| expected.is_none_or(|b| b == value);
        matches(self.forward, input.forward)
            && matches(self.backward, input.backward)
            && matches(self.left, input.left)
            && matches(self.right, input.right)
            && matches(self.jump, input.jump)
            && matches(self.sneak, input.shift)
            && matches(self.sprint, input.sprint)
    }
}
