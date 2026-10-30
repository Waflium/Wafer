use std::sync::Arc;

use super::metric_category::MetricCategory;
use super::metric_sampler::MetricSampler;
use crate::util::java_collections::JavaHashMap;

#[derive(Clone)]
pub struct ProfilerMeasured {
    pub name: String,
    pub category: MetricCategory,
    pub sampler: Arc<dyn Fn() -> f64 + Send + Sync>,
}

pub fn get_registered_samplers(measured_instances: Vec<ProfilerMeasured>) -> Vec<MetricSampler> {
    let mut samplers_by_name: JavaHashMap<Vec<ProfilerMeasured>> = JavaHashMap::new(wafer_core::random::java_string_hash);
    for measured in measured_instances {
        match samplers_by_name.get_mut(&measured.name) {
            Some(group) => group.push(measured),
            None => {
                samplers_by_name.insert(measured.name.clone(), vec![measured]);
            }
        }
    }
    aggregate_duplicates(samplers_by_name)
}

fn aggregate_duplicates(potential_duplicates: JavaHashMap<Vec<ProfilerMeasured>>) -> Vec<MetricSampler> {
    potential_duplicates
        .iter()
        .map(|(name, duplicate_samplers)| {
            let category = duplicate_samplers[0].category;
            let delegates: Vec<Arc<dyn Fn() -> f64 + Send + Sync>> = duplicate_samplers.iter().map(|measured| measured.sampler.clone()).collect();
            if delegates.len() > 1 {
                let name_hash = wafer_core::random::java_string_hash(name);
                let delegates_hash = delegates.iter().fold(1i32, |hash, _| hash.wrapping_mul(31).wrapping_add(name_hash));
                let hash_code = 31i32.wrapping_mul(31i32.wrapping_add(name_hash)).wrapping_add(delegates_hash);
                MetricSampler::create(name, category, move || average_value_from_delegates(&delegates)).with_hash_code(hash_code)
            } else {
                let delegate = delegates[0].clone();
                MetricSampler::create(name, category, move || delegate())
            }
        })
        .collect()
}

fn average_value_from_delegates(delegates: &[Arc<dyn Fn() -> f64 + Send + Sync>]) -> f64 {
    let mut aggregated_value = 0.0;
    for delegate in delegates {
        aggregated_value += delegate();
    }
    aggregated_value / delegates.len() as f64
}
