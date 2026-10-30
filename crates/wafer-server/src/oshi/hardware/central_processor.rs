use std::collections::HashMap;
use std::sync::OnceLock;

use super::super::OshiResult;
use crate::oshi::util::file_util::parse_properties;
use crate::oshi::util::parse_util::{java_trim, parse_hertz};

const OSHI_ARCHITECTURE_PROPERTIES: &str = include_str!("../oshi.architecture.properties");

fn architecture_properties() -> &'static HashMap<String, String> {
    static PROPERTIES: OnceLock<HashMap<String, String>> = OnceLock::new();
    PROPERTIES.get_or_init(|| parse_properties(OSHI_ARCHITECTURE_PROPERTIES))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LogicalProcessor {
    pub processor_number: i32,
    pub physical_processor_number: i32,
    pub physical_package_number: i32,
    pub numa_node: i32,
    pub processor_group: i32,
}

pub trait CentralProcessor {
    fn get_processor_identifier(&self) -> OshiResult<ProcessorIdentifier>;
    fn get_physical_package_count(&self) -> i32;
    fn get_physical_processor_count(&self) -> i32;
    fn get_logical_processor_count(&self) -> i32;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessorIdentifier {
    cpu_vendor: String,
    cpu_name: String,
    cpu_family: String,
    cpu_model: String,
    cpu_stepping: String,
    cpu_identifier: String,
    cpu_vendor_freq: i64,
}

impl ProcessorIdentifier {
    pub fn new(cpu_vendor: &str, cpu_name: &str, cpu_family: &str, cpu_model: &str, cpu_stepping: &str, cpu_64bit: bool, vendor_freq: i64) -> ProcessorIdentifier {
        let vendor = if cpu_vendor.starts_with("0x") { query_vendor_from_implementer(cpu_vendor) } else { cpu_vendor.to_string() };
        let mut identifier = String::new();
        if cpu_vendor == "GenuineIntel" {
            identifier.push_str(if cpu_64bit { "Intel64" } else { "x86" });
        } else {
            identifier.push_str(cpu_vendor);
        }
        identifier.push_str(" Family ");
        identifier.push_str(cpu_family);
        identifier.push_str(" Model ");
        identifier.push_str(cpu_model);
        identifier.push_str(" Stepping ");
        identifier.push_str(cpu_stepping);
        let cpu_vendor_freq = if vendor_freq > 0 {
            vendor_freq
        } else {
            match cpu_name.find("@ ") {
                Some(index) if !cpu_name[index + 2..].contains('\n') => parse_hertz(&cpu_name[index + 2..]),
                _ => -1,
            }
        };
        ProcessorIdentifier {
            cpu_vendor: vendor,
            cpu_name: cpu_name.to_string(),
            cpu_family: cpu_family.to_string(),
            cpu_model: cpu_model.to_string(),
            cpu_stepping: cpu_stepping.to_string(),
            cpu_identifier: identifier,
            cpu_vendor_freq,
        }
    }

    pub fn get_vendor(&self) -> &str {
        &self.cpu_vendor
    }

    pub fn get_name(&self) -> &str {
        &self.cpu_name
    }

    pub fn get_identifier(&self) -> &str {
        &self.cpu_identifier
    }

    pub fn get_vendor_freq(&self) -> i64 {
        self.cpu_vendor_freq
    }

    pub fn get_microarchitecture(&self) -> String {
        query_microarchitecture(&self.cpu_vendor, &self.cpu_name, &self.cpu_family, &self.cpu_model, &self.cpu_stepping)
    }
}

fn query_vendor_from_implementer(cpu_vendor: &str) -> String {
    architecture_properties().get(&format!("hw_impl.{cpu_vendor}")).cloned().unwrap_or_else(|| cpu_vendor.to_string())
}

fn query_microarchitecture(cpu_vendor: &str, cpu_name: &str, cpu_family: &str, cpu_model: &str, cpu_stepping: &str) -> String {
    let properties = architecture_properties();
    let mut arch: Option<String> = None;
    let mut key = String::new();
    let upper_vendor = cpu_vendor.to_uppercase();
    if upper_vendor.contains("AMD") {
        key.push_str("amd.");
    } else if upper_vendor.contains("ARM") {
        key.push_str("arm.");
    } else if upper_vendor.contains("IBM") {
        if let Some(power_index) = cpu_name.find("_POWER").filter(|&index| index > 0) {
            arch = Some(cpu_name[power_index + 1..].to_string());
        }
    } else if upper_vendor.contains("APPLE") {
        key.push_str("apple.");
    }
    let blank = |arch: &Option<String>| arch.as_deref().is_none_or(|arch| java_trim(arch).is_empty());
    if blank(&arch) && key != "arm." {
        key.push_str(cpu_family);
        arch = properties.get(&key).cloned();
    }
    if blank(&arch) {
        key.push('.');
        key.push_str(cpu_model);
        arch = properties.get(&key).cloned();
    }
    if blank(&arch) {
        key.push('.');
        key.push_str(cpu_stepping);
        arch = properties.get(&key).cloned();
    }
    if blank(&arch) { "unknown".to_string() } else { arch.unwrap_or_default() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn microarchitectures_resolve_like_oshi() {
        let zen3 = ProcessorIdentifier::new("AuthenticAMD", "AMD Ryzen 9 7950X3D 16-Core Processor          ", "25", "97", "2", true, 4_200_000_000);
        assert_eq!(zen3.get_microarchitecture(), "Zen 3");
        assert_eq!(zen3.get_identifier(), "AuthenticAMD Family 25 Model 97 Stepping 2");
        let zen2 = ProcessorIdentifier::new("AuthenticAMD", "AMD EPYC", "23", "49", "0", true, -1);
        assert_eq!(zen2.get_microarchitecture(), "Zen 2");
        let raptor = ProcessorIdentifier::new("GenuineIntel", "13th Gen Intel(R) Core(TM) i9-13900K", "6", "183", "1", true, -1);
        assert_eq!(raptor.get_microarchitecture(), "Raptor Lake");
        assert_eq!(raptor.get_identifier(), "Intel64 Family 6 Model 183 Stepping 1");
        let coffee = ProcessorIdentifier::new("GenuineIntel", "Intel", "6", "158", "10", false, -1);
        assert_eq!(coffee.get_microarchitecture(), "Coffee Lake");
        assert_eq!(coffee.get_identifier(), "x86 Family 6 Model 158 Stepping 10");
        let unknown = ProcessorIdentifier::new("GenuineIntel", "Intel", "99", "1", "1", true, -1);
        assert_eq!(unknown.get_microarchitecture(), "unknown");
        let arm = ProcessorIdentifier::new("0x41", "Neoverse", "8", "0xd0c", "r3p1", true, -1);
        assert_eq!(arm.get_vendor(), "ARM");
        assert_eq!(arm.get_microarchitecture(), "unknown");
    }

    #[test]
    fn vendor_frequency_comes_from_the_name_when_unknown() {
        let intel = ProcessorIdentifier::new("GenuineIntel", "Intel(R) Core(TM) i7-8700 CPU @ 3.20GHz", "6", "158", "10", true, 0);
        assert_eq!(intel.get_vendor_freq(), 3_200_000_000);
        let none = ProcessorIdentifier::new("AuthenticAMD", "AMD Ryzen", "25", "97", "2", true, -1);
        assert_eq!(none.get_vendor_freq(), -1);
    }
}
