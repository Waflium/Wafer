use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::Path;

use super::Server;
use super::main_thread_frames;
use crate::crash_report::{CrashReport, ReportType};
use crate::util::csv_output;
use crate::util::java_collections::identity_hash_code;
use crate::util::java_format_fixed;
use crate::util::java_throwable::JavaThrowable;
use crate::world::level::storage::format_uuid;

#[cfg(windows)]
const LINE_SEPARATOR: &str = "\r\n";
#[cfg(not(windows))]
const LINE_SEPARATOR: &str = "\n";

const SERVER_LEVEL_SAVE_DEBUG_REPORT_THROW: &str = "net.minecraft.server.level.ServerLevel.saveDebugReport(ServerLevel.java:1559)";
const SERVER_LEVEL_SAVE_DEBUG_REPORT_FILL: &str = "net.minecraft.server.level.ServerLevel.saveDebugReport(ServerLevel.java:1560)";
const FILL_REPORT_DETAILS: &str = "net.minecraft.server.level.ServerLevel.fillReportDetails(ServerLevel.java:1892)";
const SERVER_SAVE_DEBUG_REPORT: &str = "net.minecraft.server.MinecraftServer.saveDebugReport(MinecraftServer.java:1827)";
const CREATE_PROFILER_LAMBDA: &str = "net.minecraft.server.MinecraftServer.lambda$createProfiler$1(MinecraftServer.java:1917)";
const SUBMIT_ASYNC_LAMBDA: &str = "net.minecraft.util.thread.BlockableEventLoop.lambda$submitAsync$0(BlockableEventLoop.java:75)";
const ASYNC_SUPPLY_RUN: &str = "java.base/java.util.concurrent.CompletableFuture$AsyncSupply.run(CompletableFuture.java:1789)";

impl Server {
    pub(crate) fn save_debug_report(&mut self, output: &Path, blocked_io_worker: &str) {
        let levels_dir = output.join("levels");
        let saved = (|| -> io::Result<()> {
            for index in 0..self.levels.len() {
                let dimension = self.levels[index].dimension.to_string();
                let (namespace, path) = dimension.split_once(':').unwrap_or(("minecraft", dimension.as_str()));
                let level_path = levels_dir.join(namespace).join(path);
                fs::create_dir_all(&level_path)?;
                self.save_level_debug_report(index, &level_path)?;
            }
            self.dump_game_rules(&output.join("gamerules.txt"))?;
            self.dump_classpath(&output.join("classpath.txt"))?;
            self.dump_misc_stats(&output.join("stats.txt"))?;
            fs::write(output.join("threads.txt"), super::dedicated::server_watchdog::dump_threads_for_debug_report(blocked_io_worker))?;
            self.dump_server_properties(&output.join("server.properties.txt"))?;
            self.dump_native_modules(&output.join("modules.txt"))
        })();
        if let Err(e) = saved {
            warn!("Failed to save debug report\njava.io.IOException: {e}");
        }
    }

    fn save_level_debug_report(&mut self, index: usize, root_dir: &Path) -> io::Result<()> {
        {
            let mut output = BufWriter::new(File::create(root_dir.join("stats.txt"))?);
            self.levels[index].write_debug_stats(&mut output)?;
            output.flush()?;
        }
        let _task_scope = main_thread_frames::enter(main_thread_frames::SERVER_TASK_POLL);
        let mut tail_frames = vec![SERVER_SAVE_DEBUG_REPORT, CREATE_PROFILER_LAMBDA, SUBMIT_ASYNC_LAMBDA, ASYNC_SUPPLY_RUN];
        tail_frames.extend(main_thread_frames::current());
        let mut thrown_frames = vec![SERVER_LEVEL_SAVE_DEBUG_REPORT_THROW];
        thrown_frames.extend(tail_frames.iter().copied());
        let mut test = CrashReport::of_exception("Level dump", &JavaThrowable::new("java.lang.Exception: dummy", thrown_frames));
        let mut fill_frames = vec![FILL_REPORT_DETAILS, SERVER_LEVEL_SAVE_DEBUG_REPORT_FILL];
        fill_frames.extend(tail_frames);
        self.fill_level_report_details(index, &mut test, fill_frames);
        fs::write(root_dir.join("example_crash.txt"), test.get_friendly_report(&ReportType::TEST))?;
        let level = &self.levels[index];
        let mut chunks = BufWriter::new(File::create(root_dir.join("chunks.csv"))?);
        level.dump_chunks(&mut chunks)?;
        chunks.flush()?;
        let mut entity_chunks = BufWriter::new(File::create(root_dir.join("entity_chunks.csv"))?);
        level.dump_entity_sections(&mut entity_chunks)?;
        entity_chunks.flush()?;
        let mut entities = BufWriter::new(File::create(root_dir.join("entities.csv"))?);
        self.dump_entities(index, &mut entities)?;
        entities.flush()?;
        let mut block_entities = BufWriter::new(File::create(root_dir.join("block_entities.csv"))?);
        level.dump_block_entity_tickers(&mut block_entities)?;
        block_entities.flush()
    }

