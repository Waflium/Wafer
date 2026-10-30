use std::path::PathBuf;
use std::process::{Command, Stdio};

use super::file_util::read_all_lines;

fn find_executable(program: &str) -> Option<PathBuf> {
    if program.contains('/') {
        return Some(PathBuf::from(program));
    }
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).map(|directory| directory.join(program)).find(|candidate| candidate.is_file())
}

pub fn run_native(cmd_to_run: &str) -> Vec<String> {
    let mut command: Vec<&str> = cmd_to_run.split(' ').collect();
    while command.len() > 1 && command.last().is_some_and(|last| last.is_empty()) {
        command.pop();
    }
    run_native_args(&command)
}

pub fn run_native_args(cmd_to_run_with_args: &[&str]) -> Vec<String> {
    let Some(program) = cmd_to_run_with_args.first().and_then(|program| find_executable(program)) else { return Vec::new() };
    let output = Command::new(program)
        .args(&cmd_to_run_with_args[1..])
        .env_clear()
        .env(if cfg!(windows) { "LANGUAGE" } else { "LC_ALL" }, "C")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output();
    match output {
        Ok(output) => read_all_lines(&String::from_utf8_lossy(&output.stdout)),
        Err(_) => Vec::new(),
    }
}

pub fn get_first_answer(cmd_to_launch: &str) -> String {
    run_native(cmd_to_launch).into_iter().next().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_programs_yield_no_output() {
        assert!(run_native("definitely-not-a-program --flag").is_empty());
        assert_eq!(get_first_answer("definitely-not-a-program"), "");
    }
}
