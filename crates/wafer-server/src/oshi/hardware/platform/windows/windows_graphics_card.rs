use crate::jna::platform::win32::advapi32_util::{
    ROOT_LOCAL_MACHINE, RegistryError, RegistryValue, registry_get_keys, registry_get_long_value, registry_get_string_value, registry_get_value, registry_value_exists,
};
use crate::oshi::OshiResult;
use crate::oshi::driver::windows::wmi::win32_video_controller;
use crate::oshi::hardware::GraphicsCard;
use crate::oshi::util::parse_util::parse_device_id_to_vendor_product_serial;

const ADAPTER_STRING: &str = "HardwareInformation.AdapterString";
const DRIVER_DESC: &str = "DriverDesc";
const DRIVER_VERSION: &str = "DriverVersion";
const VENDOR: &str = "ProviderName";
const QW_MEMORY_SIZE: &str = "HardwareInformation.qwMemorySize";
const MEMORY_SIZE: &str = "HardwareInformation.MemorySize";
const DISPLAY_DEVICES_REGISTRY_PATH: &str = "SYSTEM\\CurrentControlSet\\Control\\Class\\{4d36e968-e325-11ce-bfc1-08002be10318}\\";
const ERROR_ACCESS_DENIED: i32 = 5;

fn or_unknown(value: String) -> String {
    if value.is_empty() { "unknown".to_string() } else { value }
}

pub fn byte_array_to_long_little_endian(bytes: &[u8], size: usize) -> i64 {
    let mut value = 0i64;
    for (index, byte) in bytes.iter().take(size.min(8)).enumerate() {
        value |= i64::from(*byte) << (8 * index);
    }
    value
}

fn read_card(full_key: &str, index: &mut i32) -> Result<Option<GraphicsCard>, RegistryError> {
    if !registry_value_exists(ROOT_LOCAL_MACHINE, full_key, ADAPTER_STRING)? {
        return Ok(None);
    }
    let name = registry_get_string_value(ROOT_LOCAL_MACHINE, full_key, DRIVER_DESC)?;
    let device_id = format!("VideoController{index}");
    *index += 1;
    let vendor = registry_get_string_value(ROOT_LOCAL_MACHINE, full_key, VENDOR)?;
    let version_info = registry_get_string_value(ROOT_LOCAL_MACHINE, full_key, DRIVER_VERSION)?;
    let mut vram = 0i64;
    if registry_value_exists(ROOT_LOCAL_MACHINE, full_key, QW_MEMORY_SIZE)? {
        vram = registry_get_long_value(ROOT_LOCAL_MACHINE, full_key, QW_MEMORY_SIZE)?;
    } else if registry_value_exists(ROOT_LOCAL_MACHINE, full_key, MEMORY_SIZE)? {
        match registry_get_value(ROOT_LOCAL_MACHINE, full_key, MEMORY_SIZE)? {
            RegistryValue::Long(value) => vram = value,
            RegistryValue::Int(value) => vram = i64::from(value as u32),
            RegistryValue::Binary(bytes) => vram = byte_array_to_long_little_endian(&bytes, bytes.len()),
            RegistryValue::None => {}
        }
    }
    Ok(Some(GraphicsCard::new(or_unknown(name), or_unknown(device_id), or_unknown(vendor), or_unknown(version_info), vram)))
}

pub fn get_graphics_cards() -> OshiResult<Vec<GraphicsCard>> {
    let mut card_list = Vec::new();
    let mut index = 1;
    let keys = registry_get_keys(ROOT_LOCAL_MACHINE, DISPLAY_DEVICES_REGISTRY_PATH)?;
    for key in keys {
        if key.starts_with('0') {
            let full_key = format!("{DISPLAY_DEVICES_REGISTRY_PATH}{key}");
            match read_card(&full_key, &mut index) {
                Ok(Some(card)) => card_list.push(card),
                Ok(None) => {}
                Err(RegistryError::Win32(exception)) if exception.error_code == ERROR_ACCESS_DENIED => {}
                Err(error) => return Err(error.into()),
            }
        }
    }
    Ok(if card_list.is_empty() { get_graphics_cards_from_wmi() } else { card_list })
}

fn get_graphics_cards_from_wmi() -> Vec<GraphicsCard> {
    let cards = win32_video_controller::query_video_controller();
    let mut card_list = Vec::new();
    for index in 0..cards.get_result_count() {
        let name = cards.get_string(index, win32_video_controller::NAME);
        let id_pair = parse_device_id_to_vendor_product_serial(&cards.get_string(index, win32_video_controller::PNPDEVICEID));
        let mut device_id = id_pair.as_ref().map_or_else(|| "unknown".to_string(), |pair| pair.1.clone());
        let mut vendor = cards.get_string(index, win32_video_controller::ADAPTERCOMPATIBILITY);
        if let Some(pair) = &id_pair {
            if vendor.is_empty() {
                device_id = pair.0.clone();
            } else {
                vendor = format!("{vendor} ({})", pair.0);
            }
        }
        let version_info = cards.get_string(index, win32_video_controller::DRIVERVERSION);
        let version_info = if version_info.is_empty() { "unknown".to_string() } else { format!("DriverVersion={version_info}") };
        let vram = i64::from(cards.get_int(index, win32_video_controller::ADAPTERRAM) as u32);
        card_list.push(GraphicsCard::new(or_unknown(name), device_id, or_unknown(vendor), version_info, vram));
    }
    card_list
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binary_memory_sizes_are_little_endian() {
        assert_eq!(byte_array_to_long_little_endian(&[0x00, 0x00, 0x00, 0x40], 4), 0x4000_0000);
        assert_eq!(byte_array_to_long_little_endian(&[0x01, 0x02], 2), 0x0201);
    }

    #[test]
    fn this_machine_lists_graphics_cards() {
        let cards = get_graphics_cards().ok().unwrap();
        assert!(cards.iter().all(|card| !card.get_name().is_empty() && card.get_device_id().starts_with("VideoController")));
    }
}
