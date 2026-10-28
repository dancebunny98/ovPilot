//! Native desktop window and asynchronous aircraft inventory workflow.

use eframe::egui::{self, Color32, FontFamily, FontId, RichText, Stroke};
use ovpilot::{
    discover_model_roots, AircraftTypeCatalog, AtcCategory, AtcDirectory, AudioDevices, AuthClient,
    ClientSettings, MicrophoneLoopback, MicrophoneMonitor, ModelCatalog, ModelRuleSet,
    ModelScanResult, ModelScanner, PasswordStore, SettingsStore, UpdateChecker, UpdateStatus,
    GITHUB_REPOSITORY, VERSION,
};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};

const SETTINGS: [&str; 12] = [
    "Network",
    "Notifications",
    "Fonts",
    "Audio",
    "Push-To-Talk (PTT)",
    "Model Matching (MSFS 2024)",
    "  Advanced",
    "  Custom Rules",
    "Performance",
    "Updates",
    "Miscellaneous",
    "About",
];

const CYAN: Color32 = Color32::from_rgb(0, 255, 255);
const DARK: Color32 = Color32::from_rgb(30, 30, 30);
const BORDER: Color32 = Color32::from_rgb(100, 100, 100);

/// Launches the standalone ovPilot window.
pub fn run() -> ovpilot::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1042.0, 562.0])
            .with_min_inner_size([812.0, 438.0])
            .with_decorations(false),
        ..Default::default()
    };
    eframe::run_native(
        "ovPilot",
        options,
        Box::new(|cc| Ok(Box::new(PilotWindow::new(cc)))),
    )
    .map_err(|error| format!("desktop window failed: {error}"))?;
    Ok(())
}

fn display_font(size_pt: u8, monospace: bool) -> FontId {
    FontId::new(
        f32::from(size_pt) * 4.0 / 3.0,
        if monospace {
            FontFamily::Monospace
        } else {
            FontFamily::Proportional
        },
    )
}

#[cfg(target_os = "windows")]
fn caps_lock_down() -> bool {
    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetAsyncKeyState(key: i32) -> i16;
    }
    // VK_CAPITAL reports physical key-down in its high bit, independent of toggle state.
    unsafe { GetAsyncKeyState(0x14) as u16 & 0x8000 != 0 }
}

fn audio_level_meter(ui: &mut egui::Ui, level: f32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(310.0, 18.0), egui::Sense::hover());
    let painter = ui.painter();
    for (start, end, color) in [
        (0.0, 0.7, Color32::from_rgb(29, 151, 63)),
        (0.7, 0.9, Color32::from_rgb(223, 177, 28)),
        (0.9, 1.0, Color32::from_rgb(193, 55, 40)),
    ] {
        painter.rect_filled(
            egui::Rect::from_x_y_ranges(
                (rect.left() + rect.width() * start)..=(rect.left() + rect.width() * end),
                rect.y_range(),
            ),
            0.0,
            color,
        );
    }
    let marker = rect.left() + rect.width() * level.clamp(0.0, 1.0);
    painter.line_segment(
        [
            egui::pos2(marker, rect.top()),
            egui::pos2(marker, rect.bottom()),
        ],
        Stroke::new(3.0_f32, Color32::WHITE),
    );
}

#[cfg(not(target_os = "windows"))]
fn caps_lock_down() -> bool {
    false
}

fn install_system_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    for (file, name, family) in [
        (
            "micross.ttf",
            "Microsoft Sans Serif",
            FontFamily::Proportional,
        ),
        ("consola.ttf", "Consolas", FontFamily::Monospace),
    ] {
        if let Some(root) = std::env::var_os("WINDIR") {
            if let Ok(bytes) =
                std::fs::read(std::path::PathBuf::from(root).join("Fonts").join(file))
            {
                fonts
                    .font_data
                    .insert(name.into(), egui::FontData::from_owned(bytes).into());
                if let Some(family_fonts) = fonts.families.get_mut(&family) {
                    family_fonts.insert(0, name.into());
                }
            }
        }
    }
    ctx.set_fonts(fonts);
}

enum AuthCheck {
    Verified,
    VerifiedWithoutSavedPassword(String),
    Rejected(String),
}

struct PilotWindow {
    settings_open: bool,
    connect_open: bool,
    callsign: String,
    type_code: String,
    type_catalog: AircraftTypeCatalog,
    type_suggestions_open: bool,
    selcal: String,
    observer: bool,
    password: String,
    auth: Option<Receiver<AuthCheck>>,
    auth_message: String,
    auth_success: bool,
    auth_error: bool,
    settings_tab: usize,
    settings_data: ClientSettings,
    settings_store: Option<SettingsStore>,
    new_root: String,
    new_rule_path: String,
    selected_rule: Option<usize>,
    model_catalog: Option<ModelCatalog>,
    scan: Option<Receiver<ModelScanResult>>,
    update: Option<Receiver<String>>,
    update_message: String,
    messages: Vec<String>,
    chat_input: String,
    note_input: String,
    show_notes: bool,
    models_found: usize,
    controllers: AtcDirectory,
    own_position: Option<(f64, f64)>,
    scan_busy: bool,
    audio_inputs: Vec<String>,
    audio_outputs: Vec<String>,
    audio_monitor: Option<MicrophoneMonitor>,
    audio_loopback: Option<MicrophoneLoopback>,
    audio_loopback_config: Option<(String, String, i8, i8)>,
    monitor_selection: Option<(String, i8)>,
    audio_preview: Option<Receiver<String>>,
    audio_status: String,
    ptt_capture: bool,
    ptt_pressed: bool,
}

