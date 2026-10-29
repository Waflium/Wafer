use wafer_protocol::nbt::Nbt;

use crate::util::codec;
use crate::world::entity::ai::brain::Packed;
use crate::world::entity::ai::gossip::gossip_container::GossipContainer;
use crate::world::entity::npc::villager::villager_data::VillagerData;
use crate::world::item::trading::merchant_offers::MerchantOffers;

fn nbt(snbt: &str) -> Nbt {
    crate::nbt::parse_fully(snbt).expect("valid snbt")
}

#[test]
fn gossips_report_each_failing_entry_and_keep_the_valid_ones() {
    let decoded = codec::with_nbt_ops(|| GossipContainer::decode(&nbt(r#"[{Type:"nope",Target:[I;1,2,3,4],Value:5},{Type:"major_negative",Target:[I;1,2,3,4],Value:7}]"#)));
    assert_eq!(decoded.error.as_deref(), Some("Unknown element name:nope"));
    assert!(decoded.value.is_some());
    let decoded = codec::with_nbt_ops(|| GossipContainer::decode(&nbt(r#"[{Type:"major_negative",Target:5,Value:"x"}]"#)));
    assert_eq!(decoded.error.as_deref(), Some("Not a number; Not a list"));
    assert_eq!(codec::with_nbt_ops(|| GossipContainer::decode(&nbt("5"))).error.as_deref(), Some("Not a list: 5"));
    let decoded = codec::with_nbt_ops(|| GossipContainer::decode(&nbt(r#"[{Type:"major_negative",Target:[I;1,2,3,4],Value:0}]"#)));
    assert_eq!(decoded.error.as_deref(), Some("Value must be positive: 0"));
}

#[test]
fn villager_data_only_fails_on_a_wrong_level() {
    assert_eq!(codec::with_nbt_ops(|| VillagerData::decode(&nbt(r#"{level:"x",profession:"nope",type:"nope"}"#))).err().as_deref(), Some("Not a number"));
    assert!(codec::with_nbt_ops(|| VillagerData::decode(&nbt(r#"{profession:"nope",type:"nope"}"#))).is_ok());
    assert_eq!(codec::with_nbt_ops(|| VillagerData::decode(&nbt("5"))).err().as_deref(), Some("Not a map: 5"));
}

#[test]
fn merchant_offers_collect_the_errors_of_a_recipe_in_reverse_field_order() {
    let decoded = codec::with_nbt_ops(|| MerchantOffers::decode(&nbt("{Recipes:[{}]}")));
    assert_eq!(decoded.error.as_deref(), Some("No key sell in MapLike[{}]; No key buy in MapLike[{}]"));
    assert!(decoded.value.is_some_and(|offers| offers.is_empty()));
    assert_eq!(codec::with_nbt_ops(|| MerchantOffers::decode(&nbt("5"))).error.as_deref(), Some("Not a map: 5"));
}

#[test]
fn brains_report_unknown_memories_and_bad_values() {
    let decoded = codec::with_nbt_ops(|| Packed::decode(&nbt(r#"{memories:{"minecraft:nope":{value:5}}}"#)));
    let error = decoded.error.expect("an error");
    assert!(error.starts_with("Unknown registry key in ResourceKey[minecraft:root / minecraft:memory_module_type]: minecraft:nope; Unknown registry key"), "{error}");
    assert!(decoded.value.is_some());
    let decoded = codec::with_nbt_ops(|| Packed::decode(&nbt(r#"{memories:{"minecraft:home":{value:5}}}"#)));
    assert!(decoded.error.expect("an error").starts_with("Not a map: 5 missed input: "));
    assert_eq!(codec::with_nbt_ops(|| Packed::decode(&nbt("5"))).error.as_deref(), Some("Not a map: 5"));
}
