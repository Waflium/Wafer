use wafer_core::{BlockPos, mth};
use wafer_data::blocks;

use super::game_test_batch_factory;
use super::game_test_info::{GameTestInfo, InfoId, Listener};
use super::game_test_instance::GameTestInstance;
use super::game_test_runner::{self, Builder, StructureSpawner};
use super::retry_options::RetryOptions;
use super::{game_test_ticker, multiple_test_tracker, structure_grid_spawner, structure_utils};
use crate::commands::CommandFn;
use crate::commands::brigadier::{
    CommandContext, CommandSyntaxException, Dynamic3CommandExceptionType, DynamicCommandExceptionType, SimpleCommandExceptionType, tr,
};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::component_utils;
use crate::network::chat::{Arg, ChatFormatting, ClickEvent, Component, HoverEvent};
use crate::server::Server;
use crate::world::level::block::rotation::Rotation;
use crate::world::level::clip::{ClipBlock, ClipContext, ClipFluid};
use crate::world::phys::shapes::collision_context::CollisionContext;
use crate::world::level::chunk::HeightmapType;

pub const TEST_NEARBY_SEARCH_RADIUS: i32 = 15;
pub const TEST_FULL_SEARCH_RADIUS: i32 = 250;
const DEFAULT_CLEAR_RADIUS: i32 = 250;
const MAX_CLEAR_RADIUS: i32 = 1024;
const TEST_POS_Z_OFFSET_FROM_PLAYER: i32 = 3;
const DEFAULT_SIZE: i32 = 5;

static CLEAR_NO_TESTS: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.test.clear.error.no_tests");
static TEST_INSTANCE_COULD_NOT_BE_FOUND: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.test.error.test_instance_not_found");
static NO_TEST_INSTANCES: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.test.error.no_test_instances");
static NO_TEST_CONTAINING: Dynamic3CommandExceptionType = Dynamic3CommandExceptionType(|x, y, z| tr("commands.test.error.no_test_containing_pos", vec![x, y, z]));
static TOO_LARGE: DynamicCommandExceptionType = DynamicCommandExceptionType(|size| tr("commands.test.error.too_large", vec![size]));

#[derive(Clone, Copy)]
enum PosFinder {
    None,
    Radius(i32),
    Nearest,
    AllNearby,
    LookedAt,
}

struct TestFinder {
    source: CommandSourceStack,
    tests: Vec<&'static str>,
    positions: PosFinder,
    copies: usize,
}

impl TestFinder {
    fn new(source: &CommandSourceStack, tests: Vec<&'static str>, positions: PosFinder) -> TestFinder {
        TestFinder { source: source.clone(), tests, positions, copies: 1 }
    }

    fn find_tests(&self) -> Vec<&'static str> {
        (0..self.copies).flat_map(|_| self.tests.iter().copied()).collect()
    }

    fn find_test_pos(&self, server: &mut Server) -> Vec<(usize, BlockPos)> {
        let level = self.source.level;
        let pos = BlockPos::containing(self.source.position.x, self.source.position.y, self.source.position.z);
        let found: Vec<BlockPos> = match self.positions {
            PosFinder::None => Vec::new(),
            PosFinder::Radius(radius) => structure_utils::find_test_blocks(pos, radius, &mut server.levels[level]),
            PosFinder::Nearest => structure_utils::find_nearest_test(pos, TEST_NEARBY_SEARCH_RADIUS, &mut server.levels[level]).into_iter().collect(),
            PosFinder::AllNearby => structure_utils::find_test_blocks(pos, TEST_FULL_SEARCH_RADIUS, &mut server.levels[level]),
            PosFinder::LookedAt => match self.source.player_id(server) {
                Some(player) => {
                    let entity = server.levels[level].entity(player);
                    let (eye, look) = (entity.eye_position(), entity.look_angle());
                    structure_utils::looked_at_test_pos(pos, eye, look, &mut server.levels[level]).into_iter().collect()
                }
                None => Vec::new(),
            },
        };
        let found: Vec<(usize, BlockPos)> = found.into_iter().map(|pos| (level, pos)).collect();
        (0..self.copies).flat_map(|_| found.iter().copied()).collect()
    }
}

fn selected(context: &CommandContext) -> Vec<&'static str> {
    match context.argument("tests") {
        crate::commands::arguments::ArgumentValue::ResourceSelector(ids) => ids.iter().filter_map(|id| GameTestInstance::get(id).map(|(name, _)| name)).collect(),
        _ => Vec::new(),
    }
}

