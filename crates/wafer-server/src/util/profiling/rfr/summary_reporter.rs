use std::io::Write;
use std::path::Path;

use super::parse::rfr_stats_parser::RfrStatsParser;

pub struct SummaryReporter {
    on_deregistration: Box<dyn Fn() + Send + Sync>,
}

impl SummaryReporter {
    pub fn new(on_deregistration: impl Fn() + Send + Sync + 'static) -> SummaryReporter {
        SummaryReporter { on_deregistration: Box::new(on_deregistration) }
    }

    pub fn recording_stopped(&self, result: Option<&Path>) {
        let Some(result) = result else { return };
        (self.on_deregistration)();
        info_with_fallback(&format!("Dumped flight recorder profiling to {}", result.display()));
        let stats_result = match RfrStatsParser::parse(result) {
            Ok(stats_result) => stats_result,
            Err(error) => {
                warn_with_fallback("Failed to parse RFR recording", &error);
                return;
            }
        };
        let output = (|| -> Result<(), String> {
            info_with_fallback(&stats_result.as_json()?);
            let file_name = result.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
            let stem = file_name.split_once(".rfr").map_or(file_name.as_str(), |(stem, _)| stem);
            let json_report = result.with_file_name(format!("rfr-report-{stem}.json"));
            let mut file = std::fs::OpenOptions::new().write(true).create(true).open(&json_report).map_err(|e| e.to_string())?;
            file.write_all(stats_result.as_json()?.as_bytes()).map_err(|e| e.to_string())?;
            info_with_fallback(&format!("Dumped recording summary to {}", json_report.display()));
            Ok(())
        })();
        if let Err(error) = output {
            warn_with_fallback("Failed to output RFR report", &error);
        }
    }
}

fn info_with_fallback(message: &str) {
    if crate::logging::is_logger_active() {
        info!("{message}");
    } else {
        println!("{message}");
    }
}

fn warn_with_fallback(message: &str, error: &str) {
    if crate::logging::is_logger_active() {
        warn!("{message}\n{error}");
    } else {
        println!("{message}");
        eprintln!("{error}");
    }
}
