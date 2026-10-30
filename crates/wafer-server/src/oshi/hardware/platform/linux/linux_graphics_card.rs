use crate::oshi::hardware::GraphicsCard;
use crate::oshi::util::executing_command::run_native;
use crate::oshi::util::parse_util::{java_trim, parse_lshw_resource_string, parse_lspci_machine_readable, parse_lspci_memory_size};

fn version_info(version_info_list: &[String]) -> String {
    if version_info_list.is_empty() { "unknown".to_string() } else { version_info_list.join(", ") }
}

fn split_limit_2(line: &str) -> Vec<&str> {
    match line.split_once(':') {
        Some((prefix, rest)) => vec![prefix, rest],
        None => vec![line],
    }
}

fn split_colons(line: &str) -> Vec<&str> {
    let mut parts: Vec<&str> = line.split(':').collect();
    while parts.len() > 1 && parts.last().is_some_and(|last| last.is_empty()) {
        parts.pop();
    }
    parts
}

fn query_lspci_memory_size(lookup_device: Option<&str>) -> i64 {
    let mut vram = 0;
    for mem in run_native(&format!("lspci -v -s {}", lookup_device.unwrap_or("null"))) {
        if mem.contains(" prefetchable") {
            vram += parse_lspci_memory_size(&mem);
        }
    }
    vram
}

pub fn graphics_cards_from_lspci(lspci: &[String], memory_size: impl Fn(Option<&str>) -> i64) -> Vec<GraphicsCard> {
    let mut card_list = Vec::new();
    let mut name = "unknown".to_string();
    let mut device_id = "unknown".to_string();
    let mut vendor = "unknown".to_string();
    let mut version_info_list: Vec<String> = Vec::new();
    let mut found = false;
    let mut lookup_device: Option<String> = None;
    for line in lspci {
        let trimmed = java_trim(line);
        let split = split_limit_2(trimmed);
        let prefix = split[0];
        if prefix == "Class" && (line.contains("VGA") || line.contains("3D controller")) {
            found = true;
        } else if prefix == "Device" && !found && split.len() > 1 {
            lookup_device = Some(java_trim(split[1]).to_string());
        }
        if found {
            if split.len() < 2 {
                card_list.push(GraphicsCard::new(name.clone(), device_id.clone(), vendor.clone(), version_info(&version_info_list), memory_size(lookup_device.as_deref())));
                version_info_list.clear();
                found = false;
            } else if prefix == "Device" {
                if let Some((pair_name, pair_id)) = parse_lspci_machine_readable(java_trim(split[1])) {
                    name = pair_name;
                    device_id = format!("0x{pair_id}");
                }
            } else if prefix == "Vendor" {
                vendor = match parse_lspci_machine_readable(java_trim(split[1])) {
                    Some((pair_name, pair_id)) => format!("{pair_name} (0x{pair_id})"),
                    None => java_trim(split[1]).to_string(),
                };
            } else if prefix == "Rev:" {
                version_info_list.push(trimmed.to_string());
            }
        }
    }
    if found {
        card_list.push(GraphicsCard::new(name, device_id, vendor, version_info(&version_info_list), memory_size(lookup_device.as_deref())));
    }
    card_list
}

pub fn graphics_cards_from_lshw(lshw: &[String]) -> Vec<GraphicsCard> {
    let mut card_list = Vec::new();
    let mut name = "unknown".to_string();
    let device_id = "unknown".to_string();
    let mut vendor = "unknown".to_string();
    let mut version_info_list: Vec<String> = Vec::new();
    let mut vram = 0;
    let mut card_num = 0;
    for line in lshw {
        let trimmed = java_trim(line);
        let split = split_colons(trimmed);
        if split[0].starts_with("*-display") {
            let previous = card_num;
            card_num += 1;
            if previous > 0 {
                card_list.push(GraphicsCard::new(name.clone(), device_id.clone(), vendor.clone(), version_info(&version_info_list), vram));
                version_info_list.clear();
            }
        } else if split.len() == 2 {
            let prefix = split[0];
            if prefix == "product" {
                name = java_trim(split[1]).to_string();
            } else if prefix == "vendor" {
                vendor = java_trim(split[1]).to_string();
            } else if prefix == "version" {
                version_info_list.push(trimmed.to_string());
            } else if prefix.starts_with("resources") {
                vram = parse_lshw_resource_string(java_trim(split[1]));
            }
        }
    }
    card_list.push(GraphicsCard::new(name, device_id, vendor, version_info(&version_info_list), vram));
    card_list
}

pub fn get_graphics_cards() -> Vec<GraphicsCard> {
    let card_list = graphics_cards_from_lspci(&run_native("lspci -vnnm"), query_lspci_memory_size);
    if card_list.is_empty() { graphics_cards_from_lshw(&run_native("lshw -C display")) } else { card_list }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(text: &str) -> Vec<String> {
        text.lines().map(ToString::to_string).collect()
    }

    #[test]
    fn lspci_output_lists_display_adapters() {
        let output = lines(
            "Slot:\t01:00.0\nClass:\tVGA compatible controller [0300]\nVendor:\tNVIDIA Corporation [10de]\nDevice:\tAD103 [GeForce RTX 4080 SUPER] [2702]\nSVendor:\tASUSTeK [1043]\nRev:\ta1\nProgIf:\t00\n\nSlot:\t00:1f.3\nClass:\tAudio device [0403]\nVendor:\tIntel [8086]\n\n",
        );
        let cards = graphics_cards_from_lspci(&output, |_| 1 << 30);
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].name, "AD103 [GeForce RTX 4080 SUPER]");
        assert_eq!(cards[0].device_id, "0x2702");
        assert_eq!(cards[0].vendor, "NVIDIA Corporation (0x10de)");
        assert_eq!(cards[0].version_info, "Rev:\ta1");
        assert_eq!(cards[0].vram, 1 << 30);
    }

    #[test]
    fn lshw_output_lists_display_adapters() {
        let output = lines("  *-display\n       description: VGA compatible controller\n       product: Alder Lake GT1\n       vendor: Intel Corporation\n       version: 0c\n       resources: irq:130 memory:6000000000-6000ffffff\n");
        let cards = graphics_cards_from_lshw(&output);
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].name, "Alder Lake GT1");
        assert_eq!(cards[0].vendor, "Intel Corporation");
        assert_eq!(cards[0].version_info, "version: 0c");
        assert_eq!(cards[0].vram, 0x1000000);
    }
}
