use wafer_core::BlockPos;
use wafer_data::{BlockState, blocks, game_rule};

use crate::commands::CommandFn;
use crate::commands::arguments::blocks::{BlockInput, BlockPredicate};
use crate::commands::arguments::coordinates::get_loaded_block_pos;
use crate::commands::brigadier::{CommandContext, CommandSyntaxException, Dynamic2CommandExceptionType, SimpleCommandExceptionType, tr};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::{Arg, Component};
use crate::server::Server;
use crate::server::level::ServerLevel;
use crate::server::main_thread_frames;
use crate::world::level::BlockGetter;
use crate::world::level::levelgen::structure::BoundingBox;
use crate::world::level::update_flags::UPDATE_LIMIT;

static ERROR_AREA_TOO_LARGE: Dynamic2CommandExceptionType = Dynamic2CommandExceptionType(|max, count| tr("commands.fill.toobig", vec![max, count]));
static ERROR_FAILED: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.fill.failed");

pub const REPLACE_LAMBDA: [&str; 1] = ["net.minecraft.server.commands.FillCommand.lambda$wrapWithMode$0(FillCommand.java:84)"];
pub const OUTLINE_LAMBDA: [&str; 1] = ["net.minecraft.server.commands.FillCommand.lambda$wrapWithMode$1(FillCommand.java:87)"];
pub const HOLLOW_LAMBDA: [&str; 1] = ["net.minecraft.server.commands.FillCommand.lambda$wrapWithMode$2(FillCommand.java:91)"];
pub const DESTROY_LAMBDA: [&str; 1] = ["net.minecraft.server.commands.FillCommand.lambda$wrapWithMode$3(FillCommand.java:95)"];
pub const STRICT_LAMBDA: [&str; 1] = ["net.minecraft.server.commands.FillCommand.lambda$wrapWithMode$4(FillCommand.java:99)"];
pub const REPLACE_ALL_LAMBDA: [&str; 1] = ["net.minecraft.server.commands.FillCommand.lambda$register$4(FillCommand.java:60)"];
pub const KEEP_LAMBDA: [&str; 1] = ["net.minecraft.server.commands.FillCommand.lambda$register$9(FillCommand.java:67)"];
const FILL_BLOCKS_PLACE: [&str; 1] = ["net.minecraft.server.commands.FillCommand.fillBlocks(FillCommand.java:136)"];
const FILL_BLOCKS_UPDATE: [&str; 1] = ["net.minecraft.server.commands.FillCommand.fillBlocks(FillCommand.java:150)"];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Replace,
    Outline,
    Hollow,
    Destroy,
}

impl Mode {
    fn affect(self, level: &mut ServerLevel, pos: BlockPos) -> bool {
        self == Mode::Destroy && level.destroy_block(pos, true, None, UPDATE_LIMIT)
    }

    fn filter<'a>(self, region: &BoundingBox, pos: BlockPos, block: &'a BlockInput, hollow_core: &'a BlockInput) -> Option<&'a BlockInput> {
        let inside = pos.x != region.min.x && pos.x != region.max.x && pos.y != region.min.y && pos.y != region.max.y && pos.z != region.min.z && pos.z != region.max.z;
        match self {
            Mode::Outline if inside => None,
            Mode::Hollow if inside => Some(hollow_core),
            _ => Some(block),
        }
    }
}

enum Filter<'a> {
    Predicate(&'a BlockPredicate),
    Empty,
}

pub fn executors() -> Vec<(String, CommandFn)> {
    let mut out: Vec<(String, CommandFn)> = Vec::new();
    for base in ["fill from to block", "fill from to block replace filter"] {
        out.push((base.into(), replace));
        out.push((format!("{base} outline"), outline));
        out.push((format!("{base} hollow"), hollow));
        out.push((format!("{base} destroy"), destroy));
        out.push((format!("{base} strict"), strict));
    }
    out.push(("fill from to block replace".into(), replace_all));
    out.push(("fill from to block keep".into(), keep));
    out
}

