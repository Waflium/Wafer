use wafer_data::registry;
use wafer_protocol::Writer;
use wafer_protocol::nbt::Nbt;

#[derive(Clone, Debug, PartialEq)]
pub enum SoundEvent {
    Reference(i32),
    Direct { location: String, fixed_range: Option<f32> },
}

impl From<i32> for SoundEvent {
    fn from(id: i32) -> SoundEvent {
        SoundEvent::Reference(id)
    }
}

impl SoundEvent {
    pub fn from_nbt(tag: &Nbt) -> Option<SoundEvent> {
        match tag {
            Nbt::String(name) => {
                let name = crate::commands::arguments::parse_identifier(name)?;
                registry::SOUND_EVENT.iter().position(|&s| s == name).map(|id| SoundEvent::Reference(id as i32))
            }
            Nbt::Compound(_) => Some(SoundEvent::Direct {
                location: crate::commands::arguments::parse_identifier(tag.get("sound_id")?.as_str()?)?,
                fixed_range: tag.get("range").and_then(Nbt::as_f64).map(|range| range as f32),
            }),
            _ => None,
        }
    }

    pub fn write(&self, w: &mut Writer) {
        match self {
            SoundEvent::Reference(id) => w.varint(id + 1),
            SoundEvent::Direct { location, fixed_range } => {
                w.varint(0);
                w.string(location);
                w.bool(fixed_range.is_some());
                if let Some(range) = fixed_range {
                    w.f32(*range);
                }
            }
        }
    }

    pub fn get_range(&self, volume: f32) -> f32 {
        match self {
            SoundEvent::Direct { fixed_range: Some(range), .. } => *range,
            _ if volume > 1.0 => 16.0 * volume,
            _ => 16.0,
        }
    }
}
