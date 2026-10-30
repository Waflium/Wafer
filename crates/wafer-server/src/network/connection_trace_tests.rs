use std::collections::HashMap;
use std::fmt::Write;

use wafer_core::BlockPos;
use wafer_data::packets::play::clientbound;
use wafer_data::{EquipmentSlot, items};
use wafer_protocol::nbt::Nbt;
use wafer_protocol::{Fault, stream_codec};

use super::chat::Component;
use super::chat_type::{Bound, ChatType};
use super::connection::{NettyPath, encode, encoder_exception, encoding_fault};
use super::protocol::game::{clientbound_block_entity_data_packet, clientbound_disguised_chat_packet, clientbound_system_chat_packet};
use crate::world::entity::data::{DataValue, write_values};
use crate::world::entity::living_entity::set_equipment;
use crate::world::item::ItemStack;
use crate::world::item::component as dc;

const ORACLE_FRAMES: &str = include_str!("connection_traces/oracle_frames.txt");
const ACTION_BAR_PLAIN: &str = include_str!("connection_traces/action_bar_plain.txt");
const SYSTEM_CHAT_LISTENER: &str = include_str!("connection_traces/system_chat_listener.txt");
const SYSTEM_CHAT_SKIPPED: &str = include_str!("connection_traces/system_chat_skipped.txt");
const ENTITY_DATA_BUNDLE: &str = include_str!("connection_traces/entity_data_bundle.txt");

fn long() -> String {
    "a".repeat(70000)
}

fn literal() -> Component {
    Component::literal(long())
}

fn fault_of(packet: &[u8]) -> (i32, Fault) {
    encoding_fault(packet).expect("the packet fails to encode")
}

fn describe(fault: &Fault) -> String {
    let mut out = String::new();
    let mut current = Some(fault);
    let mut number = 1;
    while let Some(fault) = current {
        let _ = writeln!(out, "C{number} {}", fault.description);
        for frame in &fault.frames {
            let _ = writeln!(out, "{frame}");
        }
        number += 1;
        current = fault.cause.as_deref();
    }
    out
}

fn item(ty: dc::DataComponentType, value: Nbt) -> ItemStack {
    let mut stack = ItemStack::new(items::STONE, 1);
    stack.set(ty, Some(value));
    stack
}

fn compound(key: &str, value: Nbt) -> Nbt {
    Nbt::Compound(vec![(key.to_string(), value)].into())
}

fn equipment(stack: ItemStack) -> Vec<u8> {
    set_equipment(1, &[(EquipmentSlot::Mainhand, Some(stack))])
}

fn entity_data(value: DataValue) -> Vec<u8> {
    encode(clientbound::SET_ENTITY_DATA, |w| write_values(w, 1, &[(8, value)]))
}

fn block_entity_data(tag: Nbt) -> Vec<u8> {
    encode(clientbound::BLOCK_ENTITY_DATA, |w| clientbound_block_entity_data_packet::write(w, BlockPos::new(0, 0, 0), 7, &tag))
}

fn disguised(name: Component, target: Option<Component>, message: Component) -> Vec<u8> {
    let bound = Bound { chat_type: ChatType::Chat, name, target_name: target };
    encode(clientbound::DISGUISED_CHAT, |w| clientbound_disguised_chat_packet::write(w, &message, &bound))
}

fn system_chat(message: Component) -> Vec<u8> {
    encode(clientbound::SYSTEM_CHAT, |w| clientbound_system_chat_packet::write(w, &message, false))
}