impl PilotWindow {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        install_system_fonts(&cc.egui_ctx);
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = DARK;
        visuals.window_fill = Color32::from_rgb(242, 242, 242);
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, BORDER);
        cc.egui_ctx.set_visuals(visuals);
        let mut style = (*cc.egui_ctx.style()).clone();
        for font in style.text_styles.values_mut() {
            font.size = 15.0;
        }
        style.spacing.item_spacing = egui::vec2(4.0, 2.0);
        cc.egui_ctx.set_style(style);
        let settings_store = SettingsStore::discover().ok();
        let settings_data = settings_store
            .as_ref()
            .and_then(|store| store.load().ok())
            .unwrap_or_else(|| ClientSettings {
                model_roots: discover_model_roots(),
                ..Default::default()
            });
        let (password, auth_message, auth_error) = if settings_data.cid.is_empty() {
            (String::new(), String::new(), false)
        } else {
            match PasswordStore.load(&settings_data.cid) {
                Ok(Some(password)) => (
                    password,
                    "Saved password loaded. Authentication not checked.".into(),
                    false,
                ),
                Ok(None) => (String::new(), String::new(), false),
                Err(error) => (
                    String::new(),
                    format!("Could not load saved password: {error}"),
                    true,
                ),
            }
        };
        let mut app = Self {
            settings_open: false,
            connect_open: false,
            callsign: settings_data.last_callsign.clone(),
            type_code: settings_data.last_type_code.clone(),
            type_catalog: AircraftTypeCatalog::bundled(),
            type_suggestions_open: false,
            selcal: settings_data.last_selcal.clone(),
            observer: settings_data.observer_mode,
            password,
            auth: None,
            auth_message,
            auth_success: false,
            auth_error,
            settings_tab: 0,
            settings_data,
            settings_store,
            new_root: String::new(),
            new_rule_path: String::new(),
            selected_rule: None,
            model_catalog: None,
            scan: None,
            update: None,
            update_message: String::new(),
            messages: vec![format!(
                "[{}] ovPilot version {VERSION}",
                chrono::Local::now().format("%H:%M:%S")
            )],
            chat_input: String::new(),
            note_input: String::new(),
            show_notes: false,
            models_found: 0,
            controllers: AtcDirectory::new(),
            own_position: None,
            scan_busy: false,
            audio_inputs: AudioDevices.inputs().unwrap_or_default(),
            audio_outputs: AudioDevices.outputs().unwrap_or_default(),
            audio_monitor: None,
            audio_loopback: None,
            audio_loopback_config: None,
            monitor_selection: None,
            audio_preview: None,
            audio_status: String::new(),
            ptt_capture: false,
            ptt_pressed: false,
        };
        if app.settings_data.keep_window_visible {
            cc.egui_ctx
                .send_viewport_cmd(egui::ViewportCommand::WindowLevel(
                    egui::WindowLevel::AlwaysOnTop,
                ));
        }
        if app.settings_data.auto_check_updates {
            app.check_updates();
        }
        app.rescan();
        app
    }

    fn rescan(&mut self) {
        if self.scan_busy {
            return;
        }
        self.log("ovPilot needs to scan your installed aircraft models.");
        self.scan_busy = true;
        let roots = self.settings_data.model_roots.clone();
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = sender.send(ModelScanner.scan(&roots));
        });
        self.scan = Some(receiver);
    }

    fn log(&mut self, message: impl AsRef<str>) {
        self.messages.push(format!(
            "[{}] {}",
            chrono::Local::now().format("%H:%M:%S"),
            message.as_ref()
        ));
    }

    fn save_settings(&mut self) {
        if let Some(store) = &self.settings_store {
            if let Err(error) = store.save(&self.settings_data) {
                self.log(format!("Could not save settings: {error}"));
            }
        } else {
            self.log("User settings directory is unavailable.");
        }
    }

    fn poll_background(&mut self) {
        if let Some(result) = self
            .scan
            .as_ref()
            .and_then(|receiver| receiver.try_recv().ok())
        {
            let catalog = ModelCatalog::new(result.models);
            self.models_found = catalog.len();
            self.model_catalog = Some(catalog);
            self.log(format!(
                "Scan complete. {} aircraft models found.",
                self.models_found
            ));
            if !result.unreadable_directories.is_empty() {
                self.log(format!(
                    "{} directories could not be read.",
                    result.unreadable_directories.len()
                ));
            }
            self.scan = None;
            self.scan_busy = false;
        }
        if let Some(message) = self
            .update
            .as_ref()
            .and_then(|receiver| receiver.try_recv().ok())
        {
            self.update_message = message;
            self.update = None;
        }
        if let Some(result) = self
            .auth
            .as_ref()
            .and_then(|receiver| receiver.try_recv().ok())
        {
            match result {
                AuthCheck::Verified => {
                    self.auth_message =
                        "Authentication successful. Password saved securely.".into();
                    self.auth_success = true;
                    self.auth_error = false;
                    self.log("VATSIM authentication successful.");
                }
                AuthCheck::VerifiedWithoutSavedPassword(error) => {
                    self.auth_message =
                        format!("Authentication successful. Password was not saved: {error}");
                    self.auth_success = true;
                    self.auth_error = false;
                    self.log("VATSIM authentication successful.");
                }
                AuthCheck::Rejected(error) => {
                    self.auth_message = format!("Authentication failed: {error}");
                    self.auth_success = false;
                    self.auth_error = true;
                }
            }
            self.auth = None;
        }
        if let Some(message) = self
            .audio_preview
            .as_ref()
            .and_then(|receiver| receiver.try_recv().ok())
        {
            self.audio_status = message;
            self.audio_preview = None;
        }
    }

    fn check_credentials(&mut self, ctx: &egui::Context) {
        if self.auth.is_some() {
            return;
        }
        let cid = self.settings_data.cid.trim().to_owned();
        let password = self.password.clone();
        self.auth_message = "Checking credentials...".into();
        self.auth_success = false;
        self.auth_error = false;
        let (sender, receiver) = mpsc::channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let result = match AuthClient::new().authenticate(&cid, &password) {
                Ok(_) => match PasswordStore.save(&cid, &password) {
                    Ok(()) => AuthCheck::Verified,
                    Err(error) => AuthCheck::VerifiedWithoutSavedPassword(error.to_string()),
                },
                Err(error) => AuthCheck::Rejected(error.to_string()),
            };
            let _ = sender.send(result);
            ctx.request_repaint();
        });
        self.auth = Some(receiver);
    }

    fn check_updates(&mut self) {
        if self.update.is_some() {
            return;
        }
        self.update_message = "Checking GitHub Releases...".into();
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let message = match UpdateChecker::new().check() {
                Ok(UpdateStatus::NoRelease) => "No stable release published yet.".into(),
                Ok(UpdateStatus::Current { latest, .. }) => {
                    format!("Up to date (latest: {latest}).")
                }
                Ok(UpdateStatus::Available { latest, url }) => {
                    format!("Version {latest} available: {url}")
                }
                Err(error) => format!("Update check failed: {error}"),
            };
            let _ = sender.send(message);
        });
        self.update = Some(receiver);
    }

    fn settings(&mut self, ctx: &egui::Context) {
        let mut open = self.settings_open;
        egui::Window::new("ovPilot Settings")
            .open(&mut open)
            .default_size([700.0, 431.0])
            .min_width(625.0)
            .collapsible(false)
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(242, 242, 242))
                    .inner_margin(egui::Margin::same(8))
                    .stroke(Stroke::new(1.0_f32, Color32::from_gray(165))),
            )
            .show(ctx, |ui| {
                ui.visuals_mut().override_text_color = Some(Color32::BLACK);
                ui.visuals_mut().extreme_bg_color = Color32::WHITE;
                ui.visuals_mut().widgets.noninteractive.bg_fill = Color32::WHITE;
                ui.visuals_mut().widgets.inactive.bg_fill = Color32::WHITE;
                ui.visuals_mut().widgets.inactive.weak_bg_fill = Color32::WHITE;
                ui.visuals_mut().widgets.inactive.bg_stroke =
                    Stroke::new(1.0_f32, Color32::from_gray(190));
                ui.visuals_mut().widgets.hovered.bg_fill = Color32::from_rgb(225, 238, 238);
                ui.visuals_mut().widgets.hovered.weak_bg_fill = Color32::from_rgb(225, 238, 238);
                ui.visuals_mut().widgets.active.bg_fill = Color32::WHITE;
                ui.visuals_mut().selection.bg_fill = Color32::from_rgb(218, 218, 218);
                ui.horizontal_top(|ui| {
                    egui::Frame::new()
                        .fill(Color32::WHITE)
                        .inner_margin(egui::Margin::same(5))
                        .show(ui, |ui| {
                            ui.set_min_width(175.0);
                            ui.set_min_height(365.0);
                            ui.vertical(|ui| {
                                for (index, label) in SETTINGS.iter().enumerate() {
                                    ui.selectable_value(&mut self.settings_tab, index, *label);
                                }
                            });
                        });
                    ui.vertical(|ui| {
                        ui.set_min_width(345.0);
                        match self.settings_tab {
                            0 => self.network_settings(ui),
                            1 => self.notification_settings(ui),
                            2 => self.font_settings(ui),
                            3 => self.audio_settings(ui),
                            4 => self.ptt_settings(ui),
                            5 => self.model_settings(ui),
                            6 => self.advanced_settings(ui),
                            7 => self.custom_rules_settings(ui),
                            8 => self.performance_settings(ui),
                            9 => {
                                if ui
                                    .checkbox(
                                        &mut self.settings_data.auto_check_updates,
                                        "Check for ovPilot updates on channel:",
                                    )
                                    .changed()
                                {
                                    self.save_settings();
                                }
                                ui.label("Stable");
                                if ui.button("Check for updates").clicked() {
                                    self.check_updates();
                                }
                                ui.label(&self.update_message);
                            }
                            10 => self.misc_settings(ctx, ui),
                            11 => {
                                ui.heading("ovPilot");
                                ui.label(format!("Version {VERSION}"));
                                ui.hyperlink_to("GitHub repository", GITHUB_REPOSITORY);
                                ui.label("Native Rust client");
                            }
                            _ => {
                                ui.label(
                                    "This setting is not available in the current native client.",
                                );
                            }
                        }
                    });
                });
                ui.add_space((ui.available_height() - 27.0).max(0.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Cancel").clicked() {
                        self.settings_open = false;
                    }
                    if ui.button("OK").clicked() {
                        self.settings_open = false;
                    }
                });
            });
        self.settings_open &= open;
    }

    fn network_settings(&mut self, ui: &mut egui::Ui) {
        let mut changed = false;
        ui.label("VATSIM CID:");
        let cid_response = ui.add_sized(
            [200.0, 22.0],
            egui::TextEdit::singleline(&mut self.settings_data.cid),
        );
        if cid_response.changed() {
            changed = true;
            self.password.clear();
            self.auth_message.clear();
            self.auth_success = false;
            self.auth_error = false;
        }
        if cid_response.lost_focus() && !self.settings_data.cid.is_empty() {
            match PasswordStore.load(&self.settings_data.cid) {
                Ok(Some(password)) => {
                    self.password = password;
                    self.auth_message = "Saved password loaded. Authentication not checked.".into();
                }
                Ok(None) => {}
                Err(error) => {
                    self.auth_message = format!("Could not load saved password: {error}");
                    self.auth_error = true;
                }
            }
        }
        ui.label("VATSIM Password:");
        if ui
            .add_sized(
                [200.0, 22.0],
                egui::TextEdit::singleline(&mut self.password).password(true),
            )
            .changed()
        {
            self.auth_message.clear();
            self.auth_success = false;
            self.auth_error = false;
        }
        ui.label("Your Full Name:");
        changed |= ui
            .add_sized(
                [200.0, 22.0],
                egui::TextEdit::singleline(&mut self.settings_data.full_name),
            )
            .changed();
        ui.label("Your Home Airport:");
        changed |= ui
            .add_sized(
                [100.0, 22.0],
                egui::TextEdit::singleline(&mut self.settings_data.home_airport),
            )
            .changed();
        ui.label("VATSIM Server:");
        ui.label("AUTOMATIC");
        if ui
            .add_enabled(self.auth.is_none(), egui::Button::new("Check credentials"))
            .clicked()
        {
            self.check_credentials(ui.ctx());
        }
        if !self.auth_message.is_empty() {
            let color = if self.auth_success {
                Color32::from_rgb(0, 125, 45)
            } else if self.auth_error {
                Color32::from_rgb(155, 25, 25)
            } else {
                Color32::BLACK
            };
            ui.colored_label(color, &self.auth_message);
        }
        ui.label("Network: disconnected");
        if ui.button("Forget saved password").clicked() {
            match PasswordStore.delete(&self.settings_data.cid) {
                Ok(()) => {
                    self.password.clear();
                    self.auth_message = "Saved password removed.".into();
                    self.auth_success = false;
                    self.auth_error = false;
                }
                Err(error) => {
                    self.auth_message = format!("Could not remove password: {error}");
                    self.auth_error = true;
                }
            }
        }
        if changed {
            self.save_settings();
        }
    }

    fn notification_settings(&mut self, ui: &mut egui::Ui) {
        let labels = [
            "Flash taskbar icon for new private message",
            "Flash taskbar icon for text radio message",
            "Flash taskbar icon for SELCAL message",
            "Flash taskbar icon when disconnected from network",
            "Show incoming text messages in simulator",
        ];
        let mut changed = false;
        for (setting, label) in self.settings_data.notifications.iter_mut().zip(labels) {
            changed |= ui.checkbox(setting, label).changed();
        }
        if changed {
            self.save_settings();
        }
    }

    fn font_settings(&mut self, ui: &mut egui::Ui) {
        let mut changed = false;
        ui.label("Controller List Font:");
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("controller_font")
                .selected_text(if self.settings_data.controller_monospace {
                    "Consolas"
                } else {
                    "Microsoft Sans Serif"
                })
                .show_ui(ui, |ui| {
                    changed |= ui
                        .selectable_value(
                            &mut self.settings_data.controller_monospace,
                            false,
                            "Microsoft Sans Serif",
                        )
                        .changed();
                    changed |= ui
                        .selectable_value(
                            &mut self.settings_data.controller_monospace,
                            true,
                            "Consolas",
                        )
                        .changed();
                });
            changed |= ui
                .add(
                    egui::DragValue::new(&mut self.settings_data.controller_font_pt)
                        .range(7..=24)
                        .suffix(" pt"),
                )
                .changed();
        });
        ui.add_space(8.0);
        ui.label("Text Messages Font:");
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("message_font")
                .selected_text(if self.settings_data.message_monospace {
                    "Consolas"
                } else {
                    "Microsoft Sans Serif"
                })
                .show_ui(ui, |ui| {
                    changed |= ui
                        .selectable_value(
                            &mut self.settings_data.message_monospace,
                            false,
                            "Microsoft Sans Serif",
                        )
                        .changed();
                    changed |= ui
                        .selectable_value(
                            &mut self.settings_data.message_monospace,
                            true,
                            "Consolas",
                        )
                        .changed();
                });
            changed |= ui
                .add(
                    egui::DragValue::new(&mut self.settings_data.message_font_pt)
                        .range(8..=28)
                        .suffix(" pt"),
                )
                .changed();
        });
        if changed {
            self.save_settings();
        }
    }

    fn audio_settings(&mut self, ui: &mut egui::Ui) {
        let mut changed = false;
        ui.label("Microphone Device:");
        egui::ComboBox::from_id_salt("microphone_device")
            .selected_text(if self.settings_data.microphone_device.is_empty() {
                "System default"
            } else {
                &self.settings_data.microphone_device
            })
            .width(310.0)
            .show_ui(ui, |ui| {
                changed |= ui
                    .selectable_value(
                        &mut self.settings_data.microphone_device,
                        String::new(),
                        "System default",
                    )
                    .changed();
                for name in &self.audio_inputs {
                    changed |= ui
                        .selectable_value(
                            &mut self.settings_data.microphone_device,
                            name.clone(),
                            name,
                        )
                        .changed();
                }
            });
        ui.label("Output Device:");
        egui::ComboBox::from_id_salt("output_device")
            .selected_text(if self.settings_data.output_device.is_empty() {
                "System default"
            } else {
                &self.settings_data.output_device
            })
            .width(310.0)
            .show_ui(ui, |ui| {
                changed |= ui
                    .selectable_value(
                        &mut self.settings_data.output_device,
                        String::new(),
                        "System default",
                    )
                    .changed();
                for name in &self.audio_outputs {
                    changed |= ui
                        .selectable_value(&mut self.settings_data.output_device, name.clone(), name)
                        .changed();
                }
            });
        if ui.button("Refresh devices").clicked() {
            self.audio_inputs = AudioDevices.inputs().unwrap_or_default();
            self.audio_outputs = AudioDevices.outputs().unwrap_or_default();
            self.monitor_selection = None;
        }
        changed |= ui
            .checkbox(
                &mut self.settings_data.disable_radio_effects,
                "Disable realistic ATC audio effects",
            )
            .changed();
        changed |= ui
            .checkbox(
                &mut self.settings_data.disable_hf_noise,
                "Disable HF background noise",
            )
            .changed();
        ui.horizontal(|ui| {
            ui.add_sized([95.0, 20.0], egui::Label::new("Output Volume:"));
            changed |= ui
                .add_sized(
                    [245.0, 20.0],
                    egui::Slider::new(&mut self.settings_data.output_volume_db, -24..=12)
                        .suffix(" dB"),
                )
                .changed();
        });
        ui.horizontal(|ui| {
            ui.add_sized([95.0, 20.0], egui::Label::new("Mic Volume:"));
            changed |= ui
                .add_sized(
                    [245.0, 20.0],
                    egui::Slider::new(&mut self.settings_data.microphone_gain_db, -24..=12)
                        .suffix(" dB"),
                )
                .changed();
        });
        if changed {
            self.monitor_selection = None;
            self.save_settings();
        }
        let loopback_config = (
            self.settings_data.microphone_device.clone(),
            self.settings_data.output_device.clone(),
            self.settings_data.microphone_gain_db,
            self.settings_data.output_volume_db,
        );
        if self.audio_loopback.is_some()
            && self.audio_loopback_config.as_ref() != Some(&loopback_config)
            && !ui.input(|input| input.pointer.primary_down())
        {
            self.audio_loopback = None;
            self.start_listening();
        }
        let selection = (
            self.settings_data.microphone_device.clone(),
            self.settings_data.microphone_gain_db,
        );
        if self.audio_loopback.is_none() && self.monitor_selection.as_ref() != Some(&selection) {
            self.audio_monitor = None;
            self.audio_monitor = match AudioDevices.monitor(&selection.0, selection.1) {
                Ok(monitor) => Some(monitor),
                Err(error) => {
                    self.audio_status = format!("Microphone: {error}");
                    None
                }
            };
            self.monitor_selection = Some(selection);
        }
        let level = self
            .audio_loopback
            .as_ref()
            .map(MicrophoneLoopback::level)
            .or_else(|| self.audio_monitor.as_ref().map(MicrophoneMonitor::level))
            .unwrap_or(0.0);
        audio_level_meter(ui, level);
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    self.audio_preview.is_none() && self.audio_loopback.is_none(),
                    egui::Button::new("Test output"),
                )
                .clicked()
            {
                let name = self.settings_data.output_device.clone();
                let volume = self.settings_data.output_volume_db;
                let (sender, receiver) = mpsc::channel();
                std::thread::spawn(move || {
                    let message = match AudioDevices.preview(&name, volume) {
                        Ok(()) => "Output test complete.".into(),
                        Err(error) => format!("Output test failed: {error}"),
                    };
                    let _ = sender.send(message);
                });
                self.audio_preview = Some(receiver);
            }
            let label = if self.audio_loopback.is_some() {
                "Stop listening"
            } else {
                "Listen to microphone"
            };
            if ui
                .add_enabled(self.audio_preview.is_none(), egui::Button::new(label))
                .clicked()
            {
                if self.audio_loopback.take().is_some() {
                    self.audio_loopback_config = None;
                    self.monitor_selection = None;
                    self.audio_status = "Microphone monitoring stopped.".into();
                } else {
                    self.start_listening();
                }
            }
        });
        ui.label(&self.audio_status);
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(50));
    }

    fn start_listening(&mut self) {
        self.audio_monitor = None;
        let config = (
            self.settings_data.microphone_device.clone(),
            self.settings_data.output_device.clone(),
            self.settings_data.microphone_gain_db,
            self.settings_data.output_volume_db,
        );
        match AudioDevices.listen(&config.0, &config.1, config.2, config.3) {
            Ok(loopback) => {
                self.audio_loopback = Some(loopback);
                self.audio_loopback_config = Some(config);
                self.audio_status = "Listening to microphone.".into();
            }
            Err(error) => {
                self.audio_loopback_config = None;
                self.monitor_selection = None;
                self.audio_status = format!("Microphone test failed: {error}");
            }
        }
    }

    fn ptt_settings(&mut self, ui: &mut egui::Ui) {
        ui.label("Current PTT Assignment:");
        let label = match self.settings_data.ptt_key.as_str() {
            "" => "None",
            "CapsLock" => "Caps Lock",
            binding => binding,
        };
        ui.colored_label(
            Color32::from_rgb(0, 0, 220),
            format!("Key or button: {label}"),
        );
        ui.add_space(10.0);
        if ui.button("Set new PTT Key or Button").clicked() {
            self.ptt_capture = true;
        }
        if self.ptt_capture {
            ui.label("Press Caps Lock or another key or extra mouse button.");
        }
        ui.add_space(10.0);
        if ui.button("Clear Current PTT Assignment").clicked() {
            self.settings_data.ptt_key.clear();
            self.ptt_capture = false;
            self.save_settings();
        }
        if self.ptt_pressed {
            ui.colored_label(Color32::from_rgb(0, 130, 0), "PTT pressed");
        }
    }

    fn performance_settings(&mut self, ui: &mut egui::Ui) {
        let mut changed = false;
        ui.horizontal(|ui| {
            ui.label("Do not display aircraft beyond");
            changed |= ui
                .add(egui::DragValue::new(&mut self.settings_data.aircraft_range_nm).range(1..=500))
                .changed();
            ui.label("NM");
        });
        ui.horizontal(|ui| {
            ui.label("Maximum aircraft to display:");
            changed |= ui
                .add(egui::DragValue::new(&mut self.settings_data.max_aircraft).range(1..=9999))
                .changed();
        });
        if changed {
            self.save_settings();
        }
    }

    fn misc_settings(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        let mut changed = ui
            .checkbox(
                &mut self.settings_data.auto_mode_c,
                "Automatically squawk mode C on takeoff",
            )
            .changed();
        if ui
            .checkbox(
                &mut self.settings_data.keep_window_visible,
                "Keep window visible",
            )
            .changed()
        {
            let level = if self.settings_data.keep_window_visible {
                egui::WindowLevel::AlwaysOnTop
            } else {
                egui::WindowLevel::Normal
            };
            ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(level));
            changed = true;
        }
        if changed {
            self.save_settings();
        }
    }

    fn connect_dialog(&mut self, ctx: &egui::Context) {
        let mut open = self.connect_open;
        egui::Window::new("Connect")
            .open(&mut open)
            .default_size([450.0, 194.0])
            .collapsible(false)
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(242, 242, 242))
                    .inner_margin(egui::Margin::same(12)),
            )
            .show(ctx, |ui| {
                ui.visuals_mut().override_text_color = Some(Color32::BLACK);
                ui.visuals_mut().extreme_bg_color = Color32::WHITE;
                ui.visuals_mut().widgets.noninteractive.bg_fill = Color32::WHITE;
                ui.visuals_mut().widgets.inactive.bg_fill = Color32::WHITE;
                ui.visuals_mut().widgets.inactive.weak_bg_fill = Color32::WHITE;
                ui.visuals_mut().widgets.inactive.bg_stroke =
                    Stroke::new(1.0_f32, Color32::from_gray(190));
                ui.visuals_mut().widgets.hovered.bg_fill = Color32::from_rgb(225, 238, 238);
                ui.visuals_mut().widgets.active.bg_fill = Color32::WHITE;
                let mut changed = false;
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label("Callsign:")
                            .on_hover_text("Aircraft callsign used on the network.");
                        changed |= ui
                            .add_sized(
                                [88.0, 20.0],
                                egui::TextEdit::singleline(&mut self.callsign)
                                    .hint_text("Enter..."),
                            )
                            .on_hover_text("Letters and digits, for example CCA1220.")
                            .changed();
                    });
                    ui.vertical(|ui| {
                        ui.label("Type Code:")
                            .on_hover_text("ICAO aircraft type designator.");
                        let response = ui
                            .add_sized(
                                [78.0, 20.0],
                                egui::TextEdit::singleline(&mut self.type_code)
                                    .hint_text("Enter type"),
                            )
                            .on_hover_text("Enter an ICAO aircraft type designator.");
                        changed |= response.changed();
                        if response.changed() || response.gained_focus() || response.clicked() {
                            self.type_suggestions_open = true;
                        }
                        let suggestions: Vec<_> = self
                            .type_catalog
                            .suggestions(&self.type_code, 8)
                            .into_iter()
                            .map(|entry| (entry.code.clone(), entry.aircraft.clone()))
                            .collect();
                        if suggestions.is_empty() {
                            self.type_suggestions_open = false;
                        }
                        let mut selected = None;
                        egui::Popup::from_response(&response)
                            .open_bool(&mut self.type_suggestions_open)
                            .width(280.0)
                            .show(|ui| {
                                for (code, aircraft) in &suggestions {
                                    if ui
                                        .selectable_label(false, format!("{code}  {aircraft}"))
                                        .clicked()
                                    {
                                        selected = Some(code.clone());
                                    }
                                }
                            });
                        if let Some(code) = selected {
                            self.type_code = code;
                            self.type_suggestions_open = false;
                            changed = true;
                        }
                    });
                    ui.vertical(|ui| {
                        ui.label("SELCAL Code:")
                            .on_hover_text("Optional selective calling code.");
                        changed |= ui
                            .add_sized(
                                [78.0, 20.0],
                                egui::TextEdit::singleline(&mut self.selcal).hint_text("Optional"),
                            )
                            .on_hover_text("Four letters, optionally separated by a hyphen.")
                            .changed();
                    });
                });
                changed |= ui
                    .checkbox(
                        &mut self.observer,
                        "Connect in observer mode (for shared cockpit flying)",
                    )
                    .changed();
                if changed {
                    self.settings_data.last_callsign = self.callsign.clone();
                    self.settings_data.last_type_code = self.type_code.clone();
                    self.settings_data.last_selcal = self.selcal.clone();
                    self.settings_data.observer_mode = self.observer;
                    self.save_settings();
                }
                ui.horizontal(|ui| {
                    ui.add_enabled(false, egui::Button::new("Connect"))
                        .on_disabled_hover_text(
                            "FSD session and simulator integration are not complete.",
                        );
                    if ui.button("Cancel").clicked() {
                        self.connect_open = false;
                    }
                });
            });
        self.connect_open &= open;
    }

    fn advanced_settings(&mut self, ui: &mut egui::Ui) {
        ui.label("Aircraft folders in matching priority order:");
        for path in &self.settings_data.model_roots {
            ui.label(path.display().to_string());
        }
    }

    fn custom_rules_settings(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.add_sized(
                [225.0, 23.0],
                egui::TextEdit::singleline(&mut self.new_rule_path).hint_text("Path to .vmr file"),
            );
            if ui.button("Add Custom Rule Set(s)...").clicked() {
                let path = PathBuf::from(self.new_rule_path.trim());
                match ModelRuleSet::load(&path) {
                    Ok(set)
                        if !set.is_empty()
                            && !self.settings_data.custom_rule_paths.contains(&path) =>
                    {
                        self.settings_data.custom_rule_paths.push(path);
                        self.new_rule_path.clear();
                        self.save_settings();
                    }
                    Ok(_) => self.log("Rule set is empty or already installed."),
                    Err(error) => self.log(format!("Could not load rule set: {error}")),
                }
            }
        });
        ui.label("Installed custom rule sets in order of priority:");
        egui::Frame::new()
            .fill(Color32::WHITE)
            .stroke(Stroke::new(1.0_f32, Color32::from_gray(180)))
            .show(ui, |ui| {
                ui.set_min_height(145.0);
                ui.set_min_width(325.0);
                for (index, path) in self.settings_data.custom_rule_paths.iter().enumerate() {
                    let label = match ModelRuleSet::load(path) {
                        Ok(set) => format!("{} ({} rules)", set.name(), set.len()),
                        Err(_) => format!("{} (unavailable)", path.display()),
                    };
                    if ui
                        .selectable_label(self.selected_rule == Some(index), label)
                        .clicked()
                    {
                        self.selected_rule = Some(index);
                    }
                }
            });
        ui.horizontal(|ui| {
            let count = self.settings_data.custom_rule_paths.len();
            let selected = self.selected_rule;
            if ui
                .add_enabled(
                    selected.is_some_and(|index| index > 0),
                    egui::Button::new("Move Up"),
                )
                .clicked()
            {
                let index = selected.unwrap();
                self.settings_data.custom_rule_paths.swap(index, index - 1);
                self.selected_rule = Some(index - 1);
                self.save_settings();
            }
            if ui
                .add_enabled(
                    selected.is_some_and(|index| index + 1 < count),
                    egui::Button::new("Move Down"),
                )
                .clicked()
            {
                let index = selected.unwrap();
                self.settings_data.custom_rule_paths.swap(index, index + 1);
                self.selected_rule = Some(index + 1);
                self.save_settings();
            }
            if ui
                .add_enabled(selected.is_some(), egui::Button::new("Remove"))
                .clicked()
            {
                self.settings_data
                    .custom_rule_paths
                    .remove(selected.unwrap());
                self.selected_rule = None;
                self.save_settings();
            }
        });
    }

    fn model_settings(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.strong("Settings for simulator:");
            ui.label("MSFS 2024 / X-Plane 11 / X-Plane 12");
        });
        ui.add_space(7.0);
        ui.horizontal(|ui| {
            ui.strong("Results:");
            ui.label(format!("{} models found", self.models_found));
            if ui
                .add_enabled(!self.scan_busy, egui::Button::new("Rescan..."))
                .clicked()
            {
                self.rescan();
            }
        });
        ui.add_space(12.0);
        ui.label("Aircraft model directories:");
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut self.new_root);
            if ui.button("Add path").clicked() {
                let path = PathBuf::from(self.new_root.trim());
                if path.is_dir() && !self.settings_data.model_roots.contains(&path) {
                    self.settings_data.model_roots.push(path);
                    self.new_root.clear();
                    self.save_settings();
                } else {
                    self.log("Model path must be an existing directory not already listed.");
                }
            }
        });
        let mut remove = None;
        for (index, path) in self.settings_data.model_roots.iter().enumerate() {
            ui.horizontal(|ui| {
                if ui.small_button("Remove").clicked() {
                    remove = Some(index);
                }
                ui.label(path.display().to_string());
            });
        }
        if let Some(index) = remove {
            self.settings_data.model_roots.remove(index);
            self.save_settings();
        }
    }
}

