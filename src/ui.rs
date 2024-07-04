use eframe::egui;
use rfd::FileDialog;
use crate::config::{Config, Fix};
use crate::utils;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

type StartButtonCallback = Box<dyn Fn() + Send + Sync>;

pub struct Application {
    pub config: Config,
    pub select_all_checked: bool,

    // handling of processing
    pub is_running: Arc<Mutex<bool>>,
}

impl Application {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            select_all_checked: false,

            is_running: Arc::new(Mutex::new(false)),
        }
    }

    fn start_processing(&self) {
        let is_running = Arc::clone(&self.is_running);
        let config = self.config.clone(); // clone the config
        *is_running.lock().unwrap() = true;
        thread::spawn(move || {
            utils::process_fixes(config);
            thread::sleep(Duration::from_secs(5));
            *is_running.lock().unwrap() = false;
        });
    }
}

impl eframe::App for Application {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.heading("fixcpp");

                egui::Grid::new("grid")
                    .num_columns(3)
                    .spacing([20.0, 10.0])
                    .show(ui, |ui| {

                        // run-clang-tidy path
                        ui.label("Path to run-clang-tidy:");
                        ui.horizontal(|ui| {
                            let available_width = ui.available_width();
                            ui.add_sized([available_width * 0.8, 20.0], egui::TextEdit::singleline(&mut self.config.run_clang_tidy_path));
                            if ui.button("Select...").clicked() {
                                if let Some(path) = FileDialog::new().pick_file() {
                                    self.config.set_run_clang_tidy_path(path.display().to_string());
                                }
                            }
                        });
                        ui.end_row();

                        // build-commands.json path
                        ui.label("build-commands.json path:");
                        ui.horizontal(|ui| {
                            let available_width = ui.available_width();
                            ui.add_sized([available_width * 0.8, 20.0], egui::TextEdit::singleline(&mut self.config.build_commands_path));
                            if ui.button("Select...").clicked() {
                                if let Some(path) = FileDialog::new()
                                    .add_filter("build-commands.json", &["json"])
                                    .set_file_name("build-commands.json")
                                    .pick_file()
                                {
                                    self.config.set_build_commands_path(path.display().to_string());
                                }
                            }
                        });
                        ui.end_row();

                        // project path
                        ui.label("Path or file to apply fixes to:");
                        ui.horizontal(|ui| {
                            let available_width = ui.available_width();
                            ui.add_sized([available_width * 0.8, 20.0], egui::TextEdit::singleline(&mut self.config.project_path));
                            if ui.button("Select...").clicked() {
                                if let Some(path) = FileDialog::new().pick_folder() {
                                    self.config.set_project_path(path.display().to_string());
                                }
                            }
                        });
                        ui.end_row();

                        // start button
                        ui.label(""); // empty label for alignment
                        ui.horizontal(|ui| {
                            let is_running = self.is_running.lock().unwrap();
                            let available_width = ui.available_width();

                            if ui.add_enabled(!*is_running, egui::Button::new("Start")).clicked() {

                                println!("Start button pressed with clang path: {} and project path: {}",
                                         self.config.run_clang_tidy_path,
                                         self.config.project_path);

                                // drop lock before starting the task
                                drop(is_running);
                                self.start_processing();
                            }

                            ui.add_space(available_width * 0.2); // Adjust spacing to align the button
                        });
                        ui.end_row();
                    });

                // fixes section
                ui.separator();
                ui.heading("Fixes");

                // Select all
                ui.horizontal(|ui| {
                    let response = ui.checkbox(&mut self.select_all_checked, "Select all");
                    if response.changed() {
                        for fix in &mut self.config.fixes {
                            fix.enabled = self.select_all_checked;
                        }
                        self.config.save(); // Save the config after changing all fixes
                    }
                });

                // Collect changes to apply after the loop
                let mut changes = Vec::new();
                for fix in &mut self.config.fixes {
                    let mut enabled = fix.enabled;
                    ui.horizontal(|ui| {
                        let response = ui.checkbox(&mut enabled, &fix.name);
                        if response.changed() {
                            changes.push((fix.name.clone(), enabled));
                        }
                    });
                }

                // Apply changes after the loop
                for (fix_name, enabled) in changes {
                    self.config.set_fix_enabled(&fix_name, enabled);
                }
            })
        });
    }
}