fn by_resource_selection(context: &CommandContext, source: &CommandSourceStack) -> TestFinder {
    TestFinder::new(source, selected(context), PosFinder::None)
}

fn failed_tests(server: &Server, source: &CommandSourceStack, only_required_tests: bool) -> TestFinder {
    let tests = server
        .game_test_ticker
        .get_last_failed_tests()
        .into_iter()
        .filter(|test| !only_required_tests || GameTestInstance::get(test).is_some_and(|(_, test)| test.required()))
        .collect();
    TestFinder::new(source, tests, PosFinder::None)
}

type FinderFn = fn(&Server, &CommandContext, &CommandSourceStack) -> TestFinder;

fn finder_run(_: &Server, context: &CommandContext, source: &CommandSourceStack) -> TestFinder {
    by_resource_selection(context, source)
}

fn finder_these(_: &Server, _: &CommandContext, source: &CommandSourceStack) -> TestFinder {
    TestFinder::new(source, Vec::new(), PosFinder::AllNearby)
}

fn finder_closest(_: &Server, _: &CommandContext, source: &CommandSourceStack) -> TestFinder {
    TestFinder::new(source, Vec::new(), PosFinder::Nearest)
}

fn finder_that(_: &Server, _: &CommandContext, source: &CommandSourceStack) -> TestFinder {
    TestFinder::new(source, Vec::new(), PosFinder::LookedAt)
}

fn finder_failed(server: &Server, context: &CommandContext, source: &CommandSourceStack) -> TestFinder {
    failed_tests(server, source, context.has_argument("onlyRequiredTests") && context.get_bool("onlyRequiredTests"))
}

fn run_with(server: &mut Server, context: &CommandContext, source: &CommandSourceStack, finder: FinderFn) -> Result<i32, CommandSyntaxException> {
    let finder = finder(server, context, source);
    let retry_options = if context.has_argument("numberOfTimes") {
        let until_failed = context.has_argument("untilFailed") && context.get_bool("untilFailed");
        RetryOptions { number_of_tries: context.get_int("numberOfTimes"), halt_on_failure: until_failed }
    } else {
        RetryOptions::no_retries()
    };
    let rotation_steps = if context.has_argument("rotationSteps") { context.get_int("rotationSteps") } else { 0 };
    let tests_per_row = if context.has_argument("testsPerRow") { context.get_int("testsPerRow") } else { 8 };
    run(server, finder, retry_options, rotation_steps, tests_per_row)
}

macro_rules! run_executor {
    ($name:ident, $finder:expr) => {
        fn $name(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
            run_with(server, context, source, $finder)
        }
    };
}

run_executor!(run_tests, finder_run);
run_executor!(run_these, finder_these);
run_executor!(run_closest, finder_closest);
run_executor!(run_that, finder_that);
run_executor!(run_failed, finder_failed);

