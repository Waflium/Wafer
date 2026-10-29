use wafer_protocol::Writer;

pub const KEY_FORWARD: u8 = 1;
pub const KEY_BACKWARD: u8 = 2;
pub const KEY_LEFT: u8 = 4;
pub const KEY_RIGHT: u8 = 8;
pub const KEY_JUMP: u8 = 16;
pub const KEY_SHIFT: u8 = 32;
pub const KEY_SPRINT: u8 = 64;

pub const ACTION_START_DESTROY_BLOCK: i32 = 0;
pub const ACTION_ABORT_DESTROY_BLOCK: i32 = 2;
pub const ACTION_STOP_DESTROY_BLOCK: i32 = 3;
pub const ACTION_DROP_ALL_ITEMS: i32 = 4;
pub const ACTION_DROP_ITEM: i32 = 5;
pub const ACTION_RELEASE_USE_ITEM: i32 = 6;
pub const ACTION_SWAP_ITEM_WITH_OFFHAND: i32 = 7;
pub const ACTION_STAB: i32 = 8;

pub const COMMAND_START_SPRINTING: i32 = 1;
pub const COMMAND_STOP_SPRINTING: i32 = 2;
pub const COMMAND_START_RIDING_JUMP: i32 = 3;
pub const COMMAND_OPEN_INVENTORY: i32 = 5;

pub const FACE_DOWN: u8 = 0;

pub struct BlockHit {
    pub pos: (i32, i32, i32),
    pub face: i32,
    pub cursor: (f32, f32, f32),
    pub inside: bool,
    pub world_border: bool,
}

pub struct TestInstanceData<'a> {
    pub test: Option<&'a str>,
    pub size: (i32, i32, i32),
    pub rotation: i32,
    pub ignore_entities: bool,
}

pub fn interact(w: &mut Writer, entity: i32, hand: i32, location: (f64, f64, f64), using_secondary_action: bool) {
    w.varint(entity);
    w.varint(hand);
    w.lp_vec3(location.0, location.1, location.2);
    w.bool(using_secondary_action);
}

pub fn attack(w: &mut Writer, entity: i32) {
    w.varint(entity);
}

pub fn player_input(w: &mut Writer, keys: u8) {
    w.u8(keys);
}

pub fn player_command(w: &mut Writer, entity: i32, action: i32, data: i32) {
    w.varint(entity);
    w.varint(action);
    w.varint(data);
}

pub fn move_player_rot(w: &mut Writer, y_rot: f32, x_rot: f32, on_ground: bool, horizontal_collision: bool) {
    w.f32(y_rot);
    w.f32(x_rot);
    w.u8(on_ground as u8 | (horizontal_collision as u8) << 1);
}

pub fn move_vehicle(w: &mut Writer, position: (f64, f64, f64), y_rot: f32, x_rot: f32, on_ground: bool) {
    w.f64(position.0);
    w.f64(position.1);
    w.f64(position.2);
    w.f32(y_rot);
    w.f32(x_rot);
    w.bool(on_ground);
}

pub fn paddle_boat(w: &mut Writer, left: bool, right: bool) {
    w.bool(left);
    w.bool(right);
}

pub fn use_item(w: &mut Writer, hand: i32, sequence: i32, y_rot: f32, x_rot: f32) {
    w.varint(hand);
    w.varint(sequence);
    w.f32(y_rot);
    w.f32(x_rot);
}

pub fn use_item_on(w: &mut Writer, hand: i32, hit: &BlockHit, sequence: i32) {
    w.varint(hand);
    w.position(hit.pos.0, hit.pos.1, hit.pos.2);
    w.varint(hit.face);
    w.f32(hit.cursor.0);
    w.f32(hit.cursor.1);
    w.f32(hit.cursor.2);
    w.bool(hit.inside);
    w.bool(hit.world_border);
    w.varint(sequence);
}

pub fn player_action(w: &mut Writer, action: i32, pos: (i32, i32, i32), face: u8, sequence: i32) {
    w.varint(action);
    w.position(pos.0, pos.1, pos.2);
    w.u8(face);
    w.varint(sequence);
}

