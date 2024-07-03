use std::fs::File;
use std::io::{self, BufRead};
use std::io::Read;
use std::io::Write;
use std::path::Path;
use std::process::{Command, ExitStatus};
use serde::{Deserialize, Serialize};
use crate::config::Config;
use crate::ui::Application;

mod ui;
mod config;
mod utils;

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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    
    // read config
    let config_path = "config.json";
    let mut config = Config::from_json_file(config_path).unwrap_or_else(|_| {
        println!("Failed to load config, continuing with empty config.");
        Config::new()
    });
    println!("{:?}", config);

    // read fixes   
    let file_path = "fixes.txt";
    let available_fixes = config::load_fixes(file_path)?;
    for available_fix in available_fixes {

        if !config.fixes.iter().any(|fix| fix.name == available_fix.name) {
            config.fixes.push(available_fix.clone());
        }

        println!("[{}] {}", if available_fix .enabled { "x" } else { " " }, available_fix .name);
    }

    // run process
    // let command = "calc"; // Command to run the Calculator application on Windows
    // let args: &[&str] = &[]; // Arguments to pass to the command
    // let status = run_process_and_wait(command, args)?;
    // println!("Process exited with status: {}", status);

    // run_clang_tidy();

    let app = Application::new(config);
    let options = eframe::NativeOptions::default();
    eframe::run_native("fixcpp", options, Box::new(|_cc| Box::new(app)));

    Ok(())
}