pub fn executors() -> Vec<(String, CommandFn)> {
    let mut all: Vec<(String, CommandFn)> = Vec::new();
    let mut with_retry = |prefix: &str, f: CommandFn, build_info: bool| {
        all.push((prefix.to_string(), f));
        all.push((format!("{prefix} numberOfTimes"), f));
        all.push((format!("{prefix} numberOfTimes untilFailed"), f));
        if build_info {
            all.push((format!("{prefix} numberOfTimes untilFailed rotationSteps"), f));
            all.push((format!("{prefix} numberOfTimes untilFailed rotationSteps testsPerRow"), f));
        }
    };
    with_retry("test run tests", run_tests, true);
    with_retry("test runthese", run_these, false);
    with_retry("test runclosest", run_closest, false);
    with_retry("test runthat", run_that, false);
    with_retry("test runfailed", run_failed, true);
    with_retry("test runfailed onlyRequiredTests", run_failed, true);
    all.push(("test runmultiple tests".into(), run_multiple));
    all.push(("test runmultiple tests amount".into(), run_multiple));
    all.push(("test verify tests".into(), verify_tests));
    all.push(("test locate tests".into(), locate_tests));
    all.push(("test resetclosest".into(), |server, context, source| reset(server, finder_closest(server, context, source))));
    all.push(("test resetthese".into(), |server, context, source| reset(server, finder_these(server, context, source))));
    all.push(("test resetthat".into(), |server, context, source| reset(server, finder_that(server, context, source))));
    all.push(("test clearthat".into(), |server, context, source| clear(server, finder_that(server, context, source))));
    all.push(("test clearthese".into(), |server, context, source| clear(server, finder_these(server, context, source))));
    all.push(("test clearall".into(), |server, _, source| clear(server, TestFinder::new(source, Vec::new(), PosFinder::Radius(DEFAULT_CLEAR_RADIUS)))));
    all.push(("test clearall radius".into(), |server, context, source| {
        clear(server, TestFinder::new(source, Vec::new(), PosFinder::Radius(context.get_int("radius").clamp(0, MAX_CLEAR_RADIUS))))
    }));
    all.push(("test stop".into(), |server, _, _| Ok(stop_tests(server))));
    all.push(("test pos".into(), |server, _, source| show_pos(server, source, "pos")));
    all.push(("test pos var".into(), |server, context, source| show_pos(server, source, &context.get_string("var").to_string())));
    all.push(("test create id".into(), |server, context, source| create_new_structure(server, source, context.get_identifier("id"), DEFAULT_SIZE, DEFAULT_SIZE, DEFAULT_SIZE)));
    all.push(("test create id width".into(), |server, context, source| {
        let width = context.get_int("width");
        create_new_structure(server, source, context.get_identifier("id"), width, width, width)
    }));
    all.push(("test create id width height depth".into(), |server, context, source| {
        create_new_structure(server, source, context.get_identifier("id"), context.get_int("width"), context.get_int("height"), context.get_int("depth"))
    }));
    all
}

fn run_multiple(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let mut finder = by_resource_selection(context, source);
    if context.has_argument("amount") {
        finder.copies = context.get_int("amount").max(0) as usize;
    }
    run(server, finder, RetryOptions::no_retries(), 0, 8)
}

fn verify_tests(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    Ok(verify(server, by_resource_selection(context, source)))
}

fn locate_tests(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    locate(server, by_resource_selection(context, source))
}

fn reset(server: &mut Server, finder: TestFinder) -> Result<i32, CommandSyntaxException> {
    stop_tests(server);
    let source = finder.source.clone();
    let mut count = 0;
    for (level, pos) in finder.find_test_pos(server) {
        if create_game_test_info(server, (level, pos), &source, RetryOptions::no_retries()).is_some() {
            let mut feedback = Vec::new();
            server.levels[level].test_instance_block_entity_reset_test(pos, &mut feedback);
            for message in feedback {
                source.send_system_message(server, &message);
            }
            count += 1;
        }
    }
    if count == 0 {
        return Err(CLEAR_NO_TESTS.create());
    }
    source.send_success(server, || Component::translatable_with("commands.test.reset.success", vec![Arg::Int(count)]), true);
    Ok(count)
}

fn clear(server: &mut Server, finder: TestFinder) -> Result<i32, CommandSyntaxException> {
    stop_tests(server);
    let source = finder.source.clone();
    let tests: Vec<(usize, BlockPos)> =
        finder.find_test_pos(server).into_iter().filter(|&(level, pos)| server.levels[level].test_instance_block_entity(pos).is_some()).collect();
    for &(level, pos) in &tests {
        let bounding_box = server.levels[level].test_instance_block_entity_get_test_bounding_box(pos);
        structure_utils::clear_space_for_structure(&bounding_box, &mut server.levels[level]);
        server.levels[level].test_instance_block_entity_remove_barriers(pos);
        server.levels[level].destroy_block(pos, false, None, 512);
    }
    if tests.is_empty() {
        return Err(CLEAR_NO_TESTS.create());
    }
    let count = tests.len() as i32;
    source.send_success(server, || Component::translatable_with("commands.test.clear.success", vec![Arg::Int(count)]), true);
    Ok(count)
}

fn to_game_test_infos(server: &mut Server, source: &CommandSourceStack, retry_options: RetryOptions, finder: &TestFinder) -> Vec<InfoId> {
    let positions = finder.find_test_pos(server);
    positions.into_iter().filter_map(|pos| create_game_test_info(server, pos, source, retry_options)).collect()
}