    fn dump_entities(&self, index: usize, output: &mut impl Write) -> io::Result<()> {
        let level = &self.levels[index];
        let mut csv_output = csv_output::builder()
            .add_column("x")
            .add_column("y")
            .add_column("z")
            .add_column("uuid")
            .add_column("type")
            .add_column("alive")
            .add_column("display_name")
            .add_column("custom_name")
            .build(output)?;
        for id in level.entity_manager.visible_entity_storage.all_entities() {
            let Some(entity) = level.entity_ref(id) else { continue };
            csv_output.write_row(&[
                Some(crate::nbt::java_double_to_string(entity.x())),
                Some(crate::nbt::java_double_to_string(entity.y())),
                Some(crate::nbt::java_double_to_string(entity.z())),
                Some(format_uuid(entity.uuid)),
                Some(level.entity_registered_name(id)),
                Some(level.is_alive(id).to_string()),
                Some(self.entity_display_name(id).get_string()),
                entity.custom_name().map(|name| name.get_string()),
            ])?;
        }
        Ok(())
    }

    fn dump_game_rules(&self, path: &Path) -> io::Result<()> {
        let mut output = BufWriter::new(File::create(path)?);
        for rule in self.game_rules.available() {
            writeln!(output, "minecraft:{}={}", rule.info().name, crate::world::level::gamerules::GameRules::serialize(rule, self.game_rules.get(rule)))?;
        }
        output.flush()
    }

    fn dump_classpath(&self, path: &Path) -> io::Result<()> {
        let mut output = BufWriter::new(File::create(path)?);
        let classpath = std::env::current_exe().map(|exe| crate::util::java_nio::path_string(&exe)).unwrap_or_default();
        for entry in classpath.split(if cfg!(windows) { ';' } else { ':' }) {
            writeln!(output, "{entry}")?;
        }
        output.flush()
    }

    fn dump_misc_stats(&self, path: &Path) -> io::Result<()> {
        let mut output = BufWriter::new(File::create(path)?);
        writeln!(output, "pending_tasks: {}", self.tasks.borrow().len())?;
        writeln!(output, "average_tick_time: {}", java_format_fixed(f64::from(self.smoothed_tick_time_millis), 6))?;
        let tick_times: Vec<String> = self.tick_times_nanos.iter().map(ToString::to_string).collect();
        writeln!(output, "tick_times: [{}]", tick_times.join(", "))?;
        writeln!(
            output,
            "queue: TracingExecutor[service=java.util.concurrent.ForkJoinPool@{:x}[Running, parallelism = {workers}, size = {workers}, active = 0, running = 0, steals = 0, tasks = 0, submissions = 0]]",
            identity_hash_code(),
            workers = super::level::chunk_workers::worker_main_count()
        )?;
        output.flush()
    }

    fn dump_server_properties(&self, path: &Path) -> io::Result<()> {
        let mut output = BufWriter::new(File::create(path)?);
        let config = &self.config;
        let mut line = |text: String| write!(output, "{text}{LINE_SEPARATOR}");
        line(format!("sync-chunk-writes={}", config.sync_chunk_writes))?;
        line(format!("gamemode={}", config.game_mode.name().to_uppercase()))?;
        line(format!("entity-broadcast-range-percentage={}", config.entity_broadcast_range_percentage))?;
        line(format!("max-world-size={}", config.max_world_size))?;
        line(format!("view-distance={}", config.view_distance))?;
        line(format!("simulation-distance={}", config.simulation_distance))?;
        line(format!("generate-structures={}", self.generate_structures))?;
        line(format!("use-native={}", config.use_native_transport))?;
        line(format!("rate-limit={}", config.rate_limit_packets_per_second))?;
        line(format!("command-spam-threshold-seconds={}", config.command_spam_threshold_seconds))?;
        line(format!("chat-spam-threshold-seconds={}", config.chat_spam_threshold_seconds))?;
        output.flush()
    }

    fn dump_native_modules(&self, path: &Path) -> io::Result<()> {
        let mut output = BufWriter::new(File::create(path)?);
        let mut modules = crate::util::native_module_lister::list_modules();
        modules.sort_by_key(|module| module.name.to_lowercase());
        for module in modules {
            writeln!(output, "{module}")?;
        }
        output.flush()
    }
}
