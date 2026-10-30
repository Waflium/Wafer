use std::collections::HashSet;
use std::sync::{Mutex, PoisonError};

use super::perf_counter_query::localize_if_needed;
use super::perf_counter_query_handler::PerfCounterQueryHandler;
use super::perf_data_util;
use super::wmi_query_handler::{self, WmiValue};
use crate::jna::platform::win32::pdh_util;
use crate::oshi::util::util::wildcard_match;

static FAILED_QUERY_CACHE: Mutex<Option<HashSet<String>>> = Mutex::new(None);

pub struct InstancesAndValues {
    pub instances: Vec<String>,
    pub values: Vec<Vec<i64>>,
}

impl InstancesAndValues {
    fn empty() -> InstancesAndValues {
        InstancesAndValues { instances: Vec::new(), values: Vec::new() }
    }
}

pub struct WildcardProperties {
    pub counters: &'static [&'static str],
    pub wmi_properties: &'static [&'static str],
}

pub fn query_instances_and_values(properties: &WildcardProperties, perf_object: &str, perf_wmi_class: &str, custom_filter: Option<&str>) -> InstancesAndValues {
    let failed = FAILED_QUERY_CACHE.lock().unwrap_or_else(PoisonError::into_inner).get_or_insert_with(HashSet::new).contains(perf_object);
    if !failed {
        let instances_and_values = query_instances_and_values_from_pdh(properties, perf_object, custom_filter);
        if !instances_and_values.instances.is_empty() {
            return instances_and_values;
        }
        if custom_filter.is_none_or(str::is_empty) {
            info!("Disabling further attempts to query {perf_object}.");
            FAILED_QUERY_CACHE.lock().unwrap_or_else(PoisonError::into_inner).get_or_insert_with(HashSet::new).insert(perf_object.to_string());
        }
    }
    query_instances_and_values_from_wmi(properties, perf_wmi_class)
}

pub fn query_instances_and_values_from_pdh(properties: &WildcardProperties, perf_object: &str, custom_filter: Option<&str>) -> InstancesAndValues {
    let instance_filter = match custom_filter {
        Some(filter) if !filter.is_empty() => filter.to_string(),
        _ => properties.counters[0].to_lowercase(),
    };
    let perf_object_localized = localize_if_needed(perf_object, true);
    let object_items = match pdh_util::pdh_enum_object_items(&perf_object_localized, 100) {
        Ok(items) => items,
        Err(exception) => {
            let message = exception.throwable.message().unwrap_or_default().to_string();
            warn!("Failed to locate performance object for {perf_object_localized} in the registry. Performance counters may be corrupt. {message}");
            return InstancesAndValues::empty();
        }
    };
    let mut instances = object_items.instances;
    instances.retain(|instance| wildcard_match(&instance.to_lowercase(), &instance_filter));
    let mut handler = PerfCounterQueryHandler::new();
    let mut counter_lists = Vec::with_capacity(properties.counters.len() - 1);
    for counter_name in &properties.counters[1..] {
        let mut counter_list = Vec::with_capacity(instances.len());
        for instance in &instances {
            let counter = perf_data_util::create_counter(perf_object, Some(instance), counter_name);
            if !handler.add_counter_to_query(&counter) {
                return InstancesAndValues::empty();
            }
            counter_list.push(counter);
        }
        counter_lists.push(counter_list);
    }
    let mut values = Vec::new();
    if handler.update_query() > 0 {
        for counter_list in &counter_lists {
            values.push(counter_list.iter().map(|counter| handler.query_counter(counter)).collect());
        }
    }
    InstancesAndValues { instances, values }
}

pub fn query_instances_and_values_from_wmi(properties: &WildcardProperties, wmi_class: &str) -> InstancesAndValues {
    let result = wmi_query_handler::query_wmi(wmi_class, properties.wmi_properties);
    let mut instances_and_values = InstancesAndValues::empty();
    if result.get_result_count() > 0 {
        instances_and_values.instances = (0..result.get_result_count()).map(|row| result.get_string(row, 0)).collect();
        for column in 1..properties.wmi_properties.len() {
            instances_and_values.values.push(
                result
                    .rows
                    .iter()
                    .map(|row| match &row[column] {
                        WmiValue::Int(value) => i64::from(*value as u32),
                        WmiValue::String(value) => value.parse().unwrap_or(0),
                        WmiValue::Null => 0,
                    })
                    .collect(),
            );
        }
    }
    instances_and_values
}
