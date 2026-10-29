use crate::util::valueproviders::int_provider_codec_in;
use crate::world::item::component::codec::Field;
use crate::world::item::component::types::req;

pub fn fields() -> Vec<Field> {
    vec![req("count", int_provider_codec_in::<0, 256>())]
}
