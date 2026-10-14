use wafer_protocol::Writer;
use wafer_protocol::nbt::Nbt;

use super::particle_types;

#[derive(Clone, Debug, PartialEq)]
pub struct ParticleOptions {
    pub particle_type: &'static str,
    pub options: Nbt,
}

impl ParticleOptions {
    pub fn get_type(&self) -> &'static str {
        self.particle_type
    }

    pub fn write(&self, w: &mut Writer) {
        w.varint(wafer_data::registry_id(wafer_data::registry::PARTICLE_TYPE, self.particle_type));
        self.write_options(w);
    }

    pub fn write_options(&self, w: &mut Writer) {
        let (codec, stream) = particle_types::codec(self.particle_type);
        crate::world::item::component::codec::write(codec, stream, &self.options, w).expect("decoded particle options");
    }
}
