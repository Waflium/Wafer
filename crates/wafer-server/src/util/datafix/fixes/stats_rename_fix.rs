use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dsl::ConstType;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::{field_finder, find_field, named_choice, type_finder};
use crate::datafixers::types::{Type, const_type};
use crate::util::datafix::fixes::references::{OBJECTIVE, STATS};

pub struct StatsRenameFix {
    base: DataFixBase,
    name: String,
    renames: &'static [(&'static str, &'static str)],
}

impl StatsRenameFix {
    pub fn new(output_schema: &Arc<Schema>, name: &str, renames: &'static [(&'static str, &'static str)]) -> StatsRenameFix {
        StatsRenameFix { base: DataFixBase::new(output_schema, false), name: name.to_string(), renames }
    }

    fn create_criteria_rule(&self) -> TypeRewriteRule {
        let output_type = self.get_output_schema().get_type(OBJECTIVE);
        let input_type = self.get_input_schema().get_type(OBJECTIVE);
        let criteria_type_finder = find_field(&input_type, "CriteriaType");
        let choice_type = find_choice_type(&criteria_type_finder.ty(), "type").unwrap_or_else(|| panic!("Can't find choice type for criteria"));
        let Some(custom_field_type) = choice_type.choices().get("minecraft:custom").cloned() else { panic!("Failed to find custom criterion type variant") };
        let custom_type_finder = named_choice("minecraft:custom", custom_field_type);
        let id_finder = field_finder("id", const_type(ConstType::NamespacedString));
        let renames = self.renames;
        self.fix_type_everywhere_typed_to(&self.name, input_type, output_type, move |input| {
            input.update_typed(&criteria_type_finder, None, |stats| {
                stats.update_typed(&custom_type_finder, None, |custom| custom.update(&id_finder, |value| value.map_string(|value| rename(renames, value))))
            })
        })
    }

    fn create_stat_rule(&self) -> TypeRewriteRule {
        let output_type = self.get_output_schema().get_type(STATS);
        let input_type = self.get_input_schema().get_type(STATS);
        let stats_finder = find_field(&input_type, "stats");
        let custom_finder = find_field(&stats_finder.ty(), "minecraft:custom");
        let name_finder = type_finder(const_type(ConstType::NamespacedString));
        let renames = self.renames;
        self.fix_type_everywhere_typed_to(&self.name, input_type, output_type, move |input| {
            input.update_typed(&stats_finder, None, |stats| {
                stats.update_typed(&custom_finder, None, |custom| custom.update(&name_finder, |value| value.map_string(|value| rename(renames, value))))
            })
        })
    }
}

impl DataFix for StatsRenameFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        TypeRewriteRule::seq(vec![self.create_stat_rule(), self.create_criteria_rule()])
    }
}

fn rename(renames: &[(&str, &str)], value: &str) -> String {
    renames.iter().find(|(from, _)| *from == value).map_or(value, |(_, to)| to).to_string()
}

fn find_choice_type(ty: &Arc<Type>, name: &str) -> Option<Arc<Type>> {
    match &**ty {
        Type::TaggedChoice(choice) if &*choice.name == name => Some(ty.clone()),
        Type::Named(_, element) | Type::Field(_, element) | Type::Hook(element, _, _) | Type::List(element) => find_choice_type(element, name),
        Type::Product(first, second) | Type::Sum(first, second) => find_choice_type(first, name).or_else(|| find_choice_type(second, name)),
        _ => None,
    }
}
