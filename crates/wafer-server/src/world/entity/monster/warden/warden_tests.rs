use wafer_core::Vec3;

use crate::commands::perform_prefixed_command;
use crate::commands::source::CommandSourceStack;
use crate::server::level::testing;

#[test]
fn wardens_can_be_summoned() {
    let mut server = testing::server();
    let source = CommandSourceStack::server(&server);
    perform_prefixed_command(&mut server, &source, "difficulty easy");
    let player = testing::player(&mut server.levels[0], Vec3::new(8.5, -60.0, 0.5), 0.0, 0.0);
    let _ = player;
    perform_prefixed_command(&mut server, &source, "summon warden 0 -60 0");
    for _ in 0..40 {
        server.tick();
    }
    assert!(server.levels[0].entities.values().any(|entity| entity.entity().entity_type == wafer_data::registry_id(wafer_data::registry::ENTITY_TYPE, "minecraft:warden")));
}