pub fn select_trade(w: &mut Writer, index: i32) {
    w.varint(index);
}

pub fn pick_item_from_block(w: &mut Writer, pos: (i32, i32, i32), include_data: bool) {
    w.position(pos.0, pos.1, pos.2);
    w.bool(include_data);
}

pub fn pick_item_from_entity(w: &mut Writer, entity: i32, include_data: bool) {
    w.varint(entity);
    w.bool(include_data);
}

pub fn container_click(w: &mut Writer, container: i32, state_id: i32, slot: i16, button: u8, mode: i32) {
    w.varint(container);
    w.varint(state_id);
    w.i16(slot);
    w.u8(button);
    w.varint(mode);
    w.varint(0);
    w.bool(false);
}

pub fn test_instance_block_action(w: &mut Writer, pos: (i32, i32, i32), action: i32, data: &TestInstanceData) {
    w.position(pos.0, pos.1, pos.2);
    w.varint(action);
    w.bool(data.test.is_some());
    if let Some(test) = data.test {
        w.string(test);
    }
    w.varint(data.size.0);
    w.varint(data.size.1);
    w.varint(data.size.2);
    w.varint(data.rotation);
    w.bool(data.ignore_entities);
    w.varint(0);
    w.bool(false);
}

pub fn set_test_block(w: &mut Writer, pos: (i32, i32, i32), mode: i32, message: &str) {
    w.position(pos.0, pos.1, pos.2);
    w.varint(mode);
    w.string(message);
}

#[cfg(test)]
mod tests {
    use wafer_protocol::Reader;

    use super::*;

    fn bytes(body: impl FnOnce(&mut Writer)) -> Vec<u8> {
        let mut w = Writer::new();
        body(&mut w);
        w.into_inner()
    }

