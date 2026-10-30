use wafer_core::BlockPos;
use wafer_protocol::nbt::Nbt;

use super::map_decoration_type::MapDecorationTypeHolder;
use super::map_decoration_types::WHITE_BANNER;
use crate::network::chat::component_serialization;
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DecodeResult, Record};
use crate::util::extra_codecs;
use crate::world::item::dye_color::DyeColor;
use crate::world::level::block::abstract_banner_block;
use crate::world::level::block::entity::BlockEntityKind;

#[derive(Clone, PartialEq, Debug)]
pub struct MapBanner {
    pub pos: BlockPos,
    pub color: DyeColor,
    pub name: Option<Nbt>,
}

impl MapBanner {
    pub fn decode(tag: &Nbt) -> DecodeResult<MapBanner> {
        if let Err(error) = codec::map(tag) {
            return DecodeResult::error(error);
        }
        let mut record = Record::default();
        let pos = record.required(tag, "pos", extra_codecs::block_pos);
        let color = tag.get("color").and_then(|color| codec::string(color).ok()).and_then(DyeColor::by_name).unwrap_or(DyeColor::VALUES[0]);
        let name = tag.get("name").and_then(|name| component_serialization::decode(name).ok()).map(|name| name.to_nbt());
        record.build(|| Some(MapBanner { pos: pos?, color, name }))
    }

    pub fn encode(&self) -> Nbt {
        let mut fields = vec![("pos".into(), Nbt::IntArray(vec![self.pos.x, self.pos.y, self.pos.z]))];
        if self.color != DyeColor::VALUES[0] {
            fields.push(("color".into(), Nbt::String(self.color.name.into())));
        }
        if let Some(name) = &self.name {
            fields.push(("name".into(), name.clone()));
        }
        Nbt::Compound(fields.into())
    }

    pub fn from_world(level: &ServerLevel, pos: BlockPos) -> Option<MapBanner> {
        let block_entity = level.chunks.block_entity_ref(pos)?;
        let BlockEntityKind::Banner(banner) = &block_entity.kind else { return None };
        let color = abstract_banner_block::get_color(block_entity.block_state().block());
        Some(MapBanner { pos, color, name: banner.get_custom_name().cloned() })
    }

    pub fn get_decoration(&self) -> MapDecorationTypeHolder {
        MapDecorationTypeHolder(WHITE_BANNER.0 + self.color.id as u16)
    }

    pub fn get_id(&self) -> String {
        format!("banner-{},{},{}", self.pos.x, self.pos.y, self.pos.z)
    }
}
