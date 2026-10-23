use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::time::Instant;

use chrono::Local;

use super::command_response_tracker::CommandResponseTracker;
use crate::commands::arguments::function_argument;
use crate::commands::brigadier::{CommandContext, CommandSyntaxException, SimpleCommandExceptionType, tr};
use crate::commands::execution::tasks::CallFunction;
use crate::commands::execution::{ChainModifiers, ChainStage, EntryAction, ExecutionContext, ExecutionControl};
use crate::commands::source::{CommandResultCallback, CommandSource, CommandSourceStack};
use crate::commands::{CommandFn, CustomCommandFn};
use crate::network::chat::{Arg, Component};
use crate::server::Server;

#[cfg(windows)]
const LINE_SEPARATOR: &str = "\r\n";
#[cfg(not(windows))]
const LINE_SEPARATOR: &str = "\n";

static ERROR_NOT_RUNNING: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.debug.notRunning");
static ERROR_ALREADY_RUNNING: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.debug.alreadyRunning");
static NO_RECURSIVE_TRACES: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.debug.function.noRecursion");
static NO_RETURN_RUN: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.debug.function.noReturnRun");

pub struct TimeProfiler {
    start_nanos: Instant,
    start_tick: i32,
}

impl TimeProfiler {
    pub fn new(start_tick: i32) -> TimeProfiler {
        TimeProfiler { start_nanos: Instant::now(), start_tick }
    }
}

pub fn executors() -> Vec<(String, CommandFn)> {
    vec![("debug start".into(), start), ("debug stop".into(), stop)]
}

pub fn custom_executors() -> Vec<(String, CustomCommandFn)> {
    vec![("debug function name".into(), trace)]
}

fn start(server: &mut Server, _context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    if server.is_time_profiler_running() {
        return Err(ERROR_ALREADY_RUNNING.create());
    }
    server.start_time_profiler();
    source.send_success(server, || Component::translatable("commands.debug.started"), true);
    Ok(0)
}

fn stop(server: &mut Server, _context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    if !server.is_time_profiler_running() {
        return Err(ERROR_NOT_RUNNING.create());
    }
    let (nano_duration, tick_duration) = match server.stop_time_profiler() {
        Some(profiler) => (profiler.start_nanos.elapsed().as_nanos() as f64, server.tick_count - profiler.start_tick),
        None => (0.0, 0),
    };
    let seconds = nano_duration / 1.0e9;
    let tps = tick_duration as f64 / seconds;
    source.send_success(
        server,
        || tr("commands.debug.stopped", vec![Arg::String(format!("{seconds:.2}")), Arg::Int(tick_duration), Arg::String(format!("{tps:.2}"))]),
        true,
    );
    Ok(tps as i32)
}

pub struct Tracer {
    output: BufWriter<File>,
    last_indent: i32,
    waiting_for_result: bool,
}

impl Tracer {
    fn write(&mut self, text: &str) {
        let _ = self.output.write_all(text.as_bytes());
    }

    fn println(&mut self, text: &str) {
        self.write(text);
        self.write(LINE_SEPARATOR);
    }

    fn indent_and_save(&mut self, value: i32) {
        self.print_indent(value);
        self.last_indent = value;
    }

    fn print_indent(&mut self, value: i32) {
        for _ in 0..value + 1 {
            self.write("    ");
        }
    }

    fn new_line(&mut self) {
        if self.waiting_for_result {
            self.write(LINE_SEPARATOR);
            self.waiting_for_result = false;
        }
    }

    pub fn on_command(&mut self, depth: i32, command: &str) {
        self.new_line();
        self.indent_and_save(depth);
        self.write("[C] ");
        self.write(command);
        self.waiting_for_result = true;
    }

    pub fn on_return(&mut self, depth: i32, command: &str, result: i32) {
        if self.waiting_for_result {
            self.write(" -> ");
            self.println(&result.to_string());
            self.waiting_for_result = false;
        } else {
            self.indent_and_save(depth);
            self.write(&format!("[R = {result}] "));
            self.println(command);
        }
    }

    pub fn on_call(&mut self, depth: i32, function: &str, size: usize) {
        self.new_line();
        self.indent_and_save(depth);
        self.write("[F] ");
        self.write(function);
        self.write(" size=");
        self.println(&size.to_string());
    }

    pub fn on_error(&mut self, message: &str) {
        self.new_line();
        self.indent_and_save(self.last_indent + 1);
        self.write("[E] ");
        self.write(message);
    }

    pub fn send_system_message(&mut self, message: &str) {
        self.new_line();
        self.print_indent(self.last_indent + 1);
        self.write("[M] ");
        self.println(message);
    }
}

fn trace(server: &mut Server, source: &CommandSourceStack, current_step: &ChainStage, modifiers: ChainModifiers, output: &ExecutionControl) {
    if let Err(e) = trace_guarded(server, source, current_step, modifiers, output) {
        source.handle_error(server, e, modifiers.is_forked());
    }
}

fn trace_guarded(server: &mut Server, source: &CommandSourceStack, current_step: &ChainStage, modifiers: ChainModifiers, output: &ExecutionControl) -> Result<(), CommandSyntaxException> {
    if modifiers.is_return() {
        return Err(NO_RETURN_RUN.create());
    }
    if ExecutionContext::with(|context| context.tracer().is_some()) {
        return Err(NO_RECURSIVE_TRACES.create());
    }
    let current_context = current_step.top_context();
    let (_, functions) = function_argument::get_function_collection(server, current_context, "name")?;
    let output_name = format!("debug-trace-{}.txt", Local::now().format("%Y-%m-%d_%H.%M.%S"));
    let mut tracker = CommandResponseTracker::new();
    let dir_path = Path::new("debug");
    match std::fs::create_dir_all(dir_path).and_then(|_| File::create(dir_path.join(&output_name))) {
        Ok(file) => {
            ExecutionContext::with(|context| context.set_tracer(Tracer { output: BufWriter::new(file), last_indent: 0, waiting_for_result: false }));
            for function in &functions {
                match function.instantiate(None) {
                    Ok(instantiated_function) => {
                        let function_source = CommandSourceStack { source: CommandSource::Tracer, ..source.clone() }.with_maximum_permission(2);
                        let id = function.id().to_string();
                        let size = instantiated_function.entries.len() as i32;
                        output.queue_next(EntryAction::Custom(Box::new(move |_, frame| {
                            ExecutionContext::with(|context| context.tracer().map(|tracer| tracer.println(&id)));
                            CallFunction { function: instantiated_function, result_callback: CommandResultCallback::EMPTY, return_parent_frame: false }.execute(function_source, frame);
                        })));
                        tracker.track_value(function.id().to_string(), size);
                    }
                    Err(message) => source.send_failure(server, message),
                }
            }
        }
        Err(e) => {
            warn!("Tracing failed\n{e}");
            source.send_failure(server, Component::translatable("commands.debug.function.traceFailed"));
        }
    }
    let source = source.clone();
    output.queue_next(EntryAction::Custom(Box::new(move |server, _| {
        source.send_success(
            server,
            || {
                tracker.dispatch_any(
                    |function, total_value| {
                        tr(
                            "commands.debug.function.success.single",
                            vec![Arg::Int(total_value), Arg::Component(Component::literal(&function)), Arg::String(output_name.clone())],
                        )
                    },
                    |function_count, total_value| {
                        tr("commands.debug.function.success.multiple", vec![Arg::Int(total_value), Arg::Int(function_count), Arg::String(output_name.clone())])
                    },
                )
            },
            true,
        )
    })));
    Ok(())
}
