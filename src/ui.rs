use crate::{
    config::Config,
    runner::{self, Event},
};
use eframe::egui::{self, Color32, RichText};
use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
        Arc,
    },
    time::{Duration, Instant},
};

const ACCENT: Color32 = Color32::from_rgb(91, 218, 190);

pub struct Application {
    config: Config,
    config_path: PathBuf,
    save_enabled: bool,
    notice: Option<String>,
    dirty_since: Option<Instant>,
    filter: String,
    family: String,
    selected_only: bool,
    receiver: Option<Receiver<Event>>,
    stop: Arc<AtomicBool>,
    stopping: bool,
    refreshing: bool,
    completed: usize,
    total: usize,
    status: String,
    output: VecDeque<String>,
    started: Option<Instant>,
    elapsed: Duration,
    dark: bool,
}

impl Application {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        config: Config,
        config_path: PathBuf,
        notice: Option<String>,
        save_enabled: bool,
    ) -> Self {
        let mut style = (*cc.egui_ctx.style()).clone();
        style.visuals = egui::Visuals::dark();
        style.visuals.panel_fill = Color32::from_rgb(18, 24, 34);
        style.visuals.window_fill = Color32::from_rgb(25, 33, 46);
        style.visuals.selection.bg_fill = Color32::from_rgb(35, 93, 88);
        style.visuals.selection.stroke.color = ACCENT;
        style.spacing.item_spacing = egui::vec2(10.0, 10.0);
        style.spacing.button_padding = egui::vec2(12.0, 7.0);
        style.spacing.interact_size.y = 30.0;
        style
            .text_styles
            .insert(egui::TextStyle::Body, egui::FontId::proportional(14.0));
        style
            .text_styles
            .insert(egui::TextStyle::Heading, egui::FontId::proportional(23.0));
        cc.egui_ctx.set_style(style);
        let dark = config.dark_theme;
        apply_theme(&cc.egui_ctx, dark);
        Self {
            config,
            config_path,
            save_enabled,
            notice,
            dirty_since: None,
            filter: String::new(),
            family: "All checks".into(),
            selected_only: false,
            receiver: None,
            stop: Arc::new(AtomicBool::new(false)),
            stopping: false,
            refreshing: false,
            completed: 0,
            total: 0,
            status: "Ready when you are".into(),
            output: VecDeque::new(),
            started: None,
            elapsed: Duration::ZERO,
            dark,
        }
    }

    fn save(&mut self) {
        if self.save_enabled {
            if let Err(error) = self.config.save(&self.config_path) {
                self.notice = Some(format!("Could not save settings: {error}"));
            }
        }
        self.dirty_since = None;
    }

    fn start(&mut self, ctx: &egui::Context) {
        if self.receiver.is_some() {
            return;
        }
        if let Err(error) = runner::validate(&self.config) {
            self.notice = Some(error);
            return;
        }
        self.notice = None;
        self.save();
        self.stop = Arc::new(AtomicBool::new(false));
        self.stopping = false;
        self.completed = 0;
        self.total = self.config.selected_count();
        self.output.clear();
        self.started = Some(Instant::now());
        let (sender, receiver) = mpsc::sync_channel(1024);
        self.receiver = Some(receiver);
        let config = self.config.clone();
        let stop = Arc::clone(&self.stop);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            runner::process_fixes(config, stop, |event| {
                let _ = sender.send(event);
                ctx.request_repaint();
            });
        });
    }

    fn poll(&mut self) {
        let mut finished = false;
        if let Some(receiver) = &self.receiver {
            // Bound work per frame as well as retained output.
            for _ in 0..400 {
                match receiver.try_recv() {
                    Ok(Event::Catalogue(result)) => {
                        match result {
                            Ok(names) => {
                                self.status =
                                    format!("Loaded {} checks from clang-tidy", names.len());
                                self.config.replace_catalogue(names);
                                self.family = "All checks".into();
                                self.dirty_since = Some(Instant::now());
                            }
                            Err(error) => {
                                self.notice = Some(error);
                                self.status = "Could not refresh checks".into();
                            }
                        }
                        finished = true;
                        break;
                    }
                    Ok(Event::Started {
                        name,
                        completed,
                        total,
                    }) => {
                        self.status = name;
                        self.completed = completed;
                        self.total = total;
                    }
                    Ok(Event::Log(line)) => {
                        self.output.push_back(line);
                        if self.output.len() > 2000 {
                            self.output.pop_front();
                        }
                    }
                    Ok(Event::Finished(result)) => {
                        self.elapsed = self.started.map_or(Duration::ZERO, |t| t.elapsed());
                        match result {
                            Ok(count) => {
                                self.completed = count;
                                self.status = format!("Completed {count} checks");
                            }
                            Err(error) => {
                                self.status = error.clone();
                                self.output.push_back(error);
                            }
                        }
                        finished = true;
                        break;
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        self.status = "Runner disconnected unexpectedly".into();
                        finished = true;
                        break;
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                }
            }
        }
        if finished {
            self.refreshing = false;
            self.receiver = None;
            self.stopping = false;
            self.started = None;
        }
    }

    fn stop(&mut self) {
        if self.refreshing {
            return;
        }
        self.stop.store(true, Ordering::Release);
        self.stopping = true;
    }

    fn setup(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        eyebrow(ui, "01 / PROJECT");
        ui.heading("Project setup");
        ui.label(
            RichText::new("Connect your build. Choose how to run.")
                .color(ui.visuals().weak_text_color()),
        );
        ui.add_space(16.0);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.add_enabled_ui(self.receiver.is_none(), |ui| {
                path_field(ui, "Working directory", &mut self.config.project_path, Picker::Folder);
                path_field(ui, "Compilation database", &mut self.config.build_commands_path, Picker::Database);
                path_field(ui, "run-clang-tidy", &mut self.config.run_clang_tidy_path, Picker::Script);
                ui.collapsing("Runner options", |ui| {
                    ui.label("clang-tidy executable");
                    ui.add(egui::TextEdit::singleline(&mut self.config.clang_tidy_path).desired_width(f32::INFINITY));
                    ui.label("Python interpreter");
                    ui.add(egui::TextEdit::singleline(&mut self.config.python_path).desired_width(f32::INFINITY));
                    ui.label("File filter (Python regular expression)");
                    ui.add(egui::TextEdit::singleline(&mut self.config.file_filter).desired_width(f32::INFINITY));
                    ui.horizontal(|ui| {
                        ui.label("Workers per check");
                        ui.add(egui::DragValue::new(&mut self.config.jobs).range(1..=64));
                    });
                    ui.small("One worker avoids concurrent edits to shared headers.");
                });
                ui.add_space(18.0);
                eyebrow(ui, "RUN MODE");
                ui.radio_value(&mut self.config.apply_fixes, false, "Analyze only");
                ui.radio_value(&mut self.config.apply_fixes, true, "Apply suggested fixes");
                ui.add_space(4.0);
                ui.label(RichText::new(if self.config.apply_fixes {
                    "Fixes edit source files. Start from a clean working tree and review the diff afterwards."
                } else {
                    "Inspect diagnostics before making changes to your source."
                }).small().color(ui.visuals().weak_text_color()));
            });
            ui.add_space(20.0);
            ui.separator();
            ui.add_space(8.0);
            ui.label(RichText::new("Your project stays yours").strong());
            ui.label(RichText::new("Checks run one at a time. Your existing .clang-tidy is preserved.").small().color(ui.visuals().weak_text_color()));
            ui.add_space(8.0);
            ui.hyperlink_to("clang-tidy documentation ↗", "https://clang.llvm.org/extra/clang-tidy/");
            ui.label(RichText::new(if self.save_enabled { "Settings saved automatically" } else { "Settings saving disabled this session" }).small().color(ui.visuals().weak_text_color())).on_hover_text(self.config_path.display().to_string());
        });
    }

    fn checks(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        eyebrow(ui, "02 / CHECKS");
        ui.horizontal(|ui| {
            ui.heading("Build your check list");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(format!("{} selected", self.config.selected_count()))
                        .color(accent(ui))
                        .strong(),
                );
            });
        });
        ui.label(
            RichText::new("A focused pass, one check at a time.")
                .color(ui.visuals().weak_text_color()),
        );
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            let search = ui.add(
                egui::TextEdit::singleline(&mut self.filter)
                    .hint_text("Search checks…  Ctrl+F")
                    .desired_width((ui.available_width() - 190.0).max(80.0)),
            );
            if ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::F)) {
                search.request_focus();
            }
            egui::ComboBox::from_id_salt("family")
                .selected_text(&self.family)
                .width(150.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.family, "All checks".into(), "All checks");
                    let families: std::collections::BTreeSet<_> = self
                        .config
                        .fixes
                        .iter()
                        .filter_map(|fix| fix.name.split_once('-').map(|(family, _)| family))
                        .collect();
                    for family in families {
                        ui.selectable_value(&mut self.family, family.into(), family);
                    }
                });
        });
        let query = self.filter.trim().to_lowercase();
        let visible: Vec<usize> = self
            .config
            .fixes
            .iter()
            .enumerate()
            .filter(|(_, fix)| {
                fix.name.to_lowercase().contains(&query)
                    && (self.family == "All checks"
                        || fix.name.starts_with(&format!("{}-", self.family)))
                    && (!self.selected_only || fix.enabled)
            })
            .map(|(index, _)| index)
            .collect();
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.selected_only, "Selected only");
            ui.label(
                RichText::new(format!("{} shown", visible.len()))
                    .small()
                    .color(ui.visuals().weak_text_color()),
            );
            ui.add_enabled_ui(self.receiver.is_none(), |ui| {
                if ui.small_button("Select shown").clicked() {
                    for &i in &visible {
                        self.config.fixes[i].enabled = true;
                    }
                }
                if ui.small_button("Clear shown").clicked() {
                    for &i in &visible {
                        self.config.fixes[i].enabled = false;
                    }
                }
            });
        });
        if ui.add_enabled(self.receiver.is_none(), egui::Button::new("Refresh from installed clang-tidy")).on_hover_text("Replace the bundled catalogue with checks supported by your clang-tidy. Matching selections are kept.").clicked() {
            let (sender, receiver) = mpsc::sync_channel(1);
            self.receiver = Some(receiver);
            self.refreshing = true;
            self.notice = None;
            self.status = "Reading installed checks…".into();
            let binary = self.config.clang_tidy_path.clone();
            let ctx = ui.ctx().clone();
            std::thread::spawn(move || {
                let _ = sender.send(Event::Catalogue(runner::discover_checks(&binary)));
                ctx.request_repaint();
            });
        }
        ui.separator();
        egui::ScrollArea::vertical().id_salt("checks").auto_shrink([false, false]).show(ui, |ui| {
            if visible.is_empty() {
                ui.add_space(28.0);
                ui.label(RichText::new("No checks match this view.").size(18.0));
                ui.label("Try another search, family, or turn off Selected only.");
            }
            ui.add_enabled_ui(self.receiver.is_none(), |ui| {
                ui.spacing_mut().item_spacing.y = 3.0;
                for index in visible {
                    let fix = &mut self.config.fixes[index];
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut fix.enabled, RichText::new(&fix.name).monospace());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let name = fix.name.replacen('-', "/", 1);
                            ui.hyperlink_to("↗", format!("https://clang.llvm.org/extra/clang-tidy/checks/{name}.html")).on_hover_text("Open check documentation");
                        });
                    });
                    ui.separator();
                }
            });
        });
    }
}

