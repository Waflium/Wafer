use wafer_protocol::nbt::Nbt;

use super::*;
use crate::network::chat::component_serialization;
use crate::util::extra_codecs;
use crate::world::entity::entity_save::decode_uuid;

fn parse(snbt: &str) -> Nbt {
    crate::nbt::parse_fully(snbt).expect("valid snbt")
}

fn report_of(root: PathElement, read: impl FnOnce(&Nbt), input: &str) -> String {
    let collector = ScopedCollector::new(root, "test");
    read(&parse(input));
    let report = collector.collector.get_tree_report();
    std::mem::forget(collector);
    PROBLEM_REPORTER.with(|current| *current.borrow_mut() = None);
    report
}

#[test]
fn field_decode_failures_use_vanilla_wording() {
    let report = report_of(
        PathElement::Empty,
        |input| {
            read(input, "CustomName", component_serialization::decode_custom_name);
            read(input, "UUID", decode_uuid);
            get_f64(input, "Health");
        },
        "{CustomName:{text:1},UUID:\"abc\",Health:\"x\"}",
    );
    assert_eq!(
        report,
        ":\n  Failed to decode value '{text:1}' from field 'CustomName': Failed to parse either. First: Failed to parse either. First: Not a string; Second: Not a list: {text:1}; Second: No matching codec found\n  Failed to decode value '\"abc\"' from field 'UUID': Not a list\n  Expected field 'Health' to contain number, but got STRING"
    );
}

#[test]
fn a_single_problem_stays_on_the_root_line() {
    let report = report_of(PathElement::Text("chunk@[0, 0]".into()), |input| for_each_child(input, "Entities", |entity| {
        read(entity, "UUID", decode_uuid);
    }), "{Entities:[{UUID:[1,2]}]}");
    assert_eq!(report, "chunk@[0, 0].Entities[0]: Failed to decode value '[1,2]' from field 'UUID': Input is not a list of 4 ints");
}

#[test]
fn list_elements_report_their_index_and_type() {
    let report = report_of(
        PathElement::Empty,
        |input| {
            for_each_child(input, "Passengers", |_| {});
            list_or_empty::<()>(input, "Items", |tag| DecodeResult::error(format!("bad {}", crate::nbt::to_string(tag))));
        },
        "{Passengers:[5],Items:[1,2]}",
    );
    assert_eq!(
        report,
        ":\n  Expected list 'Passengers' to contain at index 0 value of type COMPOUND, but got INT\n  Failed to decode value '1' from field 'Items' at index 0': bad 1\n  Failed to decode value '2' from field 'Items' at index 1': bad 2"
    );
}

#[test]
fn typed_getters_report_the_actual_type() {
    let report = report_of(
        PathElement::Empty,
        |input| {
            assert_eq!(get_string(input, "a"), None);
            assert_eq!(children_list(input, "b"), None);
            assert_eq!(get_boolean(input, "c"), Some(true));
        },
        "{a:1,b:\"x\",c:1.5f}",
    );
    assert_eq!(report, ":\n  Expected field 'a' to contain value of type STRING, but got INT\n  Expected field 'b' to contain value of type LIST, but got STRING");
}

#[test]
fn vectors_and_positions_follow_the_list_codecs() {
    let input = parse("{pos:[1.0d,2.0d,3.0d,4.0d],short:[1.0d,2.0d],block:[I;1,2,3],bad:[1,\"a\",3]}");
    let pos = extra_codecs::vec3(input.get("pos").unwrap());
    assert_eq!(pos.error.as_deref(), Some("Input is not a list of 3 elements"));
    assert_eq!(pos.value.map(|v| (v.x, v.y, v.z)), Some((1.0, 2.0, 3.0)));
    let short = extra_codecs::vec3(input.get("short").unwrap());
    assert_eq!(short.error.as_deref(), Some("Input is not a list of 3 elements"));
    assert!(short.value.is_none());
    let block = extra_codecs::block_pos(input.get("block").unwrap());
    assert!(block.error.is_none());
    let bad = crate::util::codec::with_nbt_ops(|| extra_codecs::vec3(input.get("bad").unwrap()));
    assert_eq!(bad.error.as_deref(), Some("Not a number; Input is not a list of 3 elements"));
}

#[test]
fn nothing_is_reported_without_a_collector() {
    let input = parse("{Health:\"x\"}");
    assert_eq!(get_f64(&input, "Health"), None);
}

#[test]
fn read_map_reports_a_failure_to_decode_from_map() {
    let report = report_of(
        PathElement::Empty,
        |input| {
            let decoded = read_map(input, crate::world::entity::player::abilities::Abilities::decode);
            assert!(decoded.is_some());
        },
        "{flySpeed:\"x\",walkSpeed:0.5f}",
    );
    assert_eq!(report, ": Failed to decode from map: Not a number");
}

#[test]
fn child_enters_a_field_scope_only_for_compounds() {
    let report = report_of(
        PathElement::Text("root".into()),
        |input| {
            let (tag, scope) = child(input, "inner").expect("compound child");
            get_f64(tag, "Health");
            drop(scope);
            assert!(child(input, "text").is_none());
        },
        "{inner:{Health:\"x\"},text:\"y\"}",
    );
    assert_eq!(report, "root:\n  .inner: Expected field 'Health' to contain number, but got STRING\n  Expected field 'text' to contain value of type COMPOUND, but got STRING");
}
