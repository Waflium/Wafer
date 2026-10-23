use super::validation_context::{self, ValidationContext};
use crate::core::holder::{Holder, RegistryElement};
use crate::util::problem_reporter::PathElement;
use super::parameters::loot_context_param_sets::ContextKeySet;
use crate::util::codec::DataResult;
use crate::util::problem_reporter::ProblemReporter;

pub trait Validatable {
    fn validate(&self, context: &ValidationContext);
}

pub fn validate<T: Validatable>(context: &ValidationContext, name: &str, v: &T) {
    v.validate(&context.for_field(name));
}

pub fn validate_optional<T: Validatable>(context: &ValidationContext, name: &str, optional: Option<&T>) {
    if let Some(v) = optional {
        v.validate(&context.for_field(name));
    }
}

pub fn validate_list<T: Validatable>(context: &ValidationContext, name: &str, list: &[T]) {
    for (i, v) in list.iter().enumerate() {
        v.validate(&context.for_indexed_field(name, i));
    }
}

pub fn validate_root_list<T: Validatable>(context: &ValidationContext, list: &[T]) {
    for (i, v) in list.iter().enumerate() {
        v.validate(&context.for_child(PathElement::Indexed(i)));
    }
}

pub fn validate_holder_in<T: RegistryElement + Validatable>(context: &ValidationContext, holder: &Holder<T>) {
    match holder {
        Holder::Direct(value) => value.validate(context),
        Holder::Reference(id, _) => {
            if context.has_visited_element(T::REGISTRY, id) {
                context.report_problem(validation_context::recursive_element_reference_problem(T::REGISTRY, id));
                return;
            }
            let element_context = context.enter_element(PathElement::ElementReference { id: id.clone(), registry: T::REGISTRY.to_string() }, T::REGISTRY, id);
            if let Some(value) = T::lookup(id) {
                value.validate(&element_context);
            }
        }
    }
}

pub fn validate_holder<T: RegistryElement + Validatable>(context: &ValidationContext, name: &str, holder: &Holder<T>) {
    validate_holder_in(&context.for_field(name), holder);
}

pub fn validate_optional_holder<T: RegistryElement + Validatable>(context: &ValidationContext, name: &str, optional: Option<&Holder<T>>) {
    if let Some(holder) = optional {
        validate_holder(context, name, holder);
    }
}

pub fn validate_holder_list<T: RegistryElement + Validatable>(context: &ValidationContext, name: &str, list: &[Holder<T>]) {
    validate_holder_refs(context, name, &list.iter().collect::<Vec<_>>());
}

pub fn validate_holder_refs<T: RegistryElement + Validatable>(context: &ValidationContext, name: &str, list: &[&Holder<T>]) {
    for (i, holder) in list.iter().enumerate() {
        let context = context.for_indexed_field(name, i);
        match *holder {
            Holder::Direct(value) => value.validate(&context),
            Holder::Reference(id, _) => {
                if let Some(value) = T::lookup(id) {
                    value.validate(&context);
                }
            }
        }
    }
}

fn validate_holder_set_in<T: RegistryElement + Validatable>(context: &ValidationContext, holder_set: &[Holder<T>]) {
    for (i, holder) in holder_set.iter().enumerate() {
        validate_holder_in(&context.for_child(PathElement::Indexed(i)), holder);
    }
}

pub fn validate_holder_set<T: RegistryElement + Validatable>(context: &ValidationContext, name: &str, holder_set: &[Holder<T>]) {
    validate_holder_set_in(&context.for_field(name), holder_set);
}

pub fn validate_holder_set_with_min_size<T: RegistryElement + Validatable>(context: &ValidationContext, name: &str, holder_set: &[Holder<T>], min_size: usize) {
    let named_context = context.for_field(name);
    validate_holder_set_in(&named_context, holder_set);
    if holder_set.len() < min_size {
        named_context.report_problem(validation_context::min_bounds_problem(min_size));
    }
}

pub fn validator_for_context<T: Validatable>(params: &'static ContextKeySet, v: &T) -> DataResult<()> {
    let problem_collector = ProblemReporter::default();
    v.validate(&ValidationContext::new(problem_collector.clone(), params));
    if problem_collector.is_empty() { Ok(()) } else { Err(format!("Validation error: {}", problem_collector.get_report())) }
}

pub fn list_validator_for_context<T: Validatable>(params: &'static ContextKeySet, list: &[T]) -> DataResult<()> {
    let problem_collector = ProblemReporter::default();
    validate_root_list(&ValidationContext::new(problem_collector.clone(), params), list);
    if problem_collector.is_empty() { Ok(()) } else { Err(format!("Validation error: {}", problem_collector.get_report())) }
}
