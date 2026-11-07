use serde::{Deserialize, Serialize};
use std::{fs, io::Write, path::Path};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(default)]
pub struct Config {
    pub run_clang_tidy_path: String,
    pub python_path: String,
    pub clang_tidy_path: String,
    pub build_commands_path: String,
    pub project_path: String,
    pub file_filter: String,
    pub apply_fixes: bool,
    pub jobs: u32,
    pub catalogue_from_tool: bool,
    pub catalogue_binary: String,
    pub dark_theme: bool,
    pub fixes: Vec<Fix>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Fix {
    pub name: String,
    pub enabled: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            run_clang_tidy_path: "run-clang-tidy".into(),
            python_path: if cfg!(windows) { "python" } else { "python3" }.into(),
            clang_tidy_path: "clang-tidy".into(),
            build_commands_path: String::new(),
            project_path: String::new(),
            file_filter: ".*".into(),
            apply_fixes: false,
            jobs: 1,
            catalogue_from_tool: false,
            catalogue_binary: String::new(),
            dark_theme: true,
            fixes: Vec::new(),
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<Self, String> {
        let contents = fs::read_to_string(path).map_err(|e| e.to_string())?;
        serde_json::from_str(&contents).map_err(|e| e.to_string())
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let contents = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        let mut file = tempfile::NamedTempFile::new_in(path.parent().unwrap_or(Path::new(".")))
            .map_err(|e| e.to_string())?;
        file.write_all(&contents).map_err(|e| e.to_string())?;
        file.as_file().sync_all().map_err(|e| e.to_string())?;
        file.persist(path).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn merge_catalogue(&mut self) {
        if self.catalogue_from_tool {
            return;
        }
        for name in include_str!("../fixes.txt")
            .lines()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            if !self.fixes.iter().any(|fix| fix.name == name) {
                self.fixes.push(Fix {
                    name: name.into(),
                    enabled: false,
                });
            }
        }
        self.fixes.sort_by(|a, b| a.name.cmp(&b.name));
        self.fixes.dedup_by(|a, b| a.name == b.name);
    }

    pub fn selected_count(&self) -> usize {
        self.fixes.iter().filter(|fix| fix.enabled).count()
    }

    pub fn replace_catalogue(&mut self, names: Vec<String>, binary: &str) {
        self.catalogue_from_tool = true;
        self.catalogue_binary = binary.into();
        self.fixes = names
            .into_iter()
            .map(|name| {
                let enabled = self.fixes.iter().any(|fix| fix.name == name && fix.enabled);
                Fix { name, enabled }
            })
            .collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_settings_receive_safe_defaults() {
        let mut config: Config = serde_json::from_str(r#"{"project_path":"old project","fixes":[{"name":"modernize-use-nullptr","enabled":true}]}"#).unwrap();
        config.merge_catalogue();
        let count = config.fixes.len();
        config.merge_catalogue();
        assert_eq!(config.fixes.len(), count);
        assert!(count > 200);
        assert_eq!(config.selected_count(), 1);
        assert!(!config.apply_fixes);
        assert_eq!(config.jobs, 1);
    }

    #[test]
    fn settings_round_trip_without_the_checkout() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/settings.json");
        let mut config = Config::default();
        config.merge_catalogue();
        config.fixes[0].enabled = true;
        config.save(&path).unwrap();
        config.project_path = "updated project".into();
        config.save(&path).unwrap();
        let loaded = Config::load(&path).unwrap();
        assert_eq!(loaded.project_path, "updated project");
        assert_eq!(loaded.selected_count(), 1);
        assert_eq!(loaded.fixes.len(), config.fixes.len());
    }

    #[test]
    fn discovered_catalogue_keeps_matching_selections_without_readding_stale_checks() {
        let mut config = Config::default();
        config.merge_catalogue();
        config
            .fixes
            .iter_mut()
            .find(|fix| fix.name == "modernize-use-nullptr")
            .unwrap()
            .enabled = true;
        config.replace_catalogue(
            vec!["modernize-use-nullptr".into(), "new-toolchain-check".into()],
            "clang-tidy",
        );
        config.merge_catalogue();
        assert_eq!(config.fixes.len(), 2);
        assert_eq!(config.selected_count(), 1);
    }
}
