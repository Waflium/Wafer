use super::parameters::loot_context_param_sets::{self, ContextKeySet};
use super::validation_context::ValidationContext;
use crate::util::problem_reporter::ProblemReporter;

pub struct ValidationContextSource {
    reporter: ProblemReporter,
    entity_context: Option<ValidationContext>,
}

impl ValidationContextSource {
    pub fn new(reporter: ProblemReporter) -> ValidationContextSource {
        ValidationContextSource { reporter, entity_context: None }
    }

    pub fn context(&self, params: &'static ContextKeySet) -> ValidationContext {
        ValidationContext::new(self.reporter.clone(), params)
    }

    pub fn entity_context(&mut self) -> &ValidationContext {
        if self.entity_context.is_none() {
            self.entity_context = Some(self.context(&loot_context_param_sets::ADVANCEMENT_ENTITY));
        }
        self.entity_context.as_ref().unwrap()
    }
}
