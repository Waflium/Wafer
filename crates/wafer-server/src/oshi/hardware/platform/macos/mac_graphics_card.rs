use crate::oshi::hardware::GraphicsCard;
use crate::oshi::util::executing_command::run_native;
use crate::oshi::util::parse_util::{java_trim, parse_decimal_memory_size_to_binary};

pub fn graphics_cards_from(sp: &[String]) -> Vec<GraphicsCard> {
    let mut card_list = Vec::new();
    let mut name = "unknown".to_string();
    let mut device_id = "unknown".to_string();
    let mut vendor = "unknown".to_string();
    let mut version_info_list: Vec<String> = Vec::new();
    let mut vram = 0;
    let mut card_num = 0;
    let version_info = |list: &[String]| if list.is_empty() { "unknown".to_string() } else { list.join(", ") };
    for line in sp {
        let trimmed = java_trim(line);
        if let Some((raw_prefix, value)) = trimmed.split_once(':') {
            let prefix = raw_prefix.to_lowercase();
            if prefix == "chipset model" {
                let previous = card_num;
                card_num += 1;
                if previous > 0 {
                    card_list.push(GraphicsCard::new(name.clone(), device_id.clone(), vendor.clone(), version_info(&version_info_list), vram));
                    version_info_list.clear();
                }
                name = java_trim(value).to_string();
            } else if prefix == "device id" {
                device_id = java_trim(value).to_string();
            } else if prefix == "vendor" {
                vendor = java_trim(value).to_string();
            } else if prefix.contains("version") || prefix.contains("revision") {
                version_info_list.push(trimmed.to_string());
            } else if prefix.starts_with("vram") {
                vram = parse_decimal_memory_size_to_binary(java_trim(value));
            }
        }
    }
    card_list.push(GraphicsCard::new(name, device_id, vendor, version_info(&version_info_list), vram));
    card_list
}

pub fn get_graphics_cards() -> Vec<GraphicsCard> {
    graphics_cards_from(&run_native("system_profiler SPDisplaysDataType"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_profiler_displays_are_listed() {
        let output: Vec<String> = "Graphics/Displays:\n\n    Apple M2:\n\n      Chipset Model: Apple M2\n      Type: GPU\n      Vendor: Apple (0x106b)\n      Metal Support: Metal 3\n".lines().map(ToString::to_string).collect();
        let cards = graphics_cards_from(&output);
        assert_eq!(cards.len(), 1);
        assert_eq!((cards[0].name.as_str(), cards[0].vendor.as_str(), cards[0].device_id.as_str()), ("Apple M2", "Apple (0x106b)", "unknown"));
    }
}
