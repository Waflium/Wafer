use crate::world::item::component::codec::Field;
use crate::world::item::component::types::req;
use crate::world::level::levelgen::synth;

pub fn fields() -> Vec<Field> {
    vec![req("noise", synth::noise_holder_codec())]
}
