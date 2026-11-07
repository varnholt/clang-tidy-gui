use crate::config::Config;
use std::{
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

#[derive(Debug)]
pub enum Event {
    Catalogue(Result<Vec<String>, String>),
    Started {
        name: String,
        completed: usize,
        total: usize,
    },
    Log(String),
    Finished(Result<usize, String>),
}

pub fn discover_checks(binary: &str) -> Result<Vec<String>, String> {
    let binary = resolve_program(binary)
        .ok_or("clang-tidy was not found. Set its path in Runner options.")?;
    let mut command = Command::new(binary);
    command.args(["-list-checks", "-checks=*"]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    read_checks(command, Duration::from_secs(10))
}

fn read_checks(mut command: Command, timeout: Duration) -> Result<Vec<String>, String> {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    let stdout = child.stdout.take().ok_or("Cannot capture check list")?;
    let stderr = child
        .stderr
        .take()
        .ok_or("Cannot capture discovery errors")?;
    // Consume output concurrently while polling the child, including lists larger than a pipe buffer.
    let capture = |reader: Box<dyn Read + Send>| {
        thread::spawn(move || {
            let mut bytes = Vec::new();
            reader
                .take(1_048_576)
                .read_to_end(&mut bytes)
                .map(|_| bytes)
        })
    };
    let stdout = capture(Box::new(stdout));
    let stderr = capture(Box::new(stderr));
    let started = Instant::now();
    let result = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) if started.elapsed() < timeout => thread::sleep(Duration::from_millis(20)),
            Ok(None) => {
                break Err(format!(
                    "Check discovery timed out after {} seconds.",
                    timeout.as_secs_f32()
                ))
            }
            Err(error) => break Err(format!("Cannot wait for check discovery: {error}")),
        }
    };
    if result.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }
    let stdout = stdout
        .join()
        .map_err(|_| "Check-list reader failed")?
        .map_err(|e| e.to_string())?;
    let stderr = stderr
        .join()
        .map_err(|_| "Discovery error reader failed")?
        .map_err(|e| e.to_string())?;
    let status = result?;
    if stdout.len() == 1_048_576 || stderr.len() == 1_048_576 {
        return Err("Check discovery exceeded the output limit.".into());
    }
    if !status.success() {
        return Err(format!(
            "Cannot list checks: {}",
            String::from_utf8_lossy(&stderr)
        ));
    }
    parse_checks(&String::from_utf8_lossy(&stdout))
}

fn parse_checks(output: &str) -> Result<Vec<String>, String> {
    let mut names: Vec<_> = output
        .lines()
        .map(str::trim)
        .filter(|line| {
            line.contains('-')
                && line
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
        })
        .map(str::to_owned)
        .collect();
    names.sort();
    names.dedup();
    if names.is_empty() {
        return Err("clang-tidy returned no checks.".into());
    }
    Ok(names)
}

fn resolve_program(value: &str) -> Option<PathBuf> {
    let path = Path::new(value);
    if path.is_file() {
        return path.canonicalize().ok();
    }
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths).find_map(|directory| {
            let candidate = directory.join(value);
            if candidate.is_file() {
                Some(candidate)
            } else if cfg!(windows) && candidate.with_extension("exe").is_file() {
                Some(candidate.with_extension("exe"))
            } else {
                None
            }
        })
    })
}

pub fn validate(config: &Config) -> Result<(), String> {
    if config.selected_count() == 0 {
        return Err("Select at least one check to begin.".into());
    }
    if resolve_program(&config.run_clang_tidy_path).is_none() {
        return Err("Choose an existing run-clang-tidy script or executable.".into());
    }
    if resolve_program(&config.clang_tidy_path).is_none() {
        return Err("clang-tidy was not found. Set its path in Runner options.".into());
    }
    let runner = resolve_program(&config.run_clang_tidy_path).ok_or("Runner was not found")?;
    if !runner
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
        && resolve_program(&config.python_path).is_none()
    {
        return Err("Python was not found. Set its path in Runner options.".into());
    }
    if !Path::new(&config.project_path).is_dir() {
        return Err("Choose an existing project working directory.".into());
    }
    let database = Path::new(&config.build_commands_path);
    if database.file_name().and_then(|s| s.to_str()) != Some("compile_commands.json")
        || !database.is_file()
    {
        return Err("Choose the compile_commands.json file exported by your build.".into());
    }
    let contents = std::fs::read_to_string(database)
        .map_err(|e| format!("Cannot read compilation database: {e}"))?;
    let entries: serde_json::Value = serde_json::from_str(&contents)
        .map_err(|e| format!("Invalid compilation database: {e}"))?;
    if !entries.as_array().is_some_and(|a| {
        !a.is_empty()
            && a.iter().all(|e| {
                e["directory"].is_string()
                    && e["file"].is_string()
                    && (e["command"].is_string() || e["arguments"].is_array())
            })
    }) {
        return Err("Compilation database must contain compile command entries (directory, file, and command or arguments).".into());
    }
    if config.jobs == 0 {
        return Err("Use at least one worker.".into());
    }
    if config.file_filter.trim().is_empty() {
        return Err("Enter a file regular expression, or .* for all files.".into());
    }
    Ok(())
}

