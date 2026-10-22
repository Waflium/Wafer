use wafer_data::{BlockState, block_tags, blocks};

use super::copper_golem::{self, CopperGolemState};
use crate::server::level::ServerLevel;
use crate::util::valueproviders::IntProvider;
use crate::world::entity::ai::activity_data::ActivityData;
use crate::world::entity::ai::behavior::animal_panic::AnimalPanic;
use crate::world::entity::ai::behavior::behavior_control::BehaviorControl;
use crate::world::entity::ai::behavior::count_down_cooldown_ticks::CountDownCooldownTicks;
use crate::world::entity::ai::behavior::do_nothing::DoNothing;
use crate::world::entity::ai::behavior::look_at_target_sink::LookAtTargetSink;
use crate::world::entity::ai::behavior::move_to_target_sink::MoveToTargetSink;
use crate::world::entity::ai::behavior::transport_items_between_containers::{
    ContainerInteractionState, OnTargetReachedInteraction, TransportItemTarget, TransportItemsBetweenContainers,
};
use crate::world::entity::ai::behavior::{interact_with_door, random_stroll, run_one, set_entity_look_target_sometimes};
use crate::world::entity::ai::brain::Brain;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::memory_status::MemoryStatus;
use crate::world::entity::schedule::Activity;
use crate::world::entity::{EntityId, entity_types, sound_event};
use crate::world::level::block::entity::BlockEntityKind;

const SPEED_MULTIPLIER_WHEN_PANICKING: f32 = 1.5;
const SPEED_MULTIPLIER_WHEN_IDLING: f32 = 1.0;
const TRANSPORT_ITEM_HORIZONTAL_SEARCH_RADIUS: i32 = 32;
const TRANSPORT_ITEM_VERTICAL_SEARCH_RADIUS: i32 = 8;
const TICK_TO_START_ON_REACHED_INTERACTION: i32 = 1;
const TICK_TO_PLAY_ON_REACHED_SOUND: i32 = 9;

fn transport_item_source_block(block: BlockState) -> bool {
    block.is(&block_tags::COPPER_CHESTS)
}

fn transport_item_destination_block(block: BlockState) -> bool {
    block.is_block(blocks::CHEST) || block.is_block(blocks::TRAPPED_CHEST)
}

pub fn get_activities() -> Vec<ActivityData> {
    vec![init_core_activity(), init_idle_activity()]
}

pub fn update_activity(brain: &mut Brain) {
    brain.set_active_activity_to_first_valid(&[Activity::IDLE]);
}

fn init_core_activity() -> ActivityData {
    ActivityData::create(
        Activity::CORE,
        0,
        vec![
            Box::new(AnimalPanic::new(SPEED_MULTIPLIER_WHEN_PANICKING)),
            Box::new(LookAtTargetSink::new(45, 90)),
            Box::new(MoveToTargetSink::default()),
            Box::new(interact_with_door::create()),
            Box::new(CountDownCooldownTicks::new(memory_module_type::GAZE_COOLDOWN_TICKS)),
            Box::new(CountDownCooldownTicks::new(memory_module_type::TRANSPORT_ITEMS_COOLDOWN_TICKS)),
        ],
    )
}

fn init_idle_activity() -> ActivityData {
    let stroll: Vec<(Box<dyn BehaviorControl>, i32)> =
        vec![(Box::new(random_stroll::stroll_within(SPEED_MULTIPLIER_WHEN_IDLING, 2, 2)), 1), (Box::new(DoNothing::new(30, 60)), 1)];
    ActivityData::create_pairs(
        Activity::IDLE,
        vec![
            (
                0,
                Box::new(TransportItemsBetweenContainers::new(
                    1.0,
                    transport_item_source_block,
                    transport_item_destination_block,
                    TRANSPORT_ITEM_HORIZONTAL_SEARCH_RADIUS,
                    TRANSPORT_ITEM_VERTICAL_SEARCH_RADIUS,
                    get_target_reached_interactions(),
                    on_travelling,
                    should_queue_for_target,
                )),
            ),
            (1, Box::new(set_entity_look_target_sometimes::create_for_type(&entity_types::PLAYER, 6.0, IntProvider::Uniform { min_inclusive: 40, max_inclusive: 80 }))),
            (
                2,
                Box::new(run_one::with_entry_condition(
                    vec![
                        (memory_module_type::WALK_TARGET.erase(), MemoryStatus::ValueAbsent),
                        (memory_module_type::TRANSPORT_ITEMS_COOLDOWN_TICKS.erase(), MemoryStatus::ValuePresent),
                    ],
                    stroll,
                )),
            ),
        ],
    )
}

fn get_target_reached_interactions() -> Vec<(ContainerInteractionState, OnTargetReachedInteraction)> {
    vec![
        (ContainerInteractionState::PickupItem, on_reached_target_interaction(CopperGolemState::GettingItem, Some("minecraft:entity.copper_golem.no_item_get"))),
        (ContainerInteractionState::PickupNoItem, on_reached_target_interaction(CopperGolemState::GettingNoItem, Some("minecraft:entity.copper_golem.no_item_no_get"))),
        (ContainerInteractionState::PlaceItem, on_reached_target_interaction(CopperGolemState::DroppingItem, Some("minecraft:entity.copper_golem.item_drop"))),
        (ContainerInteractionState::PlaceNoItem, on_reached_target_interaction(CopperGolemState::DroppingNoItem, Some("minecraft:entity.copper_golem.item_no_drop"))),
    ]
}

fn on_reached_target_interaction(state: CopperGolemState, sound: Option<&'static str>) -> OnTargetReachedInteraction {
    Box::new(move |level: &mut ServerLevel, body: EntityId, target: &TransportItemTarget, ticks_since_reaching_target: i32| {
        if !level.is_copper_golem(body) {
            return;
        }
        let container = target.container;
        if ticks_since_reaching_target == TICK_TO_START_ON_REACHED_INTERACTION {
            level.container_start_open(container, body);
            level.copper_golem_set_opened_chest_pos(body, target.pos);
            copper_golem::set_state(level.entity_mut(body), state);
        }
        if ticks_since_reaching_target == TICK_TO_PLAY_ON_REACHED_SOUND
            && let Some(sound) = sound
        {
            level.entity_play_sound(body, sound_event(sound), 1.0, 1.0);
        }
        if ticks_since_reaching_target == crate::world::entity::ai::behavior::transport_items_between_containers::TARGET_INTERACTION_TIME {
            if level.container_get_entities_with_container_open(container).contains(&body) {
                level.container_stop_open(container, body);
            }
            level.copper_golem_clear_opened_chest_pos(body);
        }
    })
}

fn on_travelling(level: &mut ServerLevel, body: EntityId) {
    if level.is_copper_golem(body) {
        level.copper_golem_clear_opened_chest_pos(body);
        copper_golem::set_state(level.entity_mut(body), CopperGolemState::Idle);
    }
}

fn should_queue_for_target(level: &mut ServerLevel, transport_target: &TransportItemTarget) -> bool {
    if level.get_block_entity(transport_target.pos).is_some_and(|entity| entity.id() == transport_target.block_entity && matches!(entity.kind, BlockEntityKind::Chest(_))) {
        !level.get_entities_with_container_open(transport_target.pos).is_empty()
    } else {
        false
    }
}
