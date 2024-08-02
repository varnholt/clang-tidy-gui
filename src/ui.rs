use crate::config::Config;
use crate::utils;
use eframe::egui;
use eframe::egui::Vec2;
use egui::emath;
use rfd::FileDialog;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

mod ui_constants {
    pub static BUTTON_WIDTH: f32 = 100.0;
    pub static LINEEDIT_WIDTH: f32 = 300.0;
    pub static WIDGET_HEIGHT: f32 = 20.0;
}
pub struct Application {
    pub config: Config,
    pub select_all_checked: bool,
    pub filter_text: String,

    // handling of processing
    pub is_running: Arc<Mutex<bool>>,
    pub cancel_flag: Arc<AtomicBool>,
    pub progress: Arc<Mutex<f32>>,
}

impl Application {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            select_all_checked: false,
            filter_text: String::new(),

            is_running: Arc::new(Mutex::new(false)),
            cancel_flag: Arc::new(AtomicBool::new(false)),
            progress: Arc::new(Mutex::new(0.0)),
        }
    }

    fn start_processing(&self) {
        let is_running = Arc::clone(&self.is_running);
        let cancel_flag = Arc::clone(&self.cancel_flag);
        let config = self.config.clone();
        let progress = Arc::clone(&self.progress);

        *is_running.lock().unwrap() = true;
        *progress.lock().unwrap() = 0.0;

        thread::spawn(move || {
            utils::process_fixes(config, cancel_flag, progress);
            *is_running.lock().unwrap() = false;
        });
    }

    fn cancel_processing(&self) {
        self.cancel_flag.store(true, Ordering::Release);
    }

    fn add_enabled_sized(
        &self,
        ui: &mut egui::Ui,
        enabled: bool,
        width: f32,
        widget: impl egui::Widget,
    ) -> egui::Response {
        let size = egui::Vec2::new(width, WIDGET_HEIGHT); // Adjust height if needed
        let layout = egui::Layout::centered_and_justified(ui.layout().main_dir());

        ui.allocate_ui_with_layout(size, layout, |ui| {
            ui.set_enabled(enabled);
            ui.add(widget)
        })
        .inner
    }
}

use ui_constants::BUTTON_WIDTH;
use ui_constants::LINEEDIT_WIDTH;
use ui_constants::WIDGET_HEIGHT;

impl eframe::App for Application {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {

                ui.heading("Project Configuration");
                ui.add_space(10.0);

                egui::Grid::new("grid")
                    .num_columns(3)
                    .spacing([20.0, 10.0])
                    .min_col_width(ui.available_width() / 3.0)
                    .show(ui, |ui| {

                        // run-clang-tidy path
                        ui.label("Path to run-clang-tidy:");
                        ui.horizontal(|ui| {
                            ui.add_sized([LINEEDIT_WIDTH, WIDGET_HEIGHT], egui::TextEdit::singleline(
                                &mut self.config.run_clang_tidy_path,
                            ));
                            if ui.add_sized([BUTTON_WIDTH, WIDGET_HEIGHT], egui::Button::new("Select...")).clicked() {
                                if let Some(path) = FileDialog::new().pick_file() {
                                    self.config
                                        .set_run_clang_tidy_path(path.display().to_string());
                                }
                            }
                        });
                        ui.end_row();

                        // build-commands.json path
                        ui.label("build-commands.json path:");
                        ui.horizontal(|ui| {
                            ui.add_sized([LINEEDIT_WIDTH, WIDGET_HEIGHT], egui::TextEdit::singleline(
                                &mut self.config.build_commands_path,
                            ));
                            if ui.add_sized([BUTTON_WIDTH, WIDGET_HEIGHT], egui::Button::new("Select...")).clicked() {
                                if let Some(path) = FileDialog::new()
                                    .add_filter("build-commands.json", &["json"])
                                    .set_file_name("build-commands.json")
                                    .pick_file()
                                {
                                    self.config
                                        .set_build_commands_path(path.display().to_string());
                                }
                            }
                        });
                        ui.end_row();

                        // project path
                        ui.label("Path or file to apply fixes to:");
                        ui.horizontal(|ui| {
                            ui.add_sized([LINEEDIT_WIDTH, WIDGET_HEIGHT], egui::TextEdit::singleline(&mut self.config.project_path));
                            if ui.add_sized([BUTTON_WIDTH, WIDGET_HEIGHT], egui::Button::new("Select...")).clicked() {
                                if let Some(path) = FileDialog::new().pick_folder() {
                                    self.config.set_project_path(path.display().to_string());
                                }
                            }
                        });

                        ui.end_row();

                        ui.horizontal(|ui| {

                            // start button
                            {
                                let is_running = self.is_running.lock().unwrap();

                                if self
                                    .add_enabled_sized(
                                        ui,
                                        !*is_running,
                                        BUTTON_WIDTH,
                                        egui::Button::new("Start"),
                                    )
                                    .clicked()
                                {
                                    println!(
                                        "Start button pressed with clang path: {} and project path: {}",
                                        self.config.run_clang_tidy_path, self.config.project_path
                                    );

                                    // drop lock before starting the task
                                    drop(is_running);
                                    self.start_processing();
                                }
                            }

                            // cancel button
                            {
                                let is_running = self.is_running.lock().unwrap();

                                if self
                                    .add_enabled_sized(
                                        ui,
                                        *is_running,
                                        BUTTON_WIDTH,
                                        egui::Button::new("Cancel"),
                                    )
                                    .clicked()
                                {
                                    self.cancel_processing();
                                }
                            }
                        });

                        // progress bar
                        {
                            let progress = *self.progress.lock().unwrap();
                            ui.add(egui::ProgressBar::new(progress / 100.0).show_percentage());
                        }

                        ui.end_row();
                    });

                // fixes section
                ui.add_space(30.0);
                ui.separator();
                ui.heading("Clang-Tidy Configuration");
                ui.add_space(10.0);

                // add filter
                ui.horizontal(|ui| {
                    ui.label("Search:");
                    ui.add(egui::TextEdit::singleline(&mut self.filter_text));
                });

                // select all
                ui.horizontal(|ui| {
                    let response = ui.checkbox(&mut self.select_all_checked, "Select all");
                    if response.changed() {
                        for fix in &mut self.config.fixes {
                            fix.enabled = self.select_all_checked;
                        }
                        self.config.save(); // Save the config after changing all fixes
                    }
                });

                // collect changes to apply after the loop
                let mut changes = Vec::new();
                for fix in &mut self.config.fixes {
                    if fix
                        .name
                        .to_lowercase()
                        .contains(&self.filter_text.to_lowercase())
                    {
                        let mut enabled = fix.enabled;
                        ui.horizontal(|ui| {
                            let response = ui.checkbox(&mut enabled, &fix.name);
                            if response.changed() {
                                changes.push((fix.name.clone(), enabled));
                            }
                        });
                    }
                }

                // apply changes after the loop
                for (fix_name, enabled) in changes {
                    self.config.set_fix_enabled(&fix_name, enabled);
                }
            })
        });
    }
}