fn to_game_test_info(server: &mut Server, source: &CommandSourceStack, retry_options: RetryOptions, finder: &TestFinder, rotation_steps: i32) -> Vec<InfoId> {
    let mut infos = Vec::new();
    for test in finder.find_tests() {
        let Some((name, instance)) = GameTestInstance::get(test) else { continue };
        if !verify_structure_exists(server, source, instance.structure()) {
            continue;
        }
        match server.get_level(instance.dimension()) {
            None => source.send_failure(server, Component::literal(format!("Could not resolve level for dimension: {}", instance.dimension()))),
            Some(level) => {
                let rotation = structure_utils::get_rotation_for_rotation_steps(rotation_steps).unwrap_or(Rotation::None);
                infos.push(game_test_runner::new_info(server, GameTestInfo::new((name, instance), rotation, level, retry_options)));
            }
        }
    }
    infos
}

fn create_game_test_info(server: &mut Server, (level, pos): (usize, BlockPos), source: &CommandSourceStack, retry_options: RetryOptions) -> Option<InfoId> {
    if server.levels[level].test_instance_block_entity(pos).is_none() {
        source.send_failure(server, Component::translatable_with("commands.test.error.test_instance_not_found.position", vec![Arg::Int(pos.x), Arg::Int(pos.y), Arg::Int(pos.z)]));
        return None;
    }
    let Some(test) = server.levels[level].test_instance_block_entity_test_holder(pos) else {
        let name = server.levels[level].test_instance_block_entity_get_test_name(pos);
        source.send_failure(server, Component::translatable_with("commands.test.error.non_existant_test", vec![name.into()]));
        return None;
    };
    let rotation = server.levels[level].test_instance_block_entity_get_rotation(pos);
    let mut info = GameTestInfo::new(test, rotation, level, retry_options);
    info.set_test_block_pos(Some(pos));
    if !verify_structure_exists(server, source, test.1.structure()) {
        return None;
    }
    Some(game_test_runner::new_info(server, info))
}

fn create_new_structure(server: &mut Server, source: &CommandSourceStack, id: &str, x_size: i32, y_size: i32, z_size: i32) -> Result<i32, CommandSyntaxException> {
    if x_size > 48 || y_size > 48 || z_size > 48 {
        return Err(TOO_LARGE.create(Arg::Int(48)));
    }
    let level = source.level;
    let test_pos = create_test_position_around(server, source, level);
    let id = id.to_string();
    let test = structure_utils::create_new_empty_test(&mut server.levels[level], &id, test_pos, BlockPos::new(x_size, y_size, z_size), Rotation::None);
    let low = server.levels[level].test_instance_block_entity_get_structure_pos(test);
    let high = low.offset(x_size - 1, 0, z_size - 1);
    for block_pos in BlockPos::between_closed(low, high) {
        server.levels[level].set_block(block_pos, blocks::BEDROCK.default_state(), 3);
    }
    let name = server.levels[level].test_instance_block_entity_get_test_name(test);
    source.send_success(server, || Component::translatable_with("commands.test.create.success", vec![name.into()]), true);
    Ok(1)
}

fn show_pos(server: &mut Server, source: &CommandSourceStack, var_name: &str) -> Result<i32, CommandSyntaxException> {
    let player = source.player_or_exception(server)?;
    let level = source.level;
    let entity = server.levels[level].entity(player);
    let from = entity.eye_position();
    let to = from.add_vec(entity.look_angle().scale(10.0));
    let collision_context = CollisionContext::of(&server.levels[level], player);
    let pick = server.levels[level].clip(&ClipContext { from, to, block: ClipBlock::Outline, fluid: ClipFluid::None, collision_context });
    let target_pos_absolute = pick.hit.block_pos;
    let mut test_block_pos = structure_utils::find_test_containing_pos(target_pos_absolute, TEST_NEARBY_SEARCH_RADIUS, &mut server.levels[level]);
    if test_block_pos.is_none() {
        test_block_pos = structure_utils::find_test_containing_pos(target_pos_absolute, TEST_FULL_SEARCH_RADIUS, &mut server.levels[level]);
    }
    let Some(test_block_pos) = test_block_pos else {
        return Err(NO_TEST_CONTAINING.create(Arg::Int(target_pos_absolute.x), Arg::Int(target_pos_absolute.y), Arg::Int(target_pos_absolute.z)));
    };
    if server.levels[level].test_instance_block_entity(test_block_pos).is_none() {
        return Err(TEST_INSTANCE_COULD_NOT_BE_FOUND.create());
    }
    let structure_pos = server.levels[level].test_instance_block_entity_get_structure_pos(test_block_pos);
    let relative = target_pos_absolute.offset(-structure_pos.x, -structure_pos.y, -structure_pos.z);
    let target_pos_description = format!("{}, {}, {}", relative.x, relative.y, relative.z);
    let test_name = server.levels[level].test_instance_block_entity_get_test_name(test_block_pos).get_string();
    let mut coords = Component::translatable_with("commands.test.coordinates", vec![Arg::Int(relative.x), Arg::Int(relative.y), Arg::Int(relative.z)])
        .with_color(ChatFormatting::Green)
        .with_hover(HoverEvent::ShowText(Box::new(Component::translatable("commands.test.coordinates.copy"))))
        .with_click(ClickEvent::CopyToClipboard(format!("final BlockPos {var_name} = new BlockPos({target_pos_description});")));
    coords.style.bold = Some(true);
    source.send_success(server, || Component::translatable_with("commands.test.relative_position", vec![Arg::String(test_name), coords.into()]), false);
    if let Some(player) = server.get_player(player) {
        player.connection.send(wafer_data::packets::play::clientbound::GAME_TEST_HIGHLIGHT_POS, |w| {
            w.position(target_pos_absolute.x, target_pos_absolute.y, target_pos_absolute.z);
            w.position(relative.x, relative.y, relative.z);
        });
    }
    Ok(1)
}

