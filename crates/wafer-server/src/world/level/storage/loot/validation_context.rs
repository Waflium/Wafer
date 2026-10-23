use std::rc::Rc;

use super::parameters::loot_context_param_sets::ContextKeySet;
use super::parameters::loot_context_params::LootContextParam;
use crate::util::problem_reporter::{PathElement, Problem, ProblemReporter};

#[derive(Clone)]
pub struct ValidationContext {
    reporter: ProblemReporter,
    context_key_set: &'static ContextKeySet,
    visited_objects: Rc<Vec<String>>,
}

impl ValidationContext {
    pub fn new(reporter: ProblemReporter, context_key_set: &'static ContextKeySet) -> ValidationContext {
        ValidationContext { reporter, context_key_set, visited_objects: Rc::default() }
    }

    pub fn for_child(&self, sub_context: PathElement) -> ValidationContext {
        ValidationContext { reporter: self.reporter.for_child(sub_context), context_key_set: self.context_key_set, visited_objects: self.visited_objects.clone() }
    }

    pub fn for_field(&self, name: &str) -> ValidationContext {
        self.for_child(PathElement::Field(name.to_string()))
    }

    pub fn for_indexed_field(&self, name: &str, index: usize) -> ValidationContext {
        self.for_child(PathElement::IndexedField(name.to_string(), index))
    }

    pub fn for_map_field(&self, name: &str, key: &str) -> ValidationContext {
        self.for_child(PathElement::MapEntry(name.to_string(), key.to_string()))
    }

    fn enter_object(&self, sub_context: PathElement, object: String) -> ValidationContext {
        let mut visited_objects = (*self.visited_objects).clone();
        visited_objects.push(object);
        ValidationContext { reporter: self.reporter.for_child(sub_context), context_key_set: self.context_key_set, visited_objects: Rc::new(visited_objects) }
    }

    pub fn enter_element(&self, sub_context: PathElement, registry: &str, id: &str) -> ValidationContext {
        self.enter_object(sub_context, format!("{registry} / {id}"))
    }

    pub fn has_visited_element(&self, registry: &str, id: &str) -> bool {
        self.visited_objects.contains(&format!("{registry} / {id}"))
    }

    pub fn enter_tag(&self, sub_context: PathElement, registry: &str, tag: &str) -> ValidationContext {
        self.enter_object(sub_context, format!("{registry} / #{tag}"))
    }

    pub fn has_visited_tag(&self, registry: &str, tag: &str) -> bool {
        self.visited_objects.contains(&format!("{registry} / #{tag}"))
    }

    pub fn report_problem(&self, problem: Problem) {
        self.reporter.report(problem);
    }

    pub fn validate_context_usage(&self, referenced: &[LootContextParam]) {
        let not_provided: Vec<String> =
            referenced.iter().filter(|&&param| !self.context_key_set.allows(param)).map(|param| format!("<parameter {}>", param.name())).collect();
        if !not_provided.is_empty() {
            self.reporter.report(parameters_not_provided_problem(&not_provided));
        }
    }

    pub fn reporter(&self) -> &ProblemReporter {
        &self.reporter
    }
}

fn parameters_not_provided_problem(not_provided: &[String]) -> Problem {
    Problem::new(format!("Parameters [{}] are not provided in this context", not_provided.join(", ")))
}

pub fn min_bounds_problem(min_bounds: usize) -> Problem {
    Problem::new(format!("List must contain at least {min_bounds} element(s)"))
}

pub fn recursive_element_reference_problem(registry: &str, id: &str) -> Problem {
    Problem::fatal(format!("{id} of type {registry} is recursively called"))
}

pub fn recursive_tag_reference_problem(registry: &str, tag: &str) -> Problem {
    Problem::fatal(format!("#{tag} of type {registry} is recursively called"))
}
