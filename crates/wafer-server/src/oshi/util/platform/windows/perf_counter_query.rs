use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, PoisonError};

use super::perf_counter_query_handler::PerfCounterQueryHandler;
use super::perf_data_util;
use super::wmi_query_handler::{self, WmiValue};
use crate::jna::platform::win32::advapi32_util::RegistryError;
use crate::jna::platform::win32::pdh_util;

pub struct PdhCounterProperty {
    pub instance: &'static str,
    pub counter: &'static str,
    pub wmi_property: &'static str,
}

static FAILED_QUERY_CACHE: Mutex<Option<HashSet<String>>> = Mutex::new(None);
static LOCALIZE_CACHE: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);

pub fn query_values(properties: &[PdhCounterProperty], perf_object: &str, perf_wmi_class: &str) -> Vec<i64> {
    let failed = FAILED_QUERY_CACHE.lock().unwrap_or_else(PoisonError::into_inner).get_or_insert_with(HashSet::new).contains(perf_object);
    if !failed {
        let values = query_values_from_pdh(properties, perf_object);
        if !values.is_empty() {
            return values;
        }
        info!("Disabling further attempts to query {perf_object}.");
        FAILED_QUERY_CACHE.lock().unwrap_or_else(PoisonError::into_inner).get_or_insert_with(HashSet::new).insert(perf_object.to_string());
    }
    query_values_from_wmi(properties, perf_wmi_class)
}

pub fn query_values_from_pdh(properties: &[PdhCounterProperty], perf_object: &str) -> Vec<i64> {
    let perf_object_localized = localize_if_needed(perf_object, false);
    let mut handler = PerfCounterQueryHandler::new();
    let mut counters = Vec::with_capacity(properties.len());
    for property in properties {
        let counter = perf_data_util::create_counter(&perf_object_localized, Some(property.instance), property.counter);
        if !handler.add_counter_to_query(&counter) {
            return Vec::new();
        }
        counters.push(counter);
    }
    if handler.update_query() > 0 { counters.iter().map(|counter| handler.query_counter(counter)).collect() } else { Vec::new() }
}

pub fn query_values_from_wmi(properties: &[PdhCounterProperty], wmi_class: &str) -> Vec<i64> {
    let names: Vec<&str> = properties.iter().map(|property| property.wmi_property).collect();
    let result = wmi_query_handler::query_wmi(wmi_class, &names);
    match result.rows.first() {
        Some(row) => row
            .iter()
            .map(|value| match value {
                WmiValue::Int(value) => i64::from(*value as u32),
                WmiValue::String(value) => value.parse().unwrap_or(0),
                WmiValue::Null => 0,
            })
            .collect(),
        None => Vec::new(),
    }
}

pub fn localize_if_needed(perf_object: &str, force: bool) -> String {
    if !force {
        return perf_object.to_string();
    }
    if let Some(cached) = LOCALIZE_CACHE.lock().unwrap_or_else(PoisonError::into_inner).get_or_insert_with(HashMap::new).get(perf_object) {
        return cached.clone();
    }
    let localized = localize_using_perf_index(perf_object);
    LOCALIZE_CACHE.lock().unwrap_or_else(PoisonError::into_inner).get_or_insert_with(HashMap::new).insert(perf_object.to_string(), localized.clone());
    localized
}

fn localize_using_perf_index(perf_object: &str) -> String {
    let mut localized = perf_object.to_string();
    match pdh_util::pdh_lookup_perf_index_by_english_name(perf_object) {
        Ok(index) => {
            if let Ok(name) = pdh_util::pdh_lookup_perf_name_by_index(index) {
                localized = name;
            }
        }
        Err(RegistryError::Win32(exception)) => {
            warn!(
                "Unable to locate English counter names in registry Perflib 009. Assuming English counters. Error 0x{:x}. {}",
                exception.get_hr(),
                "See https://support.microsoft.com/en-us/help/300956/how-to-manually-rebuild-performance-counter-library-values"
            );
        }
        Err(RegistryError::Other(_)) => {}
    }
    if localized.is_empty() { perf_object.to_string() } else { localized }
}