pub fn stop_tests(server: &mut Server) -> i32 {
    game_test_ticker::clear(server);
    1
}

pub fn track_and_start_runner(server: &mut Server, source: &CommandSourceStack) -> i32 {
    game_test_runner::add_listener(server, source.clone());
    let tests = game_test_runner::get_test_infos(server);
    let tracker = multiple_test_tracker::new_tracker(server, tests);
    multiple_test_tracker::add_summary_displayer(server, tracker, source.clone());
    multiple_test_tracker::add_listener(server, tracker, Listener::RememberFailure);
    game_test_runner::start(server);
    1
}

fn verify_structure_exists(server: &Server, source: &CommandSourceStack, structure: &str) -> bool {
    if crate::world::level::levelgen::structure::templatesystem::structure_template_manager::get(structure).is_none() {
        source.send_failure(server, Component::translatable_with("commands.test.error.structure_not_found", vec![Arg::Component(Component::literal(structure))]));
        return false;
    }
    true
}

fn create_test_position_around(server: &mut Server, source: &CommandSourceStack, level: usize) -> BlockPos {
    let (_, player_pos, surface_y) = player_and_test_info(server, source, level);
    BlockPos::new(player_pos.x, surface_y, player_pos.z + TEST_POS_Z_OFFSET_FROM_PLAYER)
}

fn player_and_test_info(server: &mut Server, source: &CommandSourceStack, level: usize) -> (Option<crate::world::entity::EntityId>, BlockPos, i32) {
    let player = source.player_id(server);
    let position = match player {
        Some(player) => server.entity_level(player).entity(player).position(),
        None => source.position,
    };
    let player_pos = BlockPos::containing(position.x, position.y, position.z);
    let test_pos = BlockPos::new(player_pos.x, 384, player_pos.z);
    let surface_y = server.levels[level].heightmap_pos(HeightmapType::WorldSurface, test_pos).y;
    (player, player_pos, surface_y)
}

fn is_player_in_current_test_dimension(server: &Server, player: Option<crate::world::entity::EntityId>, test_dimensions: &[String]) -> bool {
    let Some(player) = player else { return false };
    let Some(level) = server.level_of(player) else { return false };
    let player_dimension = server.levels[level].dimension;
    test_dimensions.len() <= 1 && test_dimensions.iter().any(|d| d == player_dimension) && test_dimensions.iter().any(|d| d == "minecraft:overworld")
}

pub fn output_test_coordinates(server: &mut Server, source: &CommandSourceStack, test_dimensions: &[String]) {
    for test_dimension in test_dimensions {
        let Some(test_level) = server.get_level(test_dimension) else { continue };
        let (_, test_player_pos, surface_y) = player_and_test_info(server, source, test_level);
        let (player, ..) = player_and_test_info(server, source, source.level);
        if is_player_in_current_test_dimension(server, player, test_dimensions) {
            return;
        }
        let short_dimension = test_dimension.split_once(':').map_or(test_dimension.as_str(), |(_, path)| path);
        let test_coordinates = component_utils::wrap_in_square_brackets(Component::translatable_with(
            "test.run.coordinates",
            vec![Arg::Int(test_player_pos.x), Arg::Int(surface_y), Arg::Int(test_player_pos.z), Arg::String(short_dimension.to_string())],
        ))
        .with_color(ChatFormatting::Yellow)
        .with_click(ClickEvent::SuggestCommand(format!("/execute in {test_dimension} run tp @s {} {surface_y} {} 0 0", test_player_pos.x, test_player_pos.z)))
        .with_hover(HoverEvent::ShowText(Box::new(Component::translatable("chat.coordinates.tooltip"))));
        source.send_success(server, || test_coordinates, false);
    }
}

