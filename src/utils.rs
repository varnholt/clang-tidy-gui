use std::fs::File;
use std::path::Path;
use std::io::Write;
use std::process::{Command, ExitStatus};
use std::thread;
use std::time::Duration;
use crate::config::{Config, Fix};


// run process
// let command = "calc"; // Command to run the Calculator application on Windows
// let args: &[&str] = &[]; // Arguments to pass to the command
// let status = run_process_and_wait(command, args)?;
// println!("Process exited with status: {}", status);

// run_clang_tidy();

pub fn process_fixes(config: Config)
{
    // simulate a long-running task
    thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_secs(5));
        println!("Task completed!");
    });
}


fn run_process_and_wait(command: &str, args: &[&str]) -> std::io::Result<ExitStatus> {
    let mut child = Command::new(command)
        .args(args)
        .spawn()?;

    let status = child.wait()?;
    Ok(status)
}


/// Writes a .clang-tidy file with the specified clang-tidy checks.
///
/// # Arguments
///
/// * `path` - The path where the .clang-tidy file should be created.
/// * `checks` - A list of clang-tidy checks to be enabled.
fn write_clang_tidy_file<P: AsRef<Path>>(path: P, checks: &[&str]) -> std::io::Result<()> {
    // Create or open the file at the specified path
    let mut file = File::create(path)?;

    // Construct the checks string
    let checks_str = format!("-*{}", checks.iter().map(|&check| format!(",{}", check)).collect::<String>());

    // Define the contents of the .clang-tidy file
    let contents = format!(
        r#"Checks: '{}'
WarningsAsErrors: ''
HeaderFilterRegex: ''
AnalyzeTemporaryDtors: false
FormatStyle: none
User:
ExtraArgs: []
ExtraArgsBefore: []
"#,
        checks_str
    );

    // Write the contents to the file
    file.write_all(contents.as_bytes())?;

    Ok(())
}


fn run_clang_tidy() -> std::io::Result<()> {
    // Specify the path to the .clang-tidy file
    let file_path = "path/to/.clang-tidy";

    // Specify the list of clang-tidy checks to enable
    let checks = vec![
        "modernize-use-nullptr",
        "modernize-loop-convert",
        "modernize-use-auto",
    ];

    // Write the .clang-tidy file with the specified checks
    write_clang_tidy_file(file_path, &checks)?;

    println!("Successfully wrote .clang-tidy file at {}", file_path);

    Ok(())
}
