use wafer_protocol::Writer;
use wafer_protocol::nbt::Nbt;

use super::advancement_holder::AdvancementHolder;
use super::advancement_requirements::AdvancementRequirements;
use super::advancement_rewards::AdvancementRewards;
use super::display_info::DisplayInfo;
use super::triggers::criterion::Criterion;
use crate::network::chat::component_utils;
use crate::network::chat::{Component, HoverEvent, Style};
use crate::util::codec::{self, DataResult, DecodeResult, Record};
use crate::util::problem_reporter::{PathElement, ProblemReporter};
use crate::world::level::storage::loot::validation_context_source::ValidationContextSource;

pub struct Advancement {
    pub parent: Option<String>,
    pub display: Option<DisplayInfo>,
    pub rewards: AdvancementRewards,
    pub criteria: Vec<(String, Criterion)>,
    pub requirements: AdvancementRequirements,
    pub sends_telemetry_event: bool,
    pub name: Option<Component>,
}

impl Advancement {
    pub fn validate(&self, reporter: &ProblemReporter) {
        for (name, criterion) in &self.criteria {
            let mut validator = ValidationContextSource::new(reporter.for_child(PathElement::RootField(name.clone())));
            criterion.trigger_instance.validate(&mut validator);
        }
    }

    pub fn decode(tag: &Nbt) -> DataResult<Advancement> {
        Advancement::decode_partial(tag).into_result()
    }

    fn decode_partial(tag: &Nbt) -> DecodeResult<Advancement> {
        if let Err(error) = codec::map(tag) {
            return DecodeResult::error(error);
        }
        let mut record = Record::default();
        let parent = record.optional(tag, "parent", |parent| DecodeResult::from(codec::identifier(parent)));
        let display = record.optional(tag, "display", DisplayInfo::decode_partial);
        let rewards = record.with_default(tag, "rewards", AdvancementRewards::default(), AdvancementRewards::decode_partial);
        let criteria = record.required(tag, "criteria", |criteria| {
            codec::base_map_partial(criteria, |name| Ok(name.to_string()), |criterion| DecodeResult::from(Criterion::decode(criterion)))
                .validate(|criteria| if criteria.is_empty() { Err("Advancement criteria cannot be empty".into()) } else { Ok(()) })
        });
        let requirements = record.optional(tag, "requirements", |requirements| DecodeResult::from(AdvancementRequirements::decode(requirements)));
        let sends_telemetry_event = record.with_default(tag, "sends_telemetry_event", false, |sends| DecodeResult::from(codec::bool_value(sends)));
        record
            .build(|| {
                let criteria = criteria?;
                let requirements = requirements.unwrap_or_else(|| AdvancementRequirements::all_of(criteria.iter().map(|(name, _)| name.as_str())));
                let name = display.as_ref().map(Advancement::decorate_name);
                Some(Advancement { parent, display, rewards, criteria, requirements, sends_telemetry_event, name })
            })
            .validate(Advancement::validate_codec)
    }

    fn validate_codec(&self) -> DataResult<()> {
        let is_root = self.parent.is_none();
        let is_visible = self.display.is_some();
        let has_background = self.display.as_ref().is_some_and(|display| display.background.is_some());
        if is_root && is_visible && !has_background {
            return Err("Visible advancement roots must have background".into());
        }
        if !is_root && has_background {
            return Err("Only advancement roots can have background".into());
        }
        let names: Vec<&str> = self.criteria.iter().map(|(name, _)| name.as_str()).collect();
        self.requirements.validate(&names)
    }

    pub fn write(&self, output: &mut Writer) {
        output.bool(self.parent.is_some());
        if let Some(parent) = &self.parent {
            output.string(parent);
        }
        output.bool(self.display.is_some());
        if let Some(display) = &self.display {
            display.serialize_to_network(output);
        }
        self.requirements.write(output);
        output.bool(self.sends_telemetry_event);
    }

    fn decorate_name(display: &DisplayInfo) -> Component {
        let display_title = &display.title;
        let color = display.ty.get_chat_color();
        let tooltip = component_utils::merge_styles(display_title.clone(), &Style { color: Some(color.color()), ..Style::default() })
            .append(Component::literal("\n"))
            .append(display.description.clone());
        let mut title = display_title.clone();
        title.style = Style { hover_event: Some(HoverEvent::ShowText(Box::new(tooltip))), ..Style::default() }.apply_to(&title.style);
        component_utils::wrap_in_square_brackets(title).with_color(color)
    }

    pub fn name(holder: &AdvancementHolder) -> Component {
        holder.value.name.clone().unwrap_or_else(|| Component::literal(holder.id.clone()))
    }

    pub fn is_root(&self) -> bool {
        self.parent.is_none()
    }

    pub fn criterion(&self, name: &str) -> Option<&Criterion> {
        self.criteria.iter().find(|(criterion, _)| criterion == name).map(|(_, criterion)| criterion)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode_error(json: &str) -> String {
        let value: serde_json::Value = serde_json::from_str(json).unwrap();
        crate::nbt::with_json_source(|| Advancement::decode(&crate::world::item::component::json_to_nbt(&value)).err().expect("a decode error"))
    }

    #[test]
    fn a_criteria_map_without_valid_entries_is_empty() {
        assert_eq!(
            decode_error(r#"{"criteria":{"a":null}}"#),
            r#"Not a JSON object: null missed input: {"a":null}; Advancement criteria cannot be empty"#
        );
    }

    #[test]
    fn a_failing_display_keeps_its_defaults_and_is_validated() {
        let json = r#"{"display":{"icon":{"id":"minecraft:stone"},"title":"t","description":"d","frame":"nope"},"criteria":{"a":{"trigger":"minecraft:impossible"}}}"#;
        assert_eq!(decode_error(json), "Unknown element name:nope; Visible advancement roots must have background");
    }

    #[test]
    fn every_field_error_is_reported() {
        let error = decode_error(r#"{"parent":5,"display":"x","rewards":5,"criteria":{},"requirements":"x","sends_telemetry_event":"x"}"#);
        assert!(error.starts_with("Not a boolean: \"x\"; "), "{error}");
        assert!(error.contains("Advancement criteria cannot be empty"), "{error}");
        assert!(error.ends_with("Not a string: 5"), "{error}");
    }
}