    #[test]
    fn interact_matches_vanilla_codec() {
        let data = bytes(|w| interact(w, 300, 1, (0.0, 0.5, 0.0), true));
        let mut r = Reader::new(&data);
        assert_eq!(r.varint().unwrap(), 300);
        assert_eq!(r.varint().unwrap(), 1);
        let (x, y, z) = r.lp_vec3().unwrap();
        assert_eq!((x, z), (0.0, 0.0));
        assert!((y - 0.5).abs() < 1.0e-4);
        assert!(r.bool().unwrap());
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn player_input_packs_flags_like_vanilla() {
        assert_eq!(bytes(|w| player_input(w, KEY_FORWARD | KEY_JUMP | KEY_SHIFT)), [1 | 16 | 32]);
        assert_eq!(KEY_BACKWARD | KEY_LEFT | KEY_RIGHT | KEY_SPRINT, 2 | 4 | 8 | 64);
    }

    #[test]
    fn player_command_writes_entity_action_and_data() {
        let data = bytes(|w| player_command(w, 7, COMMAND_START_RIDING_JUMP, 80));
        assert_eq!(data, [7, 3, 80]);
    }

    #[test]
    fn move_player_rot_packs_collision_flag() {
        let data = bytes(|w| move_player_rot(w, 90.0, -10.0, true, true));
        let mut r = Reader::new(&data);
        assert_eq!(r.f32().unwrap(), 90.0);
        assert_eq!(r.f32().unwrap(), -10.0);
        assert_eq!(r.u8().unwrap(), 3);
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn move_vehicle_writes_position_rotation_and_ground() {
        let data = bytes(|w| move_vehicle(w, (1.5, 64.0, -3.25), 45.0, 5.0, true));
        let mut r = Reader::new(&data);
        assert_eq!((r.f64().unwrap(), r.f64().unwrap(), r.f64().unwrap()), (1.5, 64.0, -3.25));
        assert_eq!((r.f32().unwrap(), r.f32().unwrap()), (45.0, 5.0));
        assert!(r.bool().unwrap());
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn use_item_on_matches_block_hit_result_codec() {
        let hit = BlockHit { pos: (-5, 70, 12), face: 1, cursor: (0.25, 1.0, 0.75), inside: true, world_border: false };
        let data = bytes(|w| use_item_on(w, 1, &hit, 9));
        let mut r = Reader::new(&data);
        assert_eq!(r.varint().unwrap(), 1);
        assert_eq!(r.position().unwrap(), (-5, 70, 12));
        assert_eq!(r.varint().unwrap(), 1);
        assert_eq!((r.f32().unwrap(), r.f32().unwrap(), r.f32().unwrap()), (0.25, 1.0, 0.75));
        assert!(r.bool().unwrap());
        assert!(!r.bool().unwrap());
        assert_eq!(r.varint().unwrap(), 9);
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn use_item_writes_hand_sequence_and_rotation() {
        let data = bytes(|w| use_item(w, 0, 3, 12.5, -45.0));
        let mut r = Reader::new(&data);
        assert_eq!((r.varint().unwrap(), r.varint().unwrap()), (0, 3));
        assert_eq!((r.f32().unwrap(), r.f32().unwrap()), (12.5, -45.0));
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn player_action_writes_action_position_face_sequence() {
        let data = bytes(|w| player_action(w, ACTION_STOP_DESTROY_BLOCK, (1, 2, 3), 5, 200));
        let mut r = Reader::new(&data);
        assert_eq!(r.varint().unwrap(), 3);
        assert_eq!(r.position().unwrap(), (1, 2, 3));
        assert_eq!(r.u8().unwrap(), 5);
        assert_eq!(r.varint().unwrap(), 200);
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn container_click_writes_empty_changed_slots_and_carried_item() {
        let data = bytes(|w| container_click(w, 4, 17, -999, 1, 4));
        let mut r = Reader::new(&data);
        assert_eq!((r.varint().unwrap(), r.varint().unwrap()), (4, 17));
        assert_eq!(r.i16().unwrap(), -999);
        assert_eq!(r.u8().unwrap(), 1);
        assert_eq!(r.varint().unwrap(), 4);
        assert_eq!(r.varint().unwrap(), 0);
        assert!(!r.bool().unwrap());
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn test_instance_block_action_matches_data_codec() {
        let data = TestInstanceData { test: Some("minecraft:example"), size: (3, 4, 5), rotation: 2, ignore_entities: true };
        let bytes = bytes(|w| test_instance_block_action(w, (10, 64, -10), 6, &data));
        let mut r = Reader::new(&bytes);
        assert_eq!(r.position().unwrap(), (10, 64, -10));
        assert_eq!(r.varint().unwrap(), 6);
        assert!(r.bool().unwrap());
        assert_eq!(r.string(32767).unwrap(), "minecraft:example");
        assert_eq!((r.varint().unwrap(), r.varint().unwrap(), r.varint().unwrap()), (3, 4, 5));
        assert_eq!(r.varint().unwrap(), 2);
        assert!(r.bool().unwrap());
        assert_eq!(r.varint().unwrap(), 0);
        assert!(!r.bool().unwrap());
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn test_instance_block_action_without_test_writes_absent_optional() {
        let data = TestInstanceData { test: None, size: (0, 0, 0), rotation: 0, ignore_entities: false };
        let bytes = bytes(|w| test_instance_block_action(w, (0, 0, 0), 1, &data));
        let mut r = Reader::new(&bytes);
        r.position().unwrap();
        assert_eq!(r.varint().unwrap(), 1);
        assert!(!r.bool().unwrap());
        assert_eq!(r.remaining(), 3 + 1 + 1 + 1 + 1);
    }

    #[test]
    fn set_test_block_writes_position_mode_and_message() {
        let data = bytes(|w| set_test_block(w, (1, 2, 3), 2, "boom"));
        let mut r = Reader::new(&data);
        assert_eq!(r.position().unwrap(), (1, 2, 3));
        assert_eq!(r.varint().unwrap(), 2);
        assert_eq!(r.string(32767).unwrap(), "boom");
        assert_eq!(r.remaining(), 0);
    }
}
