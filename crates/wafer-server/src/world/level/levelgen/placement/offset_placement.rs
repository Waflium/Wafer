use crate::util::valueproviders::int_provider_codec_in;
use crate::world::item::component::codec::Field;
use crate::world::item::component::types::req;

pub fn fields() -> Vec<Field> {
    vec![
        req("x", int_provider_codec_in::<-16, 16>()),
        req("y", int_provider_codec_in::<-16, 16>()),
        req("z", int_provider_codec_in::<-16, 16>()),
    ]
}