pub fn command(config: &Config, check: &str) -> Result<Command, String> {
    let runner = resolve_program(&config.run_clang_tidy_path).ok_or("Runner was not found")?;
    let executable = runner
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"));
    let mut command = if executable {
        Command::new(&runner)
    } else {
        let python = resolve_program(&config.python_path)
            .ok_or("Python was not found. Set its path in Project setup.")?;
        let mut command = Command::new(python);
        command.arg("-u").arg(&runner);
        command
    };
    let database = Path::new(&config.build_commands_path)
        .canonicalize()
        .map_err(|e| e.to_string())?;
    // The script takes a directory, not a JSON filename. Pass each argument separately.
    command
        .arg("-p")
        .arg(
            database
                .parent()
                .ok_or("Database has no parent directory")?,
        )
        .arg("-j")
        .arg(config.jobs.to_string())
        .arg(format!("-checks=-*,{check}"));
    command.arg("-clang-tidy-binary").arg(
        resolve_program(&config.clang_tidy_path)
            .ok_or("clang-tidy was not found. Set its path in Runner options.")?,
    );
    if config.apply_fixes {
        command.arg("-fix");
    }
    command
        .arg("--")
        .arg(&config.file_filter)
        .current_dir(&config.project_path);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    Ok(command)
}

/// Runs checks sequentially. A stop request lets the active check finish writing files.
pub fn process_fixes(config: Config, stop: Arc<AtomicBool>, emit: impl Fn(Event) + Sync) {
    let result = (|| {
        validate(&config)?;
        let checks: Vec<_> = config.fixes.iter().filter(|fix| fix.enabled).collect();
        for (index, check) in checks.iter().enumerate() {
            if stop.load(Ordering::Acquire) {
                return Err(format!("Stopped after {index} of {} checks.", checks.len()));
            }
            emit(Event::Started {
                name: check.name.clone(),
                completed: index,
                total: checks.len(),
            });
            let mut command = command(&config, &check.name)?;
            emit(Event::Log(format!("$ {command:?}")));
            let mut child = command
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|e| format!("Could not start runner: {e}"))?;
            let stdout = child.stdout.take().ok_or("Cannot capture stdout")?;
            let stderr = child.stderr.take().ok_or("Cannot capture stderr")?;
            // Drain both streams concurrently so a full stderr pipe cannot deadlock the runner.
            thread::scope(|scope| {
                for reader in [
                    Box::new(stdout) as Box<dyn std::io::Read + Send>,
                    Box::new(stderr),
                ] {
                    let emit = &emit;
                    scope.spawn(move || {
                        let mut reader = BufReader::new(reader);
                        loop {
                            let mut bytes = Vec::new();
                            match Read::by_ref(&mut reader)
                                .take(16_384)
                                .read_until(b'\n', &mut bytes)
                            {
                                Ok(0) => break,
                                Ok(_) => emit(Event::Log(
                                    String::from_utf8_lossy(&bytes)
                                        .trim_end_matches(['\r', '\n'])
                                        .to_string(),
                                )),
                                Err(error) => {
                                    emit(Event::Log(format!("Output read error: {error}")));
                                    break;
                                }
                            }
                        }
                    });
                }
            });
            let status = child
                .wait()
                .map_err(|e| format!("Cannot wait for runner: {e}"))?;
            if !status.success() {
                return Err(format!(
                    "{} failed ({status}). See the run output.",
                    check.name
                ));
            }
        }
        Ok(checks.len())
    })();
    emit(Event::Finished(result));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Fix;
    use std::sync::Mutex;

    fn fixture() -> (tempfile::TempDir, Config) {
        let dir = tempfile::Builder::new()
            .prefix("tidy project with spaces ")
            .tempdir()
            .unwrap();
        let runner = dir.path().join("fake runner.py");
        std::fs::write(&runner, "import json, sys\nprint(json.dumps(sys.argv[1:]), flush=True)\nprint('diagnostic on stderr', file=sys.stderr, flush=True)\n").unwrap();
        let database = dir.path().join("compile_commands.json");
        std::fs::write(
            &database,
            r#"[{"directory":".","file":"main.cpp","arguments":["clang++","main.cpp"]}]"#,
        )
        .unwrap();
        let config = Config {
            run_clang_tidy_path: runner.display().to_string(),
            build_commands_path: database.display().to_string(),
            project_path: dir.path().display().to_string(),
            // Tests use Python as an existing executable; the fake runner never invokes clang-tidy.
            clang_tidy_path: if cfg!(windows) { "python" } else { "python3" }.into(),
            fixes: vec![
                Fix {
                    name: "modernize-use-nullptr".into(),
                    enabled: true,
                },
                Fix {
                    name: "readability-braces-around-statements".into(),
                    enabled: true,
                },
            ],
            ..Config::default()
        };
        (dir, config)
    }

    #[test]
    fn arguments_preserve_spaces_and_analysis_does_not_apply_fixes() {
        let (_dir, mut config) = fixture();
        let command = command(&config, "modernize-use-nullptr").unwrap();
        let args: Vec<_> = command
            .get_args()
            .map(|s| s.to_string_lossy().into_owned())
            .collect();
        let p = args.iter().position(|s| s == "-p").unwrap();
        assert!(Path::new(&args[p + 1])
            .join("compile_commands.json")
            .is_file());
        assert!(args.contains(&"-checks=-*,modernize-use-nullptr".into()));
        assert!(!args.contains(&"-fix".into()));
        assert!(!args.contains(&"-j 10".into()));
        assert_eq!(&args[args.len() - 2..], &["--", ".*"]);
        config.apply_fixes = true;
        assert!(command_for_test(&config).get_args().any(|s| s == "-fix"));
    }

    fn command_for_test(config: &Config) -> Command {
        command(config, "modernize-use-nullptr").unwrap()
    }

    #[test]
    fn runner_streams_both_outputs_and_preserves_project_config() {
        let (dir, config) = fixture();
        let path = dir.path().join(".clang-tidy");
        std::fs::write(&path, "Checks: '*'\nCheckOptions: {}\n").unwrap();
        let events = Mutex::new(Vec::new());
        process_fixes(config, Arc::new(AtomicBool::new(false)), |e| {
            events.lock().unwrap().push(e)
        });
        let events = events.into_inner().unwrap();
        assert!(matches!(events.last(), Some(Event::Finished(Ok(2)))));
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, Event::Started { .. }))
                .count(),
            2
        );
        assert!(events
            .iter()
            .any(|e| matches!(e, Event::Log(line) if line == "diagnostic on stderr")));
        assert_eq!(
            std::fs::read_to_string(path).unwrap(),
            "Checks: '*'\nCheckOptions: {}\n"
        );
    }

    #[test]
    fn failure_stops_the_queue_and_reports_exit_status() {
        let (_dir, config) = fixture();
        std::fs::write(
            &config.run_clang_tidy_path,
            "import sys\nprint('broken runner', file=sys.stderr)\nsys.exit(7)\n",
        )
        .unwrap();
        let events = Mutex::new(Vec::new());
        process_fixes(config, Arc::new(AtomicBool::new(false)), |e| {
            events.lock().unwrap().push(e)
        });
        let events = events.into_inner().unwrap();
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, Event::Started { .. }))
                .count(),
            1
        );
        assert!(
            matches!(events.last(), Some(Event::Finished(Err(message))) if message.contains("failed"))
        );
    }

    #[test]
    fn stop_finishes_the_active_check_and_a_fresh_run_can_start() {
        let (_dir, config) = fixture();
        let stop = Arc::new(AtomicBool::new(false));
        let events = Mutex::new(Vec::new());
        process_fixes(config.clone(), Arc::clone(&stop), |event| {
            if matches!(event, Event::Started { .. }) {
                stop.store(true, Ordering::Release);
            }
            events.lock().unwrap().push(event);
        });
        let events = events.into_inner().unwrap();
        assert!(events
            .iter()
            .any(|e| matches!(e, Event::Log(line) if line == "diagnostic on stderr")));
        assert!(
            matches!(events.last(), Some(Event::Finished(Err(message))) if message.contains("Stopped after 1 of 2"))
        );
        let events = Mutex::new(Vec::new());
        process_fixes(config, Arc::new(AtomicBool::new(false)), |e| {
            events.lock().unwrap().push(e)
        });
        assert!(matches!(
            events.lock().unwrap().last(),
            Some(Event::Finished(Ok(2)))
        ));
    }

    #[test]
    fn invalid_database_and_empty_selection_are_rejected() {
        let (_dir, mut config) = fixture();
        std::fs::write(&config.build_commands_path, "[]").unwrap();
        assert!(validate(&config).unwrap_err().contains("entries"));
        config.fixes.clear();
        assert!(validate(&config).unwrap_err().contains("Select"));
    }

    #[test]
    fn catalogue_parser_rejects_noise_and_empty_output() {
        let names = parse_checks("Enabled checks:\n    modernize-use-nullptr\n    bugprone-branch-clone\n    clang-analyzer-core.CallAndMessage\n    clang-analyzer-optin.cplusplus.UninitializedObject\n    modernize-use-nullptr\nwarning: ignored\n").unwrap();
        assert_eq!(
            names,
            [
                "bugprone-branch-clone",
                "clang-analyzer-core.CallAndMessage",
                "clang-analyzer-optin.cplusplus.UninitializedObject",
                "modernize-use-nullptr"
            ]
        );
        assert!(parse_checks("Enabled checks:\n").is_err());
    }

    #[test]
    fn discovery_drains_large_output_and_times_out_a_stuck_tool() {
        let python = resolve_program(&Config::default().python_path).unwrap();
        let mut command = Command::new(&python);
        command.args(["-u", "-c", "import sys; sys.stderr.write('x' * 100000); print('Enabled checks:'); [print('    test-check-' + str(i)) for i in range(4000)]"]);
        assert_eq!(
            read_checks(command, Duration::from_secs(10)).unwrap().len(),
            4000
        );
        let mut command = Command::new(&python);
        command.args(["-u", "-c", "import time; time.sleep(30)"]);
        let started = Instant::now();
        assert!(read_checks(command, Duration::from_millis(100))
            .unwrap_err()
            .contains("timed out"));
        assert!(started.elapsed() < Duration::from_secs(5));
    }
}

