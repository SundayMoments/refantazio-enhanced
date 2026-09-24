#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod bundle;
mod licenses;
mod platform;
mod settings;
mod storage;
#[cfg(test)]
mod tests;

use anyhow::Result;
use eframe::egui::{self, Color32, RichText};
use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver},
};

struct Helper {
    folder: String,
    tab: usize,
    status: String,
    error: bool,
    replace: bool,
    remove_confirm: bool,
    accepted: bool,
    document: Option<settings::Document>,
    document_folder: String,
    work: Option<Receiver<Result<String, String>>>,
    preview: Option<PathBuf>,
    preview_frames: u8,
    license_page: usize,
    selected_license: Option<(&'static str, &'static str)>,
    preview_license: bool,
}
impl Helper {
    fn new(cc: &eframe::CreationContext<'_>, preview: Option<PathBuf>) -> Self {
        cc.egui_ctx.set_theme(egui::Theme::Dark);
        let mut style = (*cc.egui_ctx.style_of(egui::Theme::Dark)).clone();
        style.visuals = egui::Visuals::dark();
        style.visuals.panel_fill = Color32::from_rgb(20, 24, 29);
        style.visuals.override_text_color = Some(Color32::from_rgb(220, 224, 231));
        style.visuals.selection.bg_fill = Color32::from_rgb(159, 34, 58);
        style.spacing.item_spacing = egui::vec2(16., 14.);
        style.spacing.button_padding = egui::vec2(16., 8.);
        style.spacing.interact_size.y = 24.;
        style.spacing.slider_width = 150.;
        style.spacing.scroll = egui::style::ScrollStyle::solid();
        style.visuals.hyperlink_color = Color32::from_rgb(116, 202, 186);
        if preview.is_some() {
            style.animation_time = 0.;
            style.scroll_animation = egui::style::ScrollAnimation::none();
        }
        style
            .text_styles
            .insert(egui::TextStyle::Body, egui::FontId::proportional(16.));
        style
            .text_styles
            .insert(egui::TextStyle::Button, egui::FontId::proportional(16.));
        style
            .text_styles
            .insert(egui::TextStyle::Heading, egui::FontId::proportional(24.));
        cc.egui_ctx.set_style_of(egui::Theme::Dark, style);
        let folder = if preview.is_some() {
            "D:\\SteamLibrary\\steamapps\\common\\METAPHOR".into()
        } else {
            platform::discover()
                .first()
                .map(|p| {
                    p.display()
                        .to_string()
                        .trim_start_matches("\\\\?\\")
                        .to_owned()
                })
                .unwrap_or_default()
        };
        Self {
            folder,
            tab: 0,
            status: "Close the game before making changes.".into(),
            error: false,
            replace: false,
            remove_confirm: false,
            accepted: false,
            document: None,
            document_folder: String::new(),
            work: None,
            preview,
            preview_frames: 0,
            license_page: 0,
            selected_license: None,
            preview_license: false,
        }
    }
    fn result(&mut self, result: Result<String>) {
        match result {
            Ok(s) => {
                self.status = s;
                self.error = false;
            }
            Err(e) => {
                self.status = format!("{e:#}");
                self.error = true;
            }
        }
    }
    fn start(
        &mut self,
        ctx: &egui::Context,
        job: impl FnOnce() -> Result<String> + Send + 'static,
    ) {
        if self.preview.is_some() {
            self.status = "Preview mode: file changes are disabled.".into();
            return;
        }
        let (tx, rx) = mpsc::channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(job))
                .map_err(|_| {
                    "Operation stopped unexpectedly. Use recovery before trying again.".to_owned()
                })
                .and_then(|r| r.map_err(|e| format!("{e:#}")));
            let _ = tx.send(r);
            ctx.request_repaint();
        });
        self.work = Some(rx);
        self.status = "Working locally… verifying files and backups.".into();
        self.error = false;
    }
    fn load_settings(&mut self) {
        let result = if self.preview.is_some() {
            settings::Document::parse(None)
        } else {
            platform::game_root(&PathBuf::from(&self.folder))
                .and_then(|p| settings::Document::load(&p))
        };
        match result {
            Ok(d) => {
                self.document = Some(d);
                self.document_folder = self.folder.clone();
                self.status = "Settings loaded. Save changes with the game closed.".into();
                self.error = false;
            }
            Err(e) => {
                self.document = None;
                self.result(Err(e));
            }
        }
    }
    fn install_ui(&mut self, ui: &mut egui::Ui) {
        ui.label(
            RichText::new("Steam build 18330018 · NVIDIA RTX for DLSS")
                .size(14.)
                .weak(),
        );
        ui.add_space(10.);
        ui.checkbox(&mut self.replace,"Back up and replace existing mod files").on_hover_text("Use when switching from an existing Luma/ReShade installation. Removing this bundle restores those previous files.");
        ui.checkbox(&mut self.accepted, "Accept bundled runtime licenses")
            .on_hover_text("Read the NVIDIA and Microsoft terms under Credits & licenses.");
        if ui
            .add_enabled(
                self.accepted && bundle::AVAILABLE,
                egui::Button::new("Install mod").fill(Color32::from_rgb(159, 34, 58)),
            )
            .clicked()
        {
            let folder = PathBuf::from(&self.folder);
            let replace = self.replace;
            self.start(ui.ctx(),move||{platform::closed_game()?;let package=bundle::load()?;let root=platform::game_root(&folder)?;let store=storage::Store::open(&root)?;let count=package.install(&store,replace)?;Ok(format!("Installed and verified {count} files. Start the game normally through Steam. Press Home for the overlay."))});
        }
        if !bundle::AVAILABLE {
            ui.label("Development build — mod files are not embedded.");
        }
        ui.add_space(16.);
        ui.separator();
        ui.add_space(4.);
        ui.label(RichText::new("Removal").strong().size(19.));
        ui.checkbox(&mut self.remove_confirm, "Restore previous files")
            .on_hover_text(
                "Removing this mod restores the files backed up before its first installation.",
            );
        ui.horizontal(|ui| {
            if ui.add_enabled(self.remove_confirm,egui::Button::new("Remove mod")).clicked(){let folder=PathBuf::from(&self.folder);self.start(ui.ctx(),move||{platform::closed_game()?;let root=platform::game_root(&folder)?;let store=storage::Store::open(&root)?;let n=storage::uninstall(&store)?;Ok(format!("Removed {n} managed files or restored their originals. Settings, saves, and backups remain."))});}
        });
        ui.add_space(8.);
        ui.collapsing("Recovery", |ui| {
            if ui.button("Recover interrupted operation").clicked() {
                let folder = PathBuf::from(&self.folder);
                self.start(ui.ctx(), move || {
                    platform::closed_game()?;
                    let root = platform::game_root(&folder)?;
                    let store = storage::Store::open(&root)?;
                    anyhow::ensure!(store.pending()?, "No interrupted operation was found");
                    store.recover()?;
                    Ok("Previous files restored. You can try the operation again.".into())
                });
            }
            ui.label("Keep .refantazio-enhanced in the game folder for removal and recovery.");
        });
    }
    fn settings_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            if ui.button("Reload settings").clicked() {
                self.load_settings();
            }
            if ui
                .add_enabled(
                    self.document.is_some(),
                    egui::Button::new("Recommended settings"),
                )
                .clicked()
            {
                self.document.as_mut().unwrap().defaults();
            }
            if ui
                .add_enabled(
                    self.document.is_some(),
                    egui::Button::new("Save settings").fill(Color32::from_rgb(159, 34, 58)),
                )
                .clicked()
            {
                let doc = self.document.as_ref().unwrap().clone();
                let folder = PathBuf::from(&self.folder);
                self.start(ui.ctx(), move || {
                    platform::closed_game()?;
                    let root = platform::game_root(&folder)?;
                    let store = storage::Store::open(&root)?;
                    doc.save(&store)?;
                    Ok("Settings saved. They take effect on the next game launch.".into())
                });
            }
        });
        ui.label(
            RichText::new("Close the game before saving. Hover a setting for details.")
                .size(14.)
                .weak(),
        );
        let Some(doc) = &mut self.document else {
            ui.label("Load your game's settings to edit them.");
            return;
        };
        for group in ["Overlay", "Image quality", "Game"] {
            ui.add_space(12.);
            ui.label(RichText::new(group).strong().size(19.));
            egui::Grid::new(group)
                .num_columns(2)
                .min_col_width(264.)
                .max_col_width(264.)
                .min_row_height(38.)
                .spacing([24., 14.])
                .show(ui, |ui| {
                    for setting in settings::SETTINGS.iter().filter(|s| s.group == group) {
                        ui.label(setting.label).on_hover_text(setting.help);
                        let current = doc.get(setting).to_owned();
                        match setting.kind {
                            settings::Kind::Toggle => {
                                let mut enabled = current == "1";
                                if ui.checkbox(&mut enabled, "").changed() {
                                    doc.set(setting, if enabled { "1" } else { "0" }.into());
                                }
                            }
                            settings::Kind::Range(min, max) => {
                                let mut value = current
                                    .parse::<f32>()
                                    .unwrap_or_else(|_| setting.default.parse().unwrap());
                                if ui
                                    .add(egui::Slider::new(&mut value, min..=max).max_decimals(2))
                                    .changed()
                                {
                                    doc.set(setting, format!("{value:.3}"));
                                }
                            }
                            settings::Kind::Choice(options) => {
                                let mut value = current.clone();
                                egui::ComboBox::from_id_salt(setting.key)
                                    .selected_text(
                                        options
                                            .iter()
                                            .find(|(v, _)| *v == current)
                                            .map(|(_, s)| *s)
                                            .unwrap_or("Custom (choose a value)"),
                                    )
                                    .width(250.)
                                    .show_ui(ui, |ui| {
                                        for (v, label) in options {
                                            ui.selectable_value(&mut value, (*v).into(), *label);
                                        }
                                    });
                                if value != current {
                                    doc.set(setting, value);
                                }
                            }
                        }
                        ui.end_row();
                    }
                });
        }
    }
    fn about_ui(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Credits").strong().size(19.));
        ui.label("Idarion · Metaphor integration\nFilippo Tarpini / Pumbo & contributors · Luma Framework\nLyall · MetaphorFix\nPatrick Mours & contributors · ReShade\nNVIDIA RTX™ · DLSS / NGX");
        ui.label("Independent experimental fork. No affiliation or endorsement is claimed.");
        ui.separator();
        ui.label("Offline helper. No update checks, downloads, or telemetry.");
        ui.collapsing("NVIDIA DLSS runtime terms", |ui| {
            ui.add(egui::Label::new(include_str!("../../licenses/NVIDIA-DLSS.txt")).wrap());
        });
        ui.collapsing("Microsoft Visual C++ runtime terms", |ui| {
            ui.add(
                egui::Label::new(include_str!("../../licenses/Microsoft-VC-Runtime.txt")).wrap(),
            );
        });
        let project = egui::CollapsingHeader::new("Project license")
            .default_open(self.preview_license)
            .show(ui, |ui| {
                ui.add_space(4.);
                licenses::project(ui, &mut self.selected_license);
            });
        if self.preview_license && self.preview_frames < 2 {
            project.header_response.scroll_to_me(Some(egui::Align::TOP));
        }
        ui.collapsing("All dependency notices", |ui| {
            let pages = bundle::NOTICES.lines().count().div_ceil(80).max(1);
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(self.license_page > 0, egui::Button::new("Previous page"))
                    .clicked()
                {
                    self.license_page -= 1;
                }
                ui.label(format!("Page {} / {pages}", self.license_page + 1));
                if ui
                    .add_enabled(
                        self.license_page + 1 < pages,
                        egui::Button::new("Next page"),
                    )
                    .clicked()
                {
                    self.license_page += 1;
                }
            });
            let page = bundle::NOTICES
                .lines()
                .skip(self.license_page * 80)
                .take(80)
                .collect::<Vec<_>>()
                .join("\n");
            ui.add(egui::Label::new(page).wrap());
        });
    }
}
impl eframe::App for Helper {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if let Some(rx) = &self.work {
            match rx.try_recv() {
                Ok(result) => {
                    self.work = None;
                    self.result(result.map_err(anyhow::Error::msg));
                    if !self.error && self.document.is_some() {
                        let message = self.status.clone();
                        self.load_settings();
                        self.status = message;
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.work = None;
                    self.error = true;
                    self.status =
                        "Worker stopped. Use recovery before making further changes.".into();
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if self.work.is_some() && ui.ctx().input(|i| i.viewport().close_requested()) {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::central_panel(ui.style()).inner_margin(28))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("ReFantazio Enhanced");
                    ui.label(
                        RichText::new(concat!(env!("CARGO_PKG_VERSION"), " · Experimental"))
                            .small()
                            .color(Color32::from_rgb(116, 202, 186)),
                    );
                });
                ui.add_space(10.);
                ui.add_enabled_ui(self.work.is_none(), |ui| {
                    ui.label("Game folder");
                    ui.horizontal(|ui| {
                        ui.add_sized(
                            [ui.available_width() - 116., 36.],
                            egui::TextEdit::singleline(&mut self.folder)
                                .hint_text("Folder containing METAPHOR.exe"),
                        );
                        if ui.button("Browse…").clicked()
                            && let Some(folder) = rfd::FileDialog::new()
                                .set_title("Choose the folder containing METAPHOR.exe")
                                .pick_folder()
                        {
                            self.folder = folder.display().to_string();
                        }
                    });
                    if self.folder != self.document_folder {
                        self.document = None;
                    }
                    ui.add_space(6.);
                    ui.horizontal(|ui| {
                        for (i, label) in ["Installation", "Settings", "Credits & licenses"]
                            .iter()
                            .enumerate()
                        {
                            let selected = self.tab == i;
                            let text = RichText::new(*label);
                            let response = ui.add(
                                egui::Button::new(if selected {
                                    text.strong()
                                } else {
                                    text.weak()
                                })
                                .selected(selected)
                                .frame(false),
                            );
                            if selected {
                                ui.painter().hline(
                                    response.rect.x_range(),
                                    response.rect.bottom() + 3.,
                                    egui::Stroke::new(2., Color32::from_rgb(204, 61, 85)),
                                );
                            }
                            if response.clicked() {
                                self.tab = i;
                                if i == 1 && self.document.is_none() {
                                    self.load_settings();
                                }
                            }
                        }
                    });
                });
                ui.separator();
                egui::Panel::bottom("status")
                    .frame(egui::Frame::new().inner_margin(8))
                    .show(ui, |ui| {
                        if self.work.is_some() {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label("Working locally…");
                            });
                        }
                        ui.label(RichText::new(&self.status).color(if self.error {
                            Color32::from_rgb(255, 152, 151)
                        } else {
                            Color32::from_rgb(164, 210, 190)
                        }));
                        if self.error
                            && self.work.is_none()
                            && (self
                                .status
                                .to_ascii_lowercase()
                                .contains("access is denied")
                                || self
                                    .status
                                    .to_ascii_lowercase()
                                    .contains("permission denied"))
                            && ui.button("Relaunch as administrator").clicked()
                        {
                            match platform::elevate() {
                                Ok(()) => ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close),
                                Err(e) => self.result(Err(e)),
                            }
                        }
                    });
                egui::ScrollArea::vertical()
                    .id_salt(("page", self.tab))
                    .auto_shrink([false, false])
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                    .show(ui, |ui| {
                        ui.add_space(8.);
                        ui.add_enabled_ui(self.work.is_none(), |ui| match self.tab {
                            0 => self.install_ui(ui),
                            1 => self.settings_ui(ui),
                            _ => self.about_ui(ui),
                        });
                    });
            });
        licenses::dialog(ui.ctx(), &mut self.selected_license);
        if let Some(path) = &self.preview {
            self.preview_frames = self.preview_frames.saturating_add(1);
            if self.preview_frames == 5 {
                ui.ctx()
                    .send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            }
            for event in ui.ctx().input(|i| i.events.clone()) {
                if let egui::Event::Screenshot { image, .. } = event {
                    if let Ok(file) = std::fs::File::create(path) {
                        let mut encoder =
                            png::Encoder::new(file, image.width() as u32, image.height() as u32);
                        encoder.set_color(png::ColorType::Rgba);
                        encoder.set_depth(png::BitDepth::Eight);
                        if let Ok(mut writer) = encoder.write_header() {
                            let bytes: Vec<u8> =
                                image.pixels.iter().flat_map(|p| p.to_array()).collect();
                            let _ = writer.write_image_data(&bytes);
                        }
                    }
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
            ui.ctx().request_repaint();
        }
    }
}
fn main() -> eframe::Result {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.first().is_some_and(|a| a == "--verify-payload") {
        match bundle::load() {
            Ok(p) => {
                println!("Verified {} bundled files", p.files.len());
                std::process::exit(0);
            }
            Err(e) => {
                eprintln!("{e:#}");
                std::process::exit(1);
            }
        }
    }
    let preview = if args.first().is_some_and(|a| a == "--preview") {
        args.get(1).map(PathBuf::from)
    } else {
        None
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(if preview.is_some() && args.iter().any(|a| a == "narrow") {
                [780., 620.]
            } else {
                [960., 720.]
            })
            .with_min_inner_size([780., 620.]),
        ..Default::default()
    };
    let preview_page = args
        .get(2)
        .and_then(|a| a.to_str())
        .unwrap_or("")
        .to_owned();
    let preview_tab = match preview_page.as_str() {
        "settings" => 1,
        "credits" | "license" | "license-dialog" => 2,
        _ => 0,
    };
    eframe::run_native(
        "ReFantazio Enhanced",
        options,
        Box::new(move |cc| {
            let mut app = Helper::new(cc, preview);
            if app.preview.is_some() {
                app.tab = preview_tab;
                app.preview_license = preview_page.starts_with("license");
                if preview_tab == 1 {
                    app.load_settings();
                }
                if preview_page == "license-dialog" {
                    app.selected_license = Some((
                        "Luma Custom MIT",
                        licenses::embedded("licenses/Luma-Custom-MIT.txt").unwrap(),
                    ));
                }
            }
            Ok(Box::new(app))
        }),
    )
}