pub fn output_player_coordinates(server: &mut Server, source: &CommandSourceStack, test_dimensions: &[String]) {
    let (player, player_pos, _) = player_and_test_info(server, source, source.level);
    if is_player_in_current_test_dimension(server, player, test_dimensions) {
        return;
    }
    let Some(player) = player else { return };
    let entity = server.entity_level(player).entity(player);
    let (y_rot, x_rot) = (entity.y_rot, entity.x_rot);
    let player_dimension = server.entity_level(player).dimension;
    let short_dimension = player_dimension.split_once(':').map_or(player_dimension, |(_, path)| path).to_string();
    let player_coordinates = component_utils::wrap_in_square_brackets(Component::translatable_with(
        "test.player.coordinates",
        vec![Arg::Int(player_pos.x), Arg::Int(player_pos.y), Arg::Int(player_pos.z), Arg::String(short_dimension)],
    ))
    .with_color(ChatFormatting::Gold)
    .with_click(ClickEvent::SuggestCommand(format!(
        "/execute in {player_dimension} run tp @s {} {} {} {} {}",
        player_pos.x,
        player_pos.y,
        player_pos.z,
        mth::floor(y_rot as f64),
        mth::floor(x_rot as f64)
    )))
    .with_hover(HoverEvent::ShowText(Box::new(Component::translatable("chat.coordinates.tooltip"))));
    source.send_success(server, || player_coordinates, false);
}

fn run(server: &mut Server, finder: TestFinder, retry_options: RetryOptions, extra_rotation_steps: i32, tests_per_row: i32) -> Result<i32, CommandSyntaxException> {
    stop_tests(server);
    server.game_test_ticker.reset_arena_if_idle();
    let source = finder.source.clone();
    let mut infos = to_game_test_infos(server, &source, retry_options, &finder);
    infos.extend(to_game_test_info(server, &source, retry_options, &finder, extra_rotation_steps));
    if infos.is_empty() {
        source.send_success(server, || Component::translatable("commands.test.no_tests"), false);
        return Ok(0);
    }
    server.game_test_ticker.forget_failed_tests();
    let count = infos.len() as i32;
    source.send_success(server, || Component::translatable_with("commands.test.run.running", vec![Arg::Int(count)]), false);
    let corner_source = source.clone();
    let spawner = structure_grid_spawner::new(server, Box::new(move |server, dimension| test_position_for(server, &corner_source, dimension)), tests_per_row, false);
    Builder::from_info(server, infos).new_structure_spawner(StructureSpawner::Grid(spawner)).build(server);
    Ok(track_and_start_runner(server, &source))
}

fn test_position_for(server: &mut Server, source: &CommandSourceStack, dimension: &str) -> BlockPos {
    let level = server.get_level(dimension).unwrap_or(0);
    create_test_position_around(server, source, level)
}

fn verify(server: &mut Server, finder: TestFinder) -> i32 {
    stop_tests(server);
    server.game_test_ticker.reset_arena_if_idle();
    let source = finder.source.clone();
    let mut infos = to_game_test_infos(server, &source, RetryOptions::no_retries(), &finder);
    infos.extend(to_game_test_info(server, &source, RetryOptions::no_retries(), &finder, 0));
    server.game_test_ticker.forget_failed_tests();
    let mut batches = Vec::new();
    for info in infos {
        for (rotation_index, rotation) in Rotation::VALUES.into_iter().enumerate() {
            let mut transformed_infos = Vec::new();
            for _ in 0..100 {
                let original = &server.game_test_ticker.infos[info];
                let mut copy_info = GameTestInfo::new(original.test, rotation, original.level, RetryOptions { number_of_tries: 1, halt_on_failure: true });
                copy_info.set_test_block_pos(original.get_test_block_pos());
                transformed_infos.push(game_test_runner::new_info(server, copy_info));
            }
            let original = &server.game_test_ticker.infos[info];
            let (environment, dimension) = (original.test.1.batch(), server.levels[original.level].dimension.to_string());
            batches.push(game_test_batch_factory::to_game_test_batch(transformed_infos, environment, rotation_index as i32, dimension));
        }
    }
    let corner_source = source.clone();
    let spawner = structure_grid_spawner::new(server, Box::new(move |server, dimension| test_position_for(server, &corner_source, dimension)), 10, true);
    Builder::from_batches(batches)
        .batcher(100)
        .new_structure_spawner(StructureSpawner::Grid(spawner))
        .existing_structure_spawner(StructureSpawner::Grid(spawner))
        .halt_on_error()
        .clear_between_batches()
        .build(server);
    track_and_start_runner(server, &source)
}