#[allow(clippy::too_many_arguments)]
fn fill(
    server: &mut Server,
    context: &CommandContext,
    source: &CommandSourceStack,
    mode: Mode,
    keep: bool,
    strict: bool,
    lambda: &'static [&'static str],
) -> Result<i32, CommandSyntaxException> {
    let _lambda = main_thread_frames::enter_static(lambda);
    let from = get_loaded_block_pos(server, context, source, "from")?;
    let to = get_loaded_block_pos(server, context, source, "to")?;
    let filter = if keep {
        Some(Filter::Empty)
    } else if context.has_argument("filter") {
        Some(Filter::Predicate(context.get_block_predicate("filter")))
    } else {
        None
    };
    fill_blocks(server, source, BoundingBox::from_corners(from, to), context.get_block("block"), mode, filter, strict)
}

pub fn replace(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    fill(server, context, source, Mode::Replace, false, false, &REPLACE_LAMBDA)
}

pub fn replace_all(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    fill(server, context, source, Mode::Replace, false, false, &REPLACE_ALL_LAMBDA)
}

pub fn outline(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    fill(server, context, source, Mode::Outline, false, false, &OUTLINE_LAMBDA)
}

pub fn hollow(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    fill(server, context, source, Mode::Hollow, false, false, &HOLLOW_LAMBDA)
}

pub fn destroy(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    fill(server, context, source, Mode::Destroy, false, false, &DESTROY_LAMBDA)
}

pub fn strict(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    fill(server, context, source, Mode::Replace, false, true, &STRICT_LAMBDA)
}

pub fn keep(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    fill(server, context, source, Mode::Replace, true, false, &KEEP_LAMBDA)
}

fn fill_blocks(
    server: &mut Server,
    source: &CommandSourceStack,
    region: BoundingBox,
    target: &BlockInput,
    mode: Mode,
    filter: Option<Filter>,
    strict: bool,
) -> Result<i32, CommandSyntaxException> {
    let area = region.x_span() as i64 * region.y_span() as i64 * region.z_span() as i64;
    let limit = server.levels[source.level].game_rules.get_int(game_rule::MAX_BLOCK_MODIFICATIONS);
    if area > limit as i64 {
        return Err(ERROR_AREA_TOO_LARGE.create(limit, Arg::Long(area)));
    }
    let hollow_core = BlockInput { state: blocks::AIR.default_state(), properties: Vec::new(), tag: None };
    let level = &mut server.levels[source.level];
    let mut updated: Vec<(BlockPos, BlockState)> = Vec::new();
    if level.is_debug() {
        return Err(ERROR_FAILED.create());
    }
    let mut count = 0;
    let phase = main_thread_frames::enter_static(&FILL_BLOCKS_PLACE);
    for pos in region.positions() {
        let passes = match &filter {
            None => true,
            Some(Filter::Predicate(predicate)) => predicate.test(level, pos),
            Some(Filter::Empty) => level.block_state(pos).is_air(),
        };
        if !passes {
            continue;
        }
        let old = level.block_state(pos);
        let affected = mode.affect(level, pos);
        match mode.filter(&region, pos, target, &hollow_core) {
            Some(block) if block.place(level, pos, 2 | if strict { 816 } else { 256 }) => {
                if !strict {
                    updated.push((pos, old));
                }
                count += 1;
            }
            _ => {
                if affected {
                    count += 1;
                }
            }
        }
    }
    phase.switch_to(&FILL_BLOCKS_UPDATE);
    for (pos, old) in updated {
        level.update_neighbours_on_block_set(pos, old);
    }
    drop(phase);
    if count == 0 {
        return Err(ERROR_FAILED.create());
    }
    source.send_success(server, || Component::translatable_with("commands.fill.success", vec![Arg::Int(count)]), true);
    Ok(count)
}
