use super::percentiles;
use crate::nbt::java_double_to_string;

fn rendered(percentiles: Vec<(i32, f64)>) -> Vec<(i32, String)> {
    percentiles.into_iter().map(|(percentile, value)| (percentile, java_double_to_string(value))).collect()
}

#[test]
fn percentiles_interpolate_like_guava_quantiles() {
    assert_eq!(
        rendered(percentiles::evaluate(&[0.300599, 4.5447805, 16.797002, 0.5407605, 11.9274416, 3.1, 7.25, 0.9])),
        [(99, "16.456132772".to_string()), (90, "13.388309719999999".to_string()), (75, "8.4193604".to_string()), (50, "3.8223902499999998".to_string())]
    );
    assert_eq!(rendered(percentiles::evaluate(&[5.0])), [(99, "5.0".to_string()), (90, "5.0".to_string()), (75, "5.0".to_string()), (50, "5.0".to_string())]);
    assert_eq!(rendered(percentiles::evaluate(&[3.0, 1.0, 2.0])), [(99, "2.98".to_string()), (90, "2.8".to_string()), (75, "2.5".to_string()), (50, "2.0".to_string())]);
    assert_eq!(rendered(percentiles::evaluate(&[1e9, 2e9, 7334062.0, 1458950.0, 240800.0])), [(99, "1.96E9".to_string()), (90, "1.6E9".to_string()), (75, "1.0E9".to_string()), (50, "7334062.0".to_string())]);
    assert_eq!(
        rendered(percentiles::evaluate_longs(&[240800, 7804000, 7801500, 1458950, 1866425, 2596130, 7334062])),
        [(99, "7803850.0".to_string()), (90, "7802500.0".to_string()), (75, "7567781.0".to_string()), (50, "2596130.0".to_string())]
    );
    assert!(percentiles::evaluate(&[]).is_empty());
}

#[test]
fn recordings_round_trip_through_the_flight_recorder_format() {
    use crate::rfr::consumer::recording_file::RecordingFile;
    use crate::rfr::{EventType, FieldType, Recording, Value, ValueDescriptor, recording};
    static TEST_EVENT: EventType = EventType::new(
        900,
        "test.Values",
        "Values",
        &["Test"],
        true,
        true,
        &[
            ValueDescriptor::new("number", "Number", FieldType::Int),
            ValueDescriptor::new("big", "Big", FieldType::Long),
            ValueDescriptor::new("ratio", "Ratio", FieldType::Float),
            ValueDescriptor::new("text", "Text", FieldType::String),
            ValueDescriptor::new("missing", "Missing", FieldType::String),
            ValueDescriptor::new("flag", "Flag", FieldType::Boolean),
        ],
    );
    recording::register(&TEST_EVENT);
    let path = std::env::temp_dir().join(format!("wafer-rfr-{}.rfr", std::process::id()));
    let recording = Recording::new();
    recording.set_destination(path.clone());
    recording.start();
    let start = crate::rfr::ticks();
    TEST_EVENT.commit(start, start + 1_500_000, &[Value::Int(-7), Value::Long(1 << 40), Value::Float(0.25), Value::String("héllo"), Value::NullString, Value::Boolean(true)]);
    recording.stop().unwrap();
    let mut file = RecordingFile::open(&path).unwrap();
    let mut found = None;
    while file.has_more_events() {
        let event = file.read_event().unwrap();
        if event.get_event_type_name() == "test.Values" {
            found = Some(event);
        }
    }
    let event = found.expect("the committed event");
    assert_eq!(event.get_int("number"), -7);
    assert_eq!(event.get_long("big"), 1 << 40);
    assert_eq!(event.get_float("ratio"), 0.25);
    assert_eq!(event.get_string("text").as_deref(), Some("héllo"));
    assert_eq!(event.get_string("missing"), None);
    assert!(event.get_boolean("flag"));
    assert_eq!(event.get_duration().as_micros(), 1500);
    assert_eq!(event.get_thread("eventThread").and_then(|thread| thread.java_name), std::thread::current().name().map(str::to_string));
    let _ = std::fs::remove_file(path);
}