impl eframe::App for PilotWindow {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_background();
        if self.ptt_capture {
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
            let assignment = if caps_lock_down() {
                Some("CapsLock".into())
            } else {
                ctx.input(|input| {
                    input.events.iter().find_map(|event| match event {
                        egui::Event::Key {
                            key, pressed: true, ..
                        } => Some(format!("{key:?}")),
                        egui::Event::PointerButton {
                            button: egui::PointerButton::Extra1,
                            pressed: true,
                            ..
                        } => Some("MouseExtra1".into()),
                        egui::Event::PointerButton {
                            button: egui::PointerButton::Extra2,
                            pressed: true,
                            ..
                        } => Some("MouseExtra2".into()),
                        _ => None,
                    })
                })
            };
            if let Some(assignment) = assignment {
                self.ptt_capture = false;
                if assignment != "Escape" {
                    self.settings_data.ptt_key = assignment;
                    self.save_settings();
                }
            }
        }
        self.ptt_pressed = ctx.input(|input| {
            (self.settings_data.ptt_key == "CapsLock" && caps_lock_down())
                || egui::Key::ALL.iter().any(|key| {
                    format!("{key:?}") == self.settings_data.ptt_key && input.key_down(*key)
                })
                || (self.settings_data.ptt_key == "MouseExtra1"
                    && input.pointer.button_down(egui::PointerButton::Extra1))
                || (self.settings_data.ptt_key == "MouseExtra2"
                    && input.pointer.button_down(egui::PointerButton::Extra2))
        });
        if !self.settings_open || self.settings_tab != 3 {
            self.audio_monitor = None;
            self.audio_loopback = None;
            self.audio_loopback_config = None;
            self.monitor_selection = None;
        }
        egui::TopBottomPanel::top("toolbar")
            .exact_height(50.0)
            .frame(
                egui::Frame::new()
                    .fill(DARK)
                    .inner_margin(egui::Margin::same(5)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let button = |label| {
                        egui::Button::new(label)
                            .fill(DARK)
                            .stroke(Stroke::new(1.0_f32, BORDER))
                    };
                    if ui.add_sized([94.0, 29.0], button("Connect")).clicked() {
                        self.connect_open = true;
                    }
                    ui.add_enabled_ui(false, |ui| ui.add_sized([94.0, 29.0], button("Mode C")))
                        .inner
                        .on_disabled_hover_text(
                            "Requires a connected simulator and network session.",
                        );
                    ui.add_enabled_ui(false, |ui| ui.add_sized([94.0, 29.0], button("Ident")))
                        .inner
                        .on_disabled_hover_text(
                            "Requires a connected simulator and network session.",
                        );
                    ui.add_enabled_ui(false, |ui| {
                        ui.add_sized([94.0, 29.0], button("Flight Plan"))
                    })
                    .inner
                    .on_disabled_hover_text("Requires a network session.");
                    if ui.add_sized([94.0, 29.0], button("Settings")).clicked() {
                        self.settings_open = true;
                    }
                    if ui.add_sized([29.0, 29.0], button("-")).clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                    }
                    if ui
                        .add_sized(
                            [29.0, 29.0],
                            egui::Button::new("X").fill(Color32::from_rgb(150, 0, 0)),
                        )
                        .clicked()
                    {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    ui.add_space(20.0);
                    ui.vertical(|ui| {
                        for radio in ["COM1", "COM2"] {
                            ui.horizontal(|ui| {
                                ui.label(format!("{radio}: --- . ---"));
                                ui.add_enabled(false, egui::Button::new("TX"))
                                    .on_disabled_hover_text(
                                        "Voice connection is not available yet.",
                                    );
                                ui.add_enabled(false, egui::Button::new("RX"))
                                    .on_disabled_hover_text(
                                        "Voice connection is not available yet.",
                                    );
                            });
                        }
                    });
                    let drag = ui.allocate_response(
                        egui::vec2(ui.available_width().max(0.0), 29.0),
                        egui::Sense::drag(),
                    );
                    if drag.drag_started() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                    }
                });
            });
        egui::SidePanel::left("controllers")
            .exact_width(self.settings_data.controllers_width.clamp(150.0, 650.0))
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(DARK)
                    .inner_margin(egui::Margin::same(5)),
            )
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.label("Controllers In Range:");
                });
                egui::Frame::new()
                    .fill(Color32::BLACK)
                    .stroke(Stroke::new(1.0_f32, BORDER))
                    .inner_margin(egui::Margin::same(5))
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.set_min_height(ui.available_height() - 8.0);
                        let nearby = self
                            .own_position
                            .map(|(latitude, longitude)| {
                                self.controllers.nearby(latitude, longitude)
                            })
                            .unwrap_or_default();
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            for category in AtcCategory::all() {
                                ui.label(RichText::new(category.label()).color(CYAN).font(
                                    display_font(
                                        self.settings_data.controller_font_pt,
                                        self.settings_data.controller_monospace,
                                    ),
                                ));
                                for station in
                                    nearby.iter().filter(|station| station.category == category)
                                {
                                    ui.label(
                                        RichText::new(format!(
                                            "  {} - {}",
                                            station.callsign,
                                            station.frequency()
                                        ))
                                        .color(Color32::WHITE)
                                        .font(
                                            display_font(
                                                self.settings_data.controller_font_pt,
                                                self.settings_data.controller_monospace,
                                            ),
                                        ),
                                    );
                                }
                            }
                        });
                    });
                let right = ui.max_rect().right();
                let handle = egui::Rect::from_x_y_ranges(
                    (right - 10.0)..=(right + 5.0),
                    ui.max_rect().y_range(),
                );
                let response = ui.interact(
                    handle,
                    ui.id().with("controllers_resize"),
                    egui::Sense::drag(),
                );
                if response.hovered() || response.dragged() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                }
                if response.dragged() {
                    if let Some(pointer) = response.interact_pointer_pos() {
                        let width = pointer.x.clamp(150.0, 650.0);
                        if (width - self.settings_data.controllers_width).abs() > 1.0 {
                            self.settings_data.controllers_width = width;
                            self.save_settings();
                        }
                    }
                }
            });
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(DARK)
                    .inner_margin(egui::Margin::same(5)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui
                        .add_sized(
                            [128.0, 27.0],
                            egui::Button::new("Messages")
                                .fill(Color32::BLACK)
                                .stroke(Stroke::new(1.0_f32, BORDER)),
                        )
                        .clicked()
                    {
                        self.show_notes = false;
                    }
                    if ui
                        .add_sized(
                            [128.0, 27.0],
                            egui::Button::new("Notes")
                                .fill(Color32::BLACK)
                                .stroke(Stroke::new(1.0_f32, BORDER)),
                        )
                        .clicked()
                    {
                        self.show_notes = true;
                    }
                });
                let content_width = ui.available_width();
                egui::Frame::new()
                    .fill(Color32::BLACK)
                    .stroke(Stroke::new(1.0_f32, BORDER))
                    .inner_margin(egui::Margin::same(4))
                    .show(ui, |ui| {
                        ui.set_min_width((content_width - 8.0).max(0.0));
                        ui.set_min_height(ui.available_height() - 34.0);
                        if self.show_notes {
                            egui::ScrollArea::vertical()
                                .stick_to_bottom(true)
                                .show(ui, |ui| {
                                    for note in self.settings_data.notes.lines() {
                                        ui.label(
                                            RichText::new(note)
                                                .color(Color32::WHITE)
                                                .monospace()
                                                .size(16.0),
                                        );
                                    }
                                });
                        } else {
                            egui::ScrollArea::vertical()
                                .stick_to_bottom(true)
                                .show(ui, |ui| {
                                    for message in &self.messages {
                                        ui.label(
                                            RichText::new(message).color(Color32::YELLOW).font(
                                                display_font(
                                                    self.settings_data.message_font_pt,
                                                    self.settings_data.message_monospace,
                                                ),
                                            ),
                                        );
                                    }
                                });
                        }
                    });
                egui::Frame::new()
                    .fill(Color32::BLACK)
                    .stroke(Stroke::new(1.0_f32, BORDER))
                    .inner_margin(egui::Margin::same(2))
                    .show(ui, |ui| {
                        ui.set_min_width((content_width - 4.0).max(0.0));
                        let input = if self.show_notes {
                            &mut self.note_input
                        } else {
                            &mut self.chat_input
                        };
                        let response = ui.add_sized(
                            [ui.available_width(), 24.0],
                            egui::TextEdit::singleline(input)
                                .frame(false)
                                .text_color(Color32::WHITE),
                        );
                        if response.lost_focus()
                            && ui.input(|input| input.key_pressed(egui::Key::Enter))
                        {
                            if self.show_notes {
                                let note = self.note_input.trim();
                                if !note.is_empty() {
                                    if !self.settings_data.notes.is_empty() {
                                        self.settings_data.notes.push('\n');
                                    }
                                    self.settings_data.notes.push_str(note);
                                    self.note_input.clear();
                                    self.save_settings();
                                }
                            } else if !self.chat_input.trim().is_empty() {
                                self.log("Cannot send text: no network connection.");
                            }
                        }
                    });
            });
        if self.settings_open {
            self.settings(ctx);
        }
        if self.connect_open {
            self.connect_dialog(ctx);
        }
        if self.scan_busy {
            egui::Window::new("Scanning Installed Aircraft Models ...")
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .fixed_size([480.0, 135.0])
                .collapsible(false)
                .resizable(false)
                .frame(
                    egui::Frame::new()
                        .fill(Color32::from_rgb(242, 242, 242))
                        .inner_margin(egui::Margin::same(18)),
                )
                .show(ctx, |ui| {
                    ui.visuals_mut().override_text_color = Some(Color32::BLACK);
                    ui.vertical_centered(|ui| {
                        ui.add_space(10.0);
                        ui.strong("MSFS 2024 / X-Plane 11 / X-Plane 12");
                        ui.label("Scanning installed aircraft models...");
                        ui.add_space(7.0);
                        ui.add_sized([420.0, 20.0], egui::ProgressBar::new(0.5).animate(true));
                    });
                });
        }
        if self.scan_busy || self.update.is_some() || self.auth.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
}
