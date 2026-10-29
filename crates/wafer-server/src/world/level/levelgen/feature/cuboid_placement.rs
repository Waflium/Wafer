pub fn fields() -> Vec<crate::world::item::component::codec::Field> {
    use crate::util::valueproviders::int_provider_codec_in;
    use crate::world::item::component::types::{self, b, def, req};
    vec![
        req("xz_size", int_provider_codec_in::<1, 16>()),
        req("y_size", int_provider_codec_in::<1, 16>()),
        def("include_edges", types::bool(), b(true)),
        def("include_interior", types::bool(), b(true)),
    ]
}
