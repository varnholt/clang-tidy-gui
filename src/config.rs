use std::fs::File;
use std::io;
use std::io::{BufRead, Read};
use std::path::Path;
use serde::{Deserialize, Serialize};
use crate::config;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub run_clang_tidy_path: String,
    pub build_commands_path: String,
    pub project_path: String,
    pub build_commands_file_path: String,
    pub fixes: Vec<Fix>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Fix {
    pub name: String,
    pub enabled: bool,
}


impl Config {
    pub fn new() -> Self {
        Config {
            run_clang_tidy_path: String::new(),
            build_commands_path: String::new(),
            project_path: String::new(),
            build_commands_file_path: String::new(),
            fixes: Vec::new(),
        }
    }

    pub fn from_json_file<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let mut file = File::open(path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        let config: Config = serde_json::from_str(&contents)?;
        Ok(config)
    }

    pub fn to_json_file<P: AsRef<std::path::Path>>(&self, path: P) -> Result<(), Box<dyn std::error::Error>> {
        let file = File::create(path)?;
        serde_json::to_writer_pretty(file, &self)?;
        Ok(())
    }

    pub fn save(&self) {
        if let Err(e) = self.to_json_file("config.json") {
            eprintln!("Failed to save config: {}", e);
        } else {
            println!("Config saved successfully");
        }
    }
    pub fn set_run_clang_tidy_path(&mut self, path: String) {
        self.run_clang_tidy_path = path;
        self.save();
    }

    pub fn set_project_path(&mut self, path: String) {
        self.project_path = path;
        self.save();
    }

    pub fn set_build_commands_path(&mut self, path: String) {
        self.build_commands_path = path;
        self.save();
    }

    pub fn set_fix_enabled(&mut self, fix_name: &str, enabled: bool) {
        if let Some(fix) = self.fixes.iter_mut().find(|fix| fix.name == fix_name) {
            fix.enabled = enabled;
            self.save();
        }
    }
}

pub(crate) fn load_fixes<P: AsRef<Path>>(path: P) -> Result<Vec<config::Fix>, io::Error> {
    let file = File::open(path)?;
    let reader = io::BufReader::new(file);
    let mut fixes = Vec::new();

    for line in reader.lines() {
        let line = line?;
        fixes.push(config::Fix{name: line, enabled: false});
    }

    Ok(fixes)
}

