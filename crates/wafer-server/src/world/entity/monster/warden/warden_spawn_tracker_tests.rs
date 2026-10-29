use wafer_protocol::nbt::Nbt;

use super::warden_spawn_tracker::WardenSpawnTracker;

fn tag(fields: &[(&str, Nbt)]) -> Nbt {
    Nbt::Compound(fields.iter().map(|(k, v)| (k.to_string(), v.clone())).collect())
}

fn decoded(tag: &Nbt) -> Option<WardenSpawnTracker> {
    WardenSpawnTracker::decode(tag).into_result().ok()
}

#[test]
fn decode_reports_vanillas_messages() {
    assert_eq!(crate::util::codec::with_nbt_ops(|| WardenSpawnTracker::decode(&Nbt::String("x".into()))).error.as_deref(), Some("Not a map: \"x\""));
    let result = crate::util::codec::with_nbt_ops(|| WardenSpawnTracker::decode(&tag(&[("cooldown_ticks", Nbt::Int(-1)), ("warning_level", Nbt::String("a".into()))])));
    assert_eq!(result.error.as_deref(), Some("Value must be non-negative: -1; Not a number"));
    assert_eq!(result.value, None);
}

#[test]
fn tick_decreases_the_warning_level_every_12000_ticks() {
    let mut tracker = WardenSpawnTracker::new(11999, 3, 2);
    tracker.tick();
    assert_eq!(tracker, WardenSpawnTracker::new(12000, 3, 1));
    tracker.tick();
    assert_eq!(tracker, WardenSpawnTracker::new(0, 2, 0));
    tracker.tick();
    assert_eq!(tracker, WardenSpawnTracker::new(1, 2, 0));
}

#[test]
fn decrease_clamps_an_out_of_range_loaded_level() {
    let mut tracker = decoded(&tag(&[("ticks_since_last_warning", Nbt::Int(12000)), ("warning_level", Nbt::Int(9))])).unwrap();
    assert_eq!(tracker.get_warning_level(), 9);
    tracker.tick();
    assert_eq!(tracker, WardenSpawnTracker::new(0, 4, 0));
    let mut tracker = WardenSpawnTracker::new(12000, 0, 0);
    tracker.tick();
    assert_eq!(tracker.get_warning_level(), 0);
}

#[test]
fn save_writes_every_field() {
    let saved = WardenSpawnTracker::new(5, 2, 7).to_nbt();
    assert_eq!(saved, tag(&[("ticks_since_last_warning", Nbt::Int(5)), ("warning_level", Nbt::Int(2)), ("cooldown_ticks", Nbt::Int(7))]));
    assert_eq!(WardenSpawnTracker::default().to_nbt(), tag(&[("ticks_since_last_warning", Nbt::Int(0)), ("warning_level", Nbt::Int(0)), ("cooldown_ticks", Nbt::Int(0))]));
}

#[test]
fn load_matches_the_codec() {
    assert_eq!(decoded(&tag(&[])), Some(WardenSpawnTracker::default()));
    assert_eq!(decoded(&tag(&[("warning_level", Nbt::Byte(3)), ("cooldown_ticks", Nbt::Double(7.9))])), Some(WardenSpawnTracker::new(0, 3, 7)));
    assert_eq!(decoded(&tag(&[("cooldown_ticks", Nbt::Long(1 << 32 | 5))])), Some(WardenSpawnTracker::new(0, 0, 5)));
    assert_eq!(decoded(&tag(&[("warning_level", Nbt::Int(2)), ("cooldown_ticks", Nbt::Int(-1))])), None);
    assert_eq!(decoded(&tag(&[("warning_level", Nbt::String("2".into()))])), None);
    assert_eq!(decoded(&Nbt::Int(1)), None);
}
