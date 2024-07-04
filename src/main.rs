use std::io::{self, BufRead};
use std::io::Read;
use std::io::Write;
use serde::{Deserialize, Serialize};
use crate::config::Config;
use crate::ui::Application;

mod ui;
mod config;
mod utils;


fn main() -> Result<(), Box<dyn std::error::Error>> {
    
    // read config
    let config_path = "config.json";
    let mut config = Config::from_json_file(config_path).unwrap_or_else(|_| {
        println!("Failed to load config, continuing with empty config.");
        Config::new()
    });
    println!("{:?}", config);

    // merge with available fixes
    let file_path = "fixes.txt";
    let available_fixes = config::load_fixes(file_path)?;
    for available_fix in available_fixes {

        if !config.fixes.iter().any(|fix| fix.name == available_fix.name) {
            config.fixes.push(available_fix.clone());
        }

        println!("[{}] {}", if available_fix .enabled { "x" } else { " " }, available_fix .name);
    }

    let app = Application::new(config);
    let options = eframe::NativeOptions::default();
    eframe::run_native("fixcpp", options, Box::new(|_cc| Box::new(app)));

    Ok(())
}