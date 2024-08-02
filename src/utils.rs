use crate::config::Config;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

pub fn process_fixes(config: Config, cancel_flag: Arc<AtomicBool>, progress: Arc<Mutex<f32>>) {
    let enabled_fixes: Vec<_> = config.fixes.iter().filter(|fix| fix.enabled).collect();
    let total_fixes = enabled_fixes.len() as f32;

    for (index, fix) in enabled_fixes.iter().enumerate() {
        if cancel_flag.load(Ordering::Relaxed) {
            println!("Processing cancelled.");
            break;
        }

        println!("applying fix: {}", fix.name);

        // write clang tidy config file to project dir
        let checks = [&fix.name];
        write_clang_tidy_file(&config.project_path, &checks);

        // start run-clang-tidy
        let args = [&config.run_clang_tidy_path, ".", "-fix", "-j 10"];
        run_process_and_wait("python", &args, &config.project_path);

        // update progress
        let mut progress = progress.lock().unwrap();
        *progress = (index + 1) as f32 / total_fixes * 100.0;
    }

    println!("done!");
}

fn run_process_and_wait(command: &str, args: &[&str], working_directory: &str) {
    println!("executing {} {}", command, args.join(" "));

    let output = Command::new(command)
        .args(args)
        .current_dir(working_directory)
        .output()
        .expect("failed to execute process");

    println!("Output: {:?}", output);
}

/// Writes a .clang-tidy file with the specified clang-tidy checks.
///
/// # Arguments
///
/// * `path` - The path where the .clang-tidy file should be created.
/// * `checks` - A list of clang-tidy checks to be enabled.
fn write_clang_tidy_file<P: AsRef<Path>>(path: P, checks: &[&String]) -> std::io::Result<()> {
    // Create or open the file at the specified path
    let project_path = path.as_ref();
    let clang_tidy_file_path = project_path.join(".clang-tidy");
    let mut file = File::create(&clang_tidy_file_path)?;

    // Construct the checks string
    let checks_str = format!(
        "-*{}",
        checks
            .iter()
            .map(|&check| format!(",{}", check))
            .collect::<String>()
    );

    // Define the contents of the .clang-tidy file
    let contents = format!(
        r#"Checks: '{}'
WarningsAsErrors: ''
HeaderFilterRegex: ''
FormatStyle: none
User: ''
ExtraArgs: []
ExtraArgsBefore: []
"#,
        checks_str
    );

    // Write the contents to the file
    file.write_all(contents.as_bytes())?;

    println!("wrote {}", clang_tidy_file_path.display());

    Ok(())
}
