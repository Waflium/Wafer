use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use chrono::{DateTime, Local};

use super::metric_category::MetricCategory;
use super::metric_sampler::MetricSampler;
use crate::util::java_nio::path_string;
use crate::util::csv_output;
use crate::util::profiling::profile_results::ProfileResults;

pub const PROFILING_RESULTS_DIR: &str = "debug/profiling";
pub const METRICS_DIR_NAME: &str = "metrics";
pub const DEVIATIONS_DIR_NAME: &str = "deviations";
pub const PROFILING_RESULT_FILENAME: &str = "profiling.txt";

pub struct RecordedDeviation {
    pub timestamp: SystemTime,
    pub tick: i32,
    pub profiler_result_at_tick: ProfileResults,
}

pub struct MetricsPersister {
    root_folder_name: String,
}

fn valid_path_char(c: char) -> bool {
    c == '_' || c == '-' || c.is_ascii_lowercase() || c.is_ascii_digit() || c == '/' || c == '.'
}

fn sanitize_name(value: &str) -> String {
    value.to_lowercase().chars().map(|c| if valid_path_char(c) { c } else { '_' }).collect()
}

fn create_temp_directory(prefix: &str) -> io::Result<PathBuf> {
    let base = crate::util::java_runtime::tmpdir().map_or_else(std::env::temp_dir, PathBuf::from);
    loop {
        let path = base.join(format!("{prefix}{}", rand::random::<u64>()));
        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
}

impl MetricsPersister {
    pub fn new(root_folder_name: &str) -> MetricsPersister {
        MetricsPersister { root_folder_name: root_folder_name.to_string() }
    }

    pub fn save_reports(&self, samplers: &[MetricSampler], deviations_by_sampler: &[(String, Vec<RecordedDeviation>)], profiler_results: &ProfileResults) -> io::Result<PathBuf> {
        fs::create_dir_all(PROFILING_RESULTS_DIR)?;
        let temp_dir = create_temp_directory("minecraft-profiling")?;
        fs::create_dir_all(PROFILING_RESULTS_DIR)?;
        let working_dir = temp_dir.join(&self.root_folder_name);
        let metrics_dir = working_dir.join(METRICS_DIR_NAME);
        self.save_metrics(samplers, &metrics_dir)?;
        if !deviations_by_sampler.is_empty() {
            self.save_deviations(deviations_by_sampler, &working_dir.join(DEVIATIONS_DIR_NAME));
        }
        self.save_profiling_task_execution_result(profiler_results, &working_dir);
        Ok(temp_dir)
    }

    fn save_metrics(&self, samplers: &[MetricSampler], dir: &Path) -> io::Result<()> {
        if samplers.is_empty() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "java.lang.IllegalArgumentException: Expected at least one sampler to persist"));
        }
        let mut samplers_by_category: Vec<(MetricCategory, Vec<&MetricSampler>)> = Vec::new();
        for sampler in samplers {
            match samplers_by_category.iter_mut().find(|(category, _)| *category == sampler.get_category()) {
                Some((_, in_category)) => in_category.push(sampler),
                None => samplers_by_category.push((sampler.get_category(), vec![sampler])),
            }
        }
        for (category, samplers_in_category) in &samplers_by_category {
            self.save_category(*category, samplers_in_category, dir);
        }
        Ok(())
    }

    fn save_category(&self, category: MetricCategory, samplers: &[&MetricSampler], dir: &Path) {
        let file = dir.join(format!("{}.csv", sanitize_name(category.get_description())));
        let written = (|| -> io::Result<()> {
            fs::create_dir_all(dir)?;
            let mut writer = BufWriter::new(File::create(&file)?);
            let mut csv_builder = csv_output::builder().add_column("@tick");
            for sampler in samplers {
                csv_builder = csv_builder.add_column(sampler.get_name());
            }
            let mut csv_output = csv_builder.build(&mut writer)?;
            let results: Vec<_> = samplers.iter().map(|sampler| sampler.result()).collect();
            let first_tick = results.iter().map(|result| result.get_first_tick()).min().unwrap_or(i32::MAX);
            let last_tick = results.iter().map(|result| result.get_last_tick()).max().unwrap_or(i32::MIN);
            let mut tick = first_tick;
            while tick <= last_tick {
                let mut row = vec![Some(tick.to_string())];
                row.extend(results.iter().map(|result| Some(crate::nbt::java_double_to_string(result.value_at_tick(tick)))));
                csv_output.write_row(&row)?;
                tick += 1;
            }
            writer.flush()
        })();
        match written {
            Ok(()) => info!("Flushed metrics to {}", path_string(&file)),
            Err(e) => error!("Could not save profiler results to {}\n{e}", path_string(&file)),
        }
    }

    fn save_deviations(&self, deviations_by_sampler: &[(String, Vec<RecordedDeviation>)], directory: &Path) {
        for (sampler_name, deviations) in deviations_by_sampler {
            for deviation in deviations {
                let timestamp = DateTime::<Local>::from(deviation.timestamp).format("%Y-%m-%d_%H.%M.%S%.3f").to_string();
                let deviation_log_file = directory.join(sanitize_name(sampler_name)).join(format!("{}@{}.txt", deviation.tick, timestamp));
                deviation.profiler_result_at_tick.save_results(&deviation_log_file);
            }
        }
    }

    fn save_profiling_task_execution_result(&self, results: &ProfileResults, directory: &Path) {
        results.save_results(&directory.join(PROFILING_RESULT_FILENAME));
    }
}