#[cfg(test)]
mod llvm_smoke_test {
    use super::*;
    use crate::config::Fix;
    use std::sync::Mutex;

    /// Opt in with CLANG_TIDY_GUI_RUNNER pointing to LLVM's run-clang-tidy script.
    #[test]
    #[ignore = "requires Python, LLVM clang-tidy and clang-apply-replacements on PATH"]
    fn installed_llvm_analyzes_then_applies_a_fix() {
        let checks = discover_checks("clang-tidy").unwrap();
        assert!(checks.len() > 200);
        assert!(checks.iter().any(|name| name == "modernize-use-nullptr"));
        assert!(checks
            .iter()
            .any(|name| name == "clang-analyzer-core.CallAndMessage"));
        let dir = tempfile::Builder::new()
            .prefix("tidy real project ")
            .tempdir()
            .unwrap();
        let source = dir.path().join("main.cpp");
        let original = "int main() { int *p = 0; return p != 0; }\n";
        std::fs::write(&source, original).unwrap();
        let tidy = dir.path().join(".clang-tidy");
        let tidy_contents = "Checks: '-*'\nFormatStyle: none\n";
        std::fs::write(&tidy, tidy_contents).unwrap();
        let database = dir.path().join("compile_commands.json");
        let entries = serde_json::json!([{
            "directory": dir.path(), "file": source,
            "arguments": ["clang++", "-std=c++11", "-c", source]
        }]);
        std::fs::write(&database, entries.to_string()).unwrap();
        let mut config = Config {
            run_clang_tidy_path: std::env::var("CLANG_TIDY_GUI_RUNNER")
                .unwrap_or("run-clang-tidy".into()),
            project_path: dir.path().display().to_string(),
            build_commands_path: database.display().to_string(),
            fixes: vec![Fix {
                name: "modernize-use-nullptr".into(),
                enabled: true,
            }],
            ..Config::default()
        };
        for apply in [false, true] {
            config.apply_fixes = apply;
            let events = Mutex::new(Vec::new());
            process_fixes(config.clone(), Arc::new(AtomicBool::new(false)), |e| {
                events.lock().unwrap().push(e)
            });
            let events = events.into_inner().unwrap();
            assert!(
                matches!(events.last(), Some(Event::Finished(Ok(1)))),
                "{events:#?}"
            );
            let contents = std::fs::read_to_string(&source).unwrap();
            if apply {
                assert!(contents.contains("nullptr"), "{events:#?}");
            } else {
                assert_eq!(contents, original);
            }
            assert_eq!(std::fs::read_to_string(&tidy).unwrap(), tidy_contents);
        }
    }
}