impl eframe::App for Application {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll();
        let before = self.config.clone();
        let running = self.receiver.is_some();
        if running {
            ctx.request_repaint_after(Duration::from_millis(100));
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.stop();
            }
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Enter)) {
            self.start(ctx);
        }

        egui::TopBottomPanel::top("header")
            .frame(egui::Frame::new().inner_margin(20).fill(if self.dark {
                Color32::from_rgb(13, 19, 28)
            } else {
                Color32::from_gray(240)
            }))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(30.0, 30.0), egui::Sense::hover());
                    let center = rect.center();
                    let points = vec![
                        center + egui::vec2(0.0, -12.0),
                        center + egui::vec2(12.0, 0.0),
                        center + egui::vec2(0.0, 12.0),
                        center + egui::vec2(-12.0, 0.0),
                    ];
                    ui.painter().add(egui::Shape::closed_line(
                        points,
                        egui::Stroke::new(2.0, accent(ui)),
                    ));
                    ui.painter().circle_filled(center, 3.0, accent(ui));
                    ui.vertical(|ui| {
                        ui.label(RichText::new("clang-tidy-gui").size(22.0).strong());
                        ui.label(
                            RichText::new("A little clarity for your C++")
                                .small()
                                .color(ui.visuals().weak_text_color()),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .button(if self.dark { "Light" } else { "Dark" })
                            .clicked()
                        {
                            self.dark = !self.dark;
                            self.config.dark_theme = self.dark;
                            apply_theme(ctx, self.dark);
                        }
                        ui.label(
                            RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                                .small()
                                .color(ui.visuals().weak_text_color()),
                        );
                    });
                });
            });

        egui::TopBottomPanel::bottom("output").resizable(true).default_height(225.0).min_height(145.0)
            .frame(egui::Frame::new().inner_margin(16).fill(ctx.style().visuals.panel_fill)).show(ctx, |ui| {
            ui.horizontal(|ui| {
                eyebrow(ui, "03 / RUN");
                let elapsed = self.started.map_or(self.elapsed, |t| t.elapsed());
                ui.label(RichText::new(format!("{} / {} checks · {:.1}s", self.completed, self.total, elapsed.as_secs_f32())).small().color(ui.visuals().weak_text_color()));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if self.refreshing {
                        ui.spinner();
                    } else if running {
                        if ui.add_enabled(!self.stopping, egui::Button::new(if self.stopping { "Stopping after this check…" } else { "Stop after this check" })).clicked() { self.stop(); }
                    } else {
                        let label = if self.config.apply_fixes { "Run & apply fixes" } else { "Run analysis" };
                        if ui.add_enabled(self.config.selected_count() > 0, egui::Button::new(RichText::new(label).color(Color32::from_rgb(13, 32, 30)).strong()).fill(ACCENT)).on_hover_text("Ctrl+Enter").clicked() { self.start(ctx); }
                    }
                    if ui.small_button("Copy output").clicked() { ctx.copy_text(self.output.iter().cloned().collect::<Vec<_>>().join("\n")); }
                });
            });
            ui.label(&self.status);
            if self.total > 0 {
                ui.add(egui::ProgressBar::new(self.completed as f32 / self.total as f32).desired_height(4.0).fill(ACCENT));
            }
            if let Some(notice) = &self.notice {
                ui.colored_label(if self.dark { Color32::from_rgb(255, 190, 120) } else { Color32::from_rgb(150, 60, 0) }, notice);
            }
            ui.separator();
            egui::ScrollArea::both().id_salt("output").stick_to_bottom(true).auto_shrink([false, false]).show(ui, |ui| {
                if self.output.is_empty() {
                    ui.label(RichText::new("Runner output will appear here.").monospace().color(ui.visuals().weak_text_color()));
                    ui.label(RichText::new("Choose a project and a few checks, then run your first analysis.").small().color(ui.visuals().weak_text_color()));
                }
                for line in &self.output { ui.label(RichText::new(line).monospace().size(12.0)); }
            });
        });
        egui::SidePanel::left("setup")
            .resizable(true)
            .default_width(330.0)
            .min_width(280.0)
            .max_width(440.0)
            .frame(
                egui::Frame::new()
                    .inner_margin(20)
                    .fill(ctx.style().visuals.panel_fill),
            )
            .show(ctx, |ui| self.setup(ui));
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .inner_margin(22)
                    .fill(ctx.style().visuals.panel_fill),
            )
            .show(ctx, |ui| self.checks(ui));

        if self.config != before {
            self.dirty_since = Some(Instant::now());
        }
        if let Some(since) = self.dirty_since {
            if since.elapsed() > Duration::from_millis(600) {
                self.save();
            } else {
                ctx.request_repaint_after(Duration::from_millis(650));
            }
        }
        if running && ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.notice = Some(
                "A check is active. Stop after this check, then close the window once it finishes."
                    .into(),
            );
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save();
    }
}

