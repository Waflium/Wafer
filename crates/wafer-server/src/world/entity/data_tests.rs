use super::data::{DataValue, SynchedEntityData};

#[test]
fn floats_compare_like_java_float_equals() {
    let mut data = SynchedEntityData::new(vec![DataValue::Float(0.0)]);
    data.set(0, DataValue::Float(0.0));
    assert!(!data.is_dirty());
    data.set(0, DataValue::Float(-0.0));
    assert!(data.is_dirty());
    data.pack_dirty();
    data.set(0, DataValue::Float(f32::NAN));
    data.pack_dirty();
    data.set(0, DataValue::Float(-f32::NAN));
    assert!(!data.is_dirty());
}
