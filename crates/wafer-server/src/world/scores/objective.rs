use wafer_protocol::nbt::Nbt;

use super::criteria::{ObjectiveCriteria, RenderType};
use crate::network::chat::numbers::NumberFormat;
use crate::network::chat::{Component, HoverEvent, component_serialization};
use crate::util::codec::{self, DecodeResult, Record};

pub type ObjectiveId = u32;

pub struct Objective {
    pub(super) id: ObjectiveId,
    pub(super) name: String,
    pub(super) criteria: ObjectiveCriteria,
    pub(super) display_name: Component,
    pub(super) formatted_display_name: Component,
    pub(super) render_type: RenderType,
    pub(super) display_auto_update: bool,
    pub(super) number_format: Option<NumberFormat>,
}

impl Objective {
    pub(super) fn new(
        id: ObjectiveId,
        name: &str,
        criteria: ObjectiveCriteria,
        display_name: Component,
        render_type: RenderType,
        display_auto_update: bool,
        number_format: Option<NumberFormat>,
    ) -> Objective {
        let formatted_display_name = create_formatted_display_name(name, &display_name);
        Objective { id, name: name.to_string(), criteria, display_name, formatted_display_name, render_type, display_auto_update, number_format }
    }

    pub fn id(&self) -> ObjectiveId {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn criteria(&self) -> &ObjectiveCriteria {
        &self.criteria
    }

    pub fn display_name(&self) -> &Component {
        &self.display_name
    }

    pub fn formatted_display_name(&self) -> Component {
        self.formatted_display_name.clone()
    }

    pub fn render_type(&self) -> RenderType {
        self.render_type
    }

    pub fn display_auto_update(&self) -> bool {
        self.display_auto_update
    }

    pub fn number_format(&self) -> Option<&NumberFormat> {
        self.number_format.as_ref()
    }

    pub fn number_format_or_default(&self, default: NumberFormat) -> NumberFormat {
        self.number_format.clone().unwrap_or(default)
    }

    pub(super) fn set_display_name(&mut self, name: Component) {
        self.formatted_display_name = create_formatted_display_name(&self.name, &name);
        self.display_name = name;
    }

    pub fn pack(&self) -> Nbt {
        let mut fields = vec![("Name".to_string(), Nbt::String(self.name.clone()))];
        if self.criteria != ObjectiveCriteria::DUMMY {
            fields.push(("CriteriaName".into(), Nbt::String(self.criteria.name().into())));
        }
        fields.push(("DisplayName".into(), self.display_name.to_nbt()));
        if self.render_type != RenderType::Integer {
            fields.push(("RenderType".into(), Nbt::String(self.render_type.id().into())));
        }
        if self.display_auto_update {
            fields.push(("display_auto_update".into(), Nbt::Byte(1)));
        }
        if let Some(format) = &self.number_format {
            fields.push(("format".into(), format.to_nbt()));
        }
        Nbt::Compound(fields)
    }
}

fn create_formatted_display_name(name: &str, display_name: &Component) -> Component {
    let inner = display_name.clone().with_hover(HoverEvent::ShowText(Box::new(Component::literal(name))));
    Component::translatable_with("chat.square_brackets", vec![inner.into()])
}

pub struct PackedObjective {
    pub name: String,
    pub criteria: ObjectiveCriteria,
    pub display_name: Component,
    pub render_type: RenderType,
    pub display_auto_update: bool,
    pub number_format: Option<NumberFormat>,
}

impl PackedObjective {
    pub fn decode(tag: &Nbt) -> DecodeResult<PackedObjective> {
        if let Err(error) = codec::map(tag) {
            return DecodeResult::error(error);
        }
        let mut record = Record::default();
        let name = record.required(tag, "Name", |name| codec::string(name).map(str::to_string));
        let criteria = record.with_default(tag, "CriteriaName", ObjectiveCriteria::DUMMY, |criteria| codec::string(criteria).and_then(ObjectiveCriteria::from_codec));
        let display_name = record.required(tag, "DisplayName", component_serialization::decode_partial);
        let render_type = record.with_default(tag, "RenderType", RenderType::Integer, |render_type| codec::enum_by_name(render_type, RenderType::by_name));
        let display_auto_update = record.with_default(tag, "display_auto_update", false, codec::bool_value);
        let number_format = record.optional(tag, "format", NumberFormat::from_nbt);
        record.build(|| Some(PackedObjective { name: name?, criteria, display_name: display_name?, render_type, display_auto_update, number_format }))
    }
}