fn find_selected_test_positions_across_dimensions(server: &mut Server, finder: &TestFinder) -> Vec<(usize, BlockPos)> {
    let selected_tests = finder.find_tests();
    if selected_tests.is_empty() {
        return Vec::new();
    }
    let source_pos = BlockPos::containing(finder.source.position.x, finder.source.position.y, finder.source.position.z);
    let mut matches = Vec::new();
    for level in 0..server.levels.len() {
        for pos in structure_utils::find_test_blocks(source_pos, TEST_FULL_SEARCH_RADIUS, &mut server.levels[level]) {
            let test = server.levels[level].test_instance_block_entity(pos).and_then(|entity| entity.data.test.clone());
            if test.is_some_and(|test| selected_tests.iter().any(|selected| *selected == test)) {
                matches.push((level, pos));
            }
        }
    }
    matches
}

fn locate(server: &mut Server, finder: TestFinder) -> Result<i32, CommandSyntaxException> {
    let source = finder.source.clone();
    source.send_system_message(server, &Component::translatable("commands.test.locate.started"));
    let mut structures_found = 0;
    let source_pos = BlockPos::containing(source.position.x, source.position.y, source.position.z);
    let source_dimension = server.levels[source.level].dimension;
    let mut found_test_dimensions: Vec<String> = Vec::new();
    let mut positions = finder.find_test_pos(server);
    for pos in find_selected_test_positions_across_dimensions(server, &finder) {
        positions.push(pos);
    }
    let mut seen = Vec::new();
    for (level, pos) in positions {
        if seen.contains(&(level, pos)) {
            continue;
        }
        seen.push((level, pos));
        if server.levels[level].test_instance_block_entity(pos).is_none() {
            continue;
        }
        let dimension = server.levels[level].dimension;
        if !found_test_dimensions.iter().any(|found| found == dimension) {
            found_test_dimensions.push(dimension.to_string());
        }
        let direction = server.levels[level].test_instance_block_entity_get_rotation(pos).rotate(wafer_core::Direction::North);
        let teleport_position = pos.relative_n(direction, 3);
        let teleport_y_rot = direction.opposite().to_y_rot() as i32;
        let tp_command = format!("/execute in {dimension} run tp @s {} {} {} {teleport_y_rot} 0", teleport_position.x, teleport_position.y, teleport_position.z);
        let (dx, dz) = (source_pos.x - pos.x, source_pos.z - pos.z);
        let distance = if dimension == source_dimension { mth::floor(((dx * dx + dz * dz) as f32).sqrt() as f64).to_string() } else { "N/A".to_string() };
        let path = dimension.split_once(':').map_or(dimension, |(_, path)| path);
        let coordinates = component_utils::wrap_in_square_brackets(
            Component::translatable_with("chat.coordinates", vec![Arg::Int(pos.x), Arg::Int(pos.y), Arg::Int(pos.z)]).append(Component::literal(format!(" ->[{path}]"))),
        )
        .with_color(ChatFormatting::Green)
        .with_click(ClickEvent::SuggestCommand(tp_command))
        .with_hover(HoverEvent::ShowText(Box::new(Component::translatable("chat.coordinates.tooltip"))));
        source.send_success(server, || Component::translatable_with("commands.test.locate.found", vec![coordinates.into(), Arg::String(distance)]), false);
        structures_found += 1;
    }
    if structures_found == 0 {
        return Err(NO_TEST_INSTANCES.create());
    }
    output_player_coordinates(server, &source, &found_test_dimensions);
    source.send_success(server, || Component::translatable_with("commands.test.locate.done", vec![Arg::Int(structures_found)]), true);
    Ok(structures_found)
}