fn eyebrow(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).small().strong().color(accent(ui)));
}

enum Picker {
    Folder,
    Database,
    Script,
}

fn path_field(ui: &mut egui::Ui, label: &str, value: &mut String, picker: Picker) {
    ui.label(RichText::new(label).strong());
    ui.horizontal(|ui| {
        let width = (ui.available_width() - 48.0).max(60.0);
        ui.add_sized(
            [width, 30.0],
            egui::TextEdit::singleline(value)
                .desired_width(width)
                .hint_text(match picker {
                    Picker::Folder => "Project directory",
                    Picker::Database => "compile_commands.json",
                    Picker::Script => "Script path or name on PATH",
                }),
        )
        .on_hover_text(value.as_str());
        if ui.button("…").on_hover_text("Browse").clicked() {
            let dialog = rfd::FileDialog::new();
            let path = match picker {
                Picker::Folder => dialog.pick_folder(),
                Picker::Database => dialog
                    .add_filter("Compilation database", &["json"])
                    .set_file_name("compile_commands.json")
                    .pick_file(),
                Picker::Script => dialog.pick_file(),
            };
            if let Some(path) = path {
                *value = path.display().to_string();
            }
        }
    });
    ui.add_space(4.0);
}

fn accent(ui: &egui::Ui) -> Color32 {
    if ui.visuals().dark_mode {
        ACCENT
    } else {
        Color32::from_rgb(0, 107, 87)
    }
}

