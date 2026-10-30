use wafer_protocol::Writer;
use wafer_protocol::nbt::Nbt;

use super::advancement_type::AdvancementType;
use crate::network::chat::{Component, component_serialization};
use crate::util::codec::{self, DataResult, DecodeResult, Record};
use crate::world::item::component::values::ItemStackTemplate;
use crate::world::item::crafting::display::slot_display::write_item_stack_template;

#[derive(Debug)]
pub struct DisplayInfo {
    pub icon: ItemStackTemplate,
    pub title: Component,
    pub description: Component,
    pub background: Option<String>,
    pub ty: AdvancementType,
    pub show_toast: bool,
    pub announce_to_chat: bool,
    pub hidden: bool,
}

impl DisplayInfo {
    pub fn decode(tag: &Nbt) -> DataResult<DisplayInfo> {
        DisplayInfo::decode_partial(tag).into_result()
    }

    pub fn decode_partial(tag: &Nbt) -> DecodeResult<DisplayInfo> {
        codec::as_codec(|| {
            if let Err(error) = codec::map(tag) {
                return DecodeResult::error(error);
            }
            let mut record = Record::default();
            let icon = record.required(tag, "icon", ItemStackTemplate::decode_partial);
            let title = record.required(tag, "title", |title| DecodeResult::from(component_serialization::decode(title)));
            let description = record.required(tag, "description", |description| DecodeResult::from(component_serialization::decode(description)));
            let background = record.optional(tag, "background", |background| DecodeResult::from(codec::identifier(background)));
            let ty = record.with_default(tag, "frame", AdvancementType::Task, |frame| DecodeResult::from(AdvancementType::decode(frame)));
            let show_toast = record.with_default(tag, "show_toast", true, |show| DecodeResult::from(codec::bool_value(show)));
            let announce_to_chat = record.with_default(tag, "announce_to_chat", true, |announce| DecodeResult::from(codec::bool_value(announce)));
            let hidden = record.with_default(tag, "hidden", false, |hidden| DecodeResult::from(codec::bool_value(hidden)));
            record.build(|| Some(DisplayInfo { icon: icon?, title: title?, description: description?, background, ty, show_toast, announce_to_chat, hidden }))
        })
    }

    pub fn serialize_to_network(&self, output: &mut Writer) {
        self.title.write(output);
        self.description.write(output);
        write_item_stack_template(output, &self.icon);
        output.varint(self.ty.id());
        let mut flags = 0;
        if self.background.is_some() {
            flags |= 1;
        }
        if self.show_toast {
            flags |= 2;
        }
        if self.hidden {
            flags |= 4;
        }
        output.i32(flags);
        if let Some(background) = &self.background {
            output.string(background);
        }
    }
}