fn cases() -> Vec<(&'static str, Vec<u8>)> {
    let styled = Component::literal("a").append(Component::literal(long()).with_color(super::chat::ChatFormatting::Red));
    let list = Nbt::List(vec![compound("b", Nbt::String(long()))]);
    vec![
        ("system_chat", system_chat(literal())),
        ("system_chat_list", system_chat(Component::literal("a").append(literal()))),
        ("system_chat_compound", system_chat(styled)),
        ("disguised_message", disguised(Component::literal("n"), None, literal())),
        ("disguised_name", disguised(literal(), None, Component::literal("m"))),
        ("disguised_target", disguised(Component::literal("n"), Some(literal()), Component::literal("m"))),
        ("block_entity_data_root", block_entity_data(compound("a", Nbt::String(long())))),
        ("block_entity_data_list", block_entity_data(compound("a", Nbt::List(vec![Nbt::String(long())])))),
        ("block_entity_data_list_compound", block_entity_data(compound("a", list.clone()))),
        ("block_entity_data_key", block_entity_data(compound(&long(), Nbt::String("x".into())))),
        ("item_equipment", equipment(item(dc::CUSTOM_NAME, Nbt::String(long())))),
        ("equip_item_name", equipment(item(dc::ITEM_NAME, Nbt::String(long())))),
        ("equip_lore", equipment(item(dc::LORE, Nbt::List(vec![Nbt::String("x".into()), Nbt::String(long())])))),
        ("equip_custom_data", equipment(item(dc::CUSTOM_DATA, compound("a", Nbt::String(long()))))),
        ("equip_custom_data_deep", equipment(item(dc::CUSTOM_DATA, compound("a", list)))),
        ("entity_data_item", entity_data(DataValue::ItemStack(Some(item(dc::CUSTOM_NAME, Nbt::String(long())))))),
        ("entity_data_opt_component", entity_data(DataValue::OptionalComponent(Some(Nbt::String(long()))))),
        ("entity_data_component", entity_data(DataValue::Component(Nbt::String(long())))),
        (
            "entity_data_second",
            encode(clientbound::SET_ENTITY_DATA, |w| write_values(w, 1, &[(0, DataValue::Byte(0)), (2, DataValue::Component(Nbt::String(long())))])),
        ),
    ]
}

fn oracle_sections() -> HashMap<&'static str, String> {
    let mut sections: HashMap<&'static str, String> = HashMap::new();
    let mut current = None;
    for line in ORACLE_FRAMES.lines() {
        if let Some(name) = line.strip_prefix("=== ") {
            current = Some(name);
            sections.insert(name, String::new());
        } else if let Some(name) = current {
            let section = sections.get_mut(name).unwrap();
            section.push_str(line);
            section.push('\n');
        }
    }
    sections
}

#[test]
fn codec_frames_match_what_vanillas_codecs_throw() {
    let oracle = oracle_sections();
    let cases = cases();
    assert_eq!(cases.len(), oracle.len());
    for (name, packet) in cases {
        let (_, fault) = fault_of(&packet);
        assert_eq!(describe(&fault), oracle[name], "{name}");
    }
}

#[test]
fn a_plain_send_prints_the_netty_write_path_of_the_server_thread() {
    let packet = encode(clientbound::SET_ACTION_BAR_TEXT, |w| w.layer(&[stream_codec::composite(1, 1)], |w| literal().write(w)));
    let (id, fault) = fault_of(&packet);
    assert_eq!(encoder_exception(id, fault, NettyPath::Plain).wrapper.print_stack_trace(), ACTION_BAR_PLAIN);
}

#[test]
fn a_send_with_a_fallback_listener_takes_the_listener_path() {
    let (id, fault) = fault_of(&system_chat(literal()));
    let exception = encoder_exception(id, fault, NettyPath::Listener);
    assert_eq!(exception.wrapper.print_stack_trace(), SYSTEM_CHAT_LISTENER);
    assert_eq!(exception.skip().print_stack_trace(), SYSTEM_CHAT_SKIPPED);
}

#[test]
fn a_packet_inside_a_bundle_takes_the_bundle_path() {
    let (id, fault) = fault_of(&entity_data(DataValue::ItemStack(Some(item(dc::CUSTOM_NAME, Nbt::String(long()))))));
    assert_eq!(encoder_exception(id, fault, NettyPath::Bundle).wrapper.print_stack_trace(), ENTITY_DATA_BUNDLE);
}

#[test]
fn faults_that_were_written_without_codec_frames_elide_the_whole_trace() {
    let packet = encode(clientbound::LOGIN, |w| w.fail("java.lang.IllegalArgumentException: no frames".into()));
    let (id, fault) = fault_of(&packet);
    let trace = encoder_exception(id, fault, NettyPath::Plain).wrapper.print_stack_trace();
    assert!(trace.ends_with("Caused by: java.lang.IllegalArgumentException: no frames\n\t... 24 more\n"), "{trace}");
}