fn apply_theme(ctx: &egui::Context, dark: bool) {
    let mut visuals = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    if dark {
        visuals.panel_fill = Color32::from_rgb(18, 24, 34);
        visuals.window_fill = Color32::from_rgb(25, 33, 46);
        visuals.extreme_bg_color = Color32::from_rgb(12, 18, 27);
        visuals.widgets.noninteractive.fg_stroke.color = Color32::from_rgb(228, 235, 244);
        visuals.widgets.inactive.bg_fill = Color32::from_rgb(35, 45, 60);
        visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(35, 45, 60);
        visuals.widgets.inactive.fg_stroke.color = Color32::from_rgb(208, 219, 233);
        visuals.selection.bg_fill = Color32::from_rgb(35, 93, 88);
        visuals.selection.stroke.color = ACCENT;
        visuals.hyperlink_color = ACCENT;
    } else {
        visuals.panel_fill = Color32::from_rgb(248, 250, 253);
        visuals.weak_text_color = Some(Color32::from_rgb(95, 108, 124));
        visuals.selection.bg_fill = Color32::from_rgb(185, 229, 216);
        visuals.selection.stroke.color = Color32::from_rgb(0, 88, 70);
        visuals.hyperlink_color = Color32::from_rgb(0, 107, 87);
    }
    ctx.set_visuals(visuals);
}
