use crate::{
    i18n,
    model::*,
    platform::{self, Instance},
    tray::Shell,
    worker::{Command, Snapshot, Worker},
};
use chrono::{DateTime, Local, Utc};
use egui::{Color32, RichText};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex, atomic::Ordering},
    time::Duration,
};
use uuid::Uuid;

#[derive(Clone, Copy, PartialEq)]
enum Page {
    Accounts,
    Manage,
    Settings,
    Switch(Uuid),
    Remove(Uuid),
}
pub struct App {
    state: Snapshot,
    worker: Option<Worker>,
    _instance: Option<Instance>,
    tray: Option<Shell>,
    shared: Arc<Mutex<Snapshot>>,
    page: Page,
    repository: String,
    import_path: String,
    exit: bool,
    initial_hide: bool,
    fixture: bool,
    screenshot: Option<PathBuf>,
    capture_frames: u32,
    quit_after_capture: bool,
    local_error: Option<String>,
}
pub struct Options {
    pub fixture: bool,
    pub shell_fixture: bool,
    pub empty: bool,
    pub language: String,
    pub theme: Option<String>,
    pub scale: Option<f32>,
    pub page: String,
    pub screenshot: Option<PathBuf>,
    pub quit_after_capture: bool,
    pub background: bool,
}
impl App {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        instance: Option<Instance>,
        options: Options,
    ) -> Self {
        if let Some(theme) = &options.theme {
            cc.egui_ctx.set_theme(if theme == "dark" {
                egui::Theme::Dark
            } else {
                egui::Theme::Light
            });
        }
        if let Some(scale) = options.scale {
            cc.egui_ctx.set_pixels_per_point(scale);
        }
        cc.egui_ctx.style_mut(|s| {
            for (style, size) in [
                (egui::TextStyle::Body, 17.0),
                (egui::TextStyle::Button, 17.0),
                (egui::TextStyle::Small, 14.5),
                (egui::TextStyle::Heading, 24.0),
                (egui::TextStyle::Monospace, 16.0),
            ] {
                s.text_styles
                    .insert(style, egui::FontId::proportional(size));
            }
            s.spacing.item_spacing = egui::vec2(9.0, 11.0);
            s.spacing.button_padding = egui::vec2(11.0, 7.0);
        });
        let mut local_error = None;
        let state = if options.fixture {
            fixture(&options.language, options.empty)
        } else {
            Snapshot::default()
        };
        let shared = Arc::new(Mutex::new(state.clone()));
        let tray = if options.fixture && !options.shell_fixture {
            None
        } else {
            let window = cc.window_handle().ok().and_then(|h| match h.as_raw() {
                RawWindowHandle::Win32(h) => Some(h.hwnd.get()),
                _ => None,
            });
            let shell = window
                .ok_or_else(|| anyhow::anyhow!("Native window handle unavailable"))
                .and_then(|h| {
                    Shell::start(
                        cc.egui_ctx.clone(),
                        h,
                        instance.as_ref().map(Instance::activation_handle),
                        shared.clone(),
                    )
                });
            match shell {
                Ok(t) => Some(t),
                Err(e) => {
                    local_error = Some(format!("Tray could not start: {e}"));
                    None
                }
            }
        };
        let worker = if options.fixture {
            None
        } else {
            Some(Worker::start(
                platform::local_data().join("Cat Codex Switcher"),
                platform::active_home(),
                cc.egui_ctx.clone(),
                shared.clone(),
            ))
        };
        let page = match options.page.as_str() {
            "settings" => Page::Settings,
            "manage" => Page::Manage,
            "switch" => state
                .rows
                .first()
                .map(|r| Page::Switch(r.profile.id))
                .unwrap_or(Page::Accounts),
            "remove" => state
                .rows
                .last()
                .map(|r| Page::Remove(r.profile.id))
                .unwrap_or(Page::Accounts),
            _ => Page::Accounts,
        };
        Self {
            repository: state.settings.update_repository.clone(),
            state,
            worker,
            _instance: instance,
            tray,
            shared,
            page,
            import_path: platform::local_data()
                .join("Codex Account Switcher")
                .display()
                .to_string(),
            exit: false,
            initial_hide: options.background,
            fixture: options.fixture,
            screenshot: options.screenshot,
            capture_frames: 0,
            quit_after_capture: options.quit_after_capture,
            local_error,
        }
    }
    fn send(&mut self, command: Command) {
        if let Some(worker) = &self.worker {
            worker.send(command);
            if !self.state.busy {
                self.state.busy = true;
                if let Ok(mut s) = self.shared.lock() {
                    s.busy = true;
                }
            }
        }
    }
    fn show(&mut self, ctx: &egui::Context) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        if self.state.adding {
            self.page = Page::Manage;
        }
        if !self.state.busy {
            self.send(Command::Refresh);
        }
    }
    fn quit(&mut self, ctx: &egui::Context) {
        if self.state.busy || self.state.adding {
            return;
        }
        self.exit = true;
        if let Some(worker) = &self.worker {
            worker.send(Command::Shutdown);
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
    fn accounts(&mut self, ui: &mut egui::Ui, lang: &str, manage: bool) {
        if manage {
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(!self.state.busy, egui::Button::new(i18n::t(lang, "add")))
                    .clicked()
                {
                    self.send(Command::Add);
                }
                ui.small(i18n::t(lang, "browser_hint"));
            });
            if self.state.adding {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(i18n::t(lang, "login"));
                    if ui.button(i18n::t(lang, "cancel")).clicked()
                        && let Some(worker) = &self.worker
                    {
                        worker.cancel.store(true, Ordering::SeqCst);
                    }
                });
            }
            if ui
                .add_enabled(
                    !self.state.busy,
                    egui::Button::new(i18n::t(lang, "register")),
                )
                .clicked()
            {
                self.send(Command::Register);
            }
            ui.separator();
        }
        if self.state.rows.is_empty() {
            ui.add_space(25.0);
            ui.label(i18n::t(lang, "empty"));
            ui.add_space(25.0);
        }
        for row in self.state.rows.clone() {
            let active = self.state.active == Some(row.profile.id);
            let fill = if active {
                ui.visuals().selection.bg_fill.gamma_multiply(0.32)
            } else {
                ui.visuals().faint_bg_color
            };
            let card = egui::Frame::new()
                .fill(fill)
                .inner_margin(12.0)
                .corner_radius(6.0)
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.horizontal(|ui| {
                        let width =
                            (ui.available_width() - if manage { 120.0 } else { 12.0 }).max(80.0);
                        ui.allocate_ui_with_layout(
                            egui::vec2(width, 24.0),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                ui.add(
                                    egui::Label::new(RichText::new(row.profile.title()).strong())
                                        .truncate(),
                                )
                                .on_hover_text(row.profile.title());
                            },
                        );
                        if manage
                            && ui
                                .add_enabled(
                                    !active && !self.state.busy,
                                    egui::Button::new(i18n::t(lang, "remove")),
                                )
                                .clicked()
                        {
                            self.page = Page::Remove(row.profile.id);
                        }
                    });
                    if let Some(usage) = &row.usage {
                        usage_line(
                            ui,
                            i18n::t(lang, "weekly"),
                            usage.remaining_percent,
                            usage.resets_at,
                            lang,
                        );
                        if self.state.settings.shows_five_hour_usage
                            && let (Some(p), Some(at)) =
                                (usage.five_hour_remaining_percent, usage.five_hour_resets_at)
                        {
                            usage_line(ui, i18n::t(lang, "five_hour"), p, at, lang);
                        }
                    } else {
                        ui.small(i18n::t(lang, "unavailable"))
                            .on_hover_text(row.error.as_deref().unwrap_or(""));
                    }
                    account_info(ui, &row, lang);
                    if row.stale {
                        ui.colored_label(ui.visuals().warn_fg_color, i18n::t(lang, "stale"))
                            .on_hover_text(row.error.as_deref().unwrap_or(""));
                    }
                });
            if !manage {
                let response = card
                    .response
                    .interact(egui::Sense::click())
                    .on_hover_text(row.profile.title());
                response.widget_info(|| {
                    egui::WidgetInfo::selected(
                        egui::WidgetType::Button,
                        !self.state.busy,
                        active,
                        row.profile.title(),
                    )
                });
                if response.clicked() && !self.state.busy && !active {
                    self.page = Page::Switch(row.profile.id);
                }
            }
            ui.add_space(3.0);
        }
        if manage {
            ui.separator();
            ui.strong(i18n::t(lang, "import"));
            ui.small(i18n::t(lang, "import_hint"));
            ui.add(egui::TextEdit::singleline(&mut self.import_path).desired_width(f32::INFINITY));
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(
                        !self.state.busy,
                        egui::Button::new(i18n::t(lang, "choose_folder")),
                    )
                    .clicked()
                    && let Some(path) = rfd::FileDialog::new()
                        .set_directory(&self.import_path)
                        .pick_folder()
                {
                    self.import_path = path.display().to_string();
                }
                if ui
                    .add_enabled(
                        !self.state.busy,
                        egui::Button::new(i18n::t(lang, "preview")),
                    )
                    .clicked()
                {
                    self.send(Command::Preview(PathBuf::from(&self.import_path)));
                }
            });
        }
    }
    fn settings(&mut self, ui: &mut egui::Ui, lang: &str) {
        let mut s = self.state.settings.clone();
        ui.strong(i18n::t(lang, "general"));
        ui.checkbox(&mut s.launch_at_login, i18n::t(lang, "startup"));
        ui.checkbox(
            &mut s.shows_menu_bar_percentage,
            i18n::t(lang, "percentage"),
        );
        ui.horizontal(|ui| {
            ui.label(i18n::t(lang, "tray_window"));
            egui::ComboBox::from_id_salt("tray_window")
                .selected_text(i18n::t(
                    lang,
                    if s.status_bar_usage_window == "weekly" {
                        "weekly"
                    } else {
                        "five_hour"
                    },
                ))
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut s.status_bar_usage_window,
                        "fiveHour".into(),
                        i18n::t(lang, "five_hour"),
                    );
                    ui.selectable_value(
                        &mut s.status_bar_usage_window,
                        "weekly".into(),
                        i18n::t(lang, "weekly"),
                    );
                });
        });
        ui.checkbox(&mut s.shows_five_hour_usage, i18n::t(lang, "five_toggle"));
        ui.horizontal(|ui| {
            ui.label(i18n::t(lang, "language"));
            egui::ComboBox::from_id_salt("language")
                .selected_text(match s.language.as_str() {
                    "uk" => "Українська",
                    "ru" => "Русский",
                    "system" => i18n::t(lang, "system"),
                    _ => "English",
                })
                .show_ui(ui, |ui| {
                    for (code, title) in [
                        ("uk", "Українська"),
                        ("en", "English"),
                        ("ru", "Русский"),
                        ("system", i18n::t(lang, "system")),
                    ] {
                        ui.selectable_value(&mut s.language, code.into(), title);
                    }
                });
        });
        ui.separator();
        ui.strong(i18n::t(lang, "experimental"));
        ui.checkbox(
            &mut s.reset_expiry_experiment_enabled,
            i18n::t(lang, "experiment"),
        );
        ui.small(i18n::t(lang, "experiment_hint"));
        if ui.button(i18n::t(lang, "open_log")).clicked()
            && !self.fixture
            && let Err(e) = platform::open(platform::local_data().join("Cat Codex Switcher"))
        {
            self.local_error = Some(e.to_string());
        }
        ui.separator();
        ui.strong(i18n::t(lang, "updates"));
        ui.checkbox(
            &mut s.automatically_check_updates,
            i18n::t(lang, "auto_update"),
        );
        ui.label(i18n::t(lang, "repository"));
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.repository)
                    .hint_text("owner/repository")
                    .desired_width(ui.available_width() - 110.0),
            );
            if ui.button(i18n::t(lang, "save")).clicked() {
                s.update_repository = self.repository.trim().into();
            }
        });
        let status = if self.state.settings.update_repository.is_empty() {
            "update_unconfigured"
        } else {
            &self.state.update_status
        };
        ui.small(i18n::t(lang, status));
        ui.horizontal_wrapped(|ui| {
            ui.small(concat!("v", env!("CARGO_PKG_VERSION")));
            if let Some(url) = self.state.update_url.clone() {
                if ui.button(i18n::t(lang, "download")).clicked()
                    && let Err(e) = platform::browser(&url)
                {
                    self.local_error = Some(e.to_string());
                }
            } else if ui.button(i18n::t(lang, "check")).clicked() {
                self.send(Command::CheckUpdates);
            }
        });
        if s != self.state.settings {
            if self.fixture {
                self.state.settings = s;
            } else {
                self.send(Command::Settings(s));
            }
        }
    }
    fn confirmation(&mut self, ui: &mut egui::Ui, lang: &str, id: Uuid, remove: bool) {
        if let Some(row) = self.state.rows.iter().find(|r| r.profile.id == id) {
            ui.heading(i18n::t(lang, if remove { "remove" } else { "switch" }));
            ui.label(row.profile.title());
            ui.add_space(10.0);
            ui.label(i18n::t(
                lang,
                if remove { "remove_hint" } else { "switch_hint" },
            ));
            ui.add_space(15.0);
            ui.horizontal(|ui| {
                if ui.button(i18n::t(lang, "cancel")).clicked() {
                    self.page = if remove { Page::Manage } else { Page::Accounts };
                }
                let confirm = ui.add_enabled(
                    !self.state.busy,
                    egui::Button::new(i18n::t(lang, "confirm")),
                );
                if confirm.clicked() {
                    self.send(if remove {
                        Command::Remove(id)
                    } else {
                        Command::Switch(id)
                    });
                    self.page = if remove { Page::Manage } else { Page::Accounts };
                }
            });
        } else {
            self.page = Page::Accounts;
        }
    }
}
impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.worker.is_some()
            && let Ok(state) = self.shared.lock()
        {
            if self.state.settings.update_repository != state.settings.update_repository {
                self.repository = state.settings.update_repository.clone();
            }
            self.state = state.clone();
        }
        if self.initial_hide {
            self.initial_hide = false;
            if self.tray.is_some() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            }
        }
        if self.tray.as_ref().is_some_and(Shell::take_open) {
            self.show(ctx);
        }
        if self.tray.as_ref().is_some_and(Shell::take_quit) {
            self.quit(ctx);
        }
        let lang = i18n::language(&self.state.settings.language);
        if ctx.input(|i| i.viewport().close_requested()) && !self.exit {
            if self.tray.is_some() {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            } else if self.state.busy {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            } else {
                self.quit(ctx);
            }
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) && !self.state.busy {
            self.page = Page::Accounts;
        }
        egui::TopBottomPanel::top("navigation").show(ctx, |ui| {
            ui.horizontal(|ui| {
                for (page, key) in [
                    (Page::Accounts, "accounts"),
                    (Page::Manage, "manage"),
                    (Page::Settings, "settings"),
                ] {
                    if ui
                        .selectable_label(self.page == page, i18n::t(&lang, key))
                        .clicked()
                    {
                        self.page = page;
                    }
                }
            });
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(error) = self
                .local_error
                .clone()
                .or_else(|| self.state.error.clone())
            {
                egui::Frame::new()
                    .fill(ui.visuals().error_fg_color.gamma_multiply(0.12))
                    .inner_margin(8.0)
                    .show(ui, |ui| {
                        ui.strong(i18n::t(&lang, "error"));
                        ui.label(error);
                        if ui.button(i18n::t(&lang, "dismiss")).clicked() {
                            self.local_error = None;
                            self.send(Command::Dismiss);
                        }
                    });
            }
            if self.state.busy || !self.state.stage.is_empty() {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(i18n::t(
                        &lang,
                        if self.state.stage.is_empty() {
                            "working"
                        } else {
                            &self.state.stage
                        },
                    ));
                });
            }
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| match self.page {
                    Page::Accounts => self.accounts(ui, &lang, false),
                    Page::Manage => self.accounts(ui, &lang, true),
                    Page::Settings => {
                        ui.add_enabled_ui(!self.state.busy, |ui| self.settings(ui, &lang));
                    }
                    Page::Switch(id) => self.confirmation(ui, &lang, id, false),
                    Page::Remove(id) => self.confirmation(ui, &lang, id, true),
                });
        });
        if let Some(preview) = self.state.preview.clone() {
            egui::Window::new(i18n::t(&lang, "preview"))
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label(preview.source.display().to_string());
                    for title in &preview.titles {
                        ui.label(title);
                    }
                    ui.label(format!(
                        "{}: {}",
                        i18n::t(&lang, "duplicates"),
                        preview.duplicates
                    ));
                    ui.small(i18n::t(&lang, "import_hint"));
                    ui.horizontal(|ui| {
                        if ui.button(i18n::t(&lang, "cancel")).clicked() {
                            self.send(Command::Dismiss);
                        }
                        if ui
                            .add_enabled(
                                !self.state.busy,
                                egui::Button::new(i18n::t(&lang, "confirm")),
                            )
                            .clicked()
                        {
                            self.send(Command::Import(preview.source));
                        }
                    });
                });
        }
        if self.fixture && self.screenshot.is_some() {
            self.capture_frames += 1;
            if self.capture_frames == 4 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            }
            let captured = ctx.input(|i| {
                i.events.iter().find_map(|e| {
                    if let egui::Event::Screenshot { image, .. } = e {
                        Some(image.clone())
                    } else {
                        None
                    }
                })
            });
            if let Some(image) = captured
                && let Some(path) = self.screenshot.take()
            {
                let bytes: Vec<_> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
                if let Err(e) = image::save_buffer(
                    path,
                    &bytes,
                    image.width() as u32,
                    image.height() as u32,
                    image::ColorType::Rgba8,
                ) {
                    self.local_error = Some(e.to_string());
                }
                if self.quit_after_capture {
                    self.exit = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
        ctx.request_repaint_after(if self.fixture && self.screenshot.is_some() {
            Duration::from_millis(80)
        } else {
            Duration::from_millis(400)
        });
        if self.fixture
            && let Ok(mut state) = self.shared.lock()
        {
            *state = self.state.clone();
        }
    }
}
impl Drop for App {
    fn drop(&mut self) {
        self.tray.take();
        if let Some(worker) = &mut self.worker {
            worker.cancel.store(true, Ordering::SeqCst);
            let _ = worker.commands.send(Command::Shutdown);
            if let Some(join) = worker.join.take() {
                let _ = join.join();
            }
        }
    }
}
fn usage_line(ui: &mut egui::Ui, title: &str, p: i32, at: DateTime<Utc>, lang: &str) {
    ui.horizontal(|ui| {
        ui.small(title);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.small(format!(
                "{p}%  {} {}",
                i18n::t(lang, "resets"),
                at.with_timezone(&Local).format("%m-%d %H:%M")
            ));
        });
    });
    ui.add(
        egui::ProgressBar::new(p as f32 / 100.0)
            .desired_height(4.0)
            .fill(if p < 15 {
                Color32::from_rgb(220, 80, 65)
            } else {
                ui.visuals().selection.bg_fill
            }),
    );
}
fn account_info(ui: &mut egui::Ui, row: &crate::worker::Row, lang: &str) {
    ui.horizontal_wrapped(|ui| {
        if let Some((until, checked)) = row.subscription {
            ui.small(format!(
                "{}: {}",
                i18n::t(lang, "subscription"),
                until.with_timezone(&Local).format("%Y-%m-%d")
            ))
            .on_hover_text(format!(
                "{} {}",
                i18n::t(lang, "snapshot_hint"),
                checked.with_timezone(&Local).format("%Y-%m-%d %H:%M")
            ));
        } else {
            ui.small(format!(
                "{}: {}",
                i18n::t(lang, "subscription"),
                i18n::t(lang, "unknown")
            ));
        }
        let credits = row
            .usage
            .as_ref()
            .and_then(|u| u.reset_credits_remaining)
            .map(|n| n.to_string())
            .unwrap_or_else(|| i18n::t(lang, "unknown").into());
        ui.small(format!("{}: {}", i18n::t(lang, "credits"), credits));
    });
}
pub fn fixture(language: &str, empty: bool) -> Snapshot {
    let mut state = Snapshot {
        busy: false,
        ..Default::default()
    };
    state.settings.language = language.into();
    state.settings.shows_five_hour_usage = true;
    if empty {
        return state;
    }
    for (index, email) in [
        "cat@example.invalid",
        "a-very-long-account-name-for-layout-testing@example.invalid",
        "work@example.invalid",
    ]
    .iter()
    .enumerate()
    {
        let profile = Identity {
            account_id: Some(format!("fixture-{index}")),
            email: Some((*email).into()),
        }
        .profile();
        if index == 0 {
            state.active = Some(profile.id);
        }
        state.rows.push(crate::worker::Row {
            profile,
            usage: Some(Usage {
                remaining_percent: [74, 32, 8][index],
                resets_at: Utc::now() + chrono::Duration::days(3),
                five_hour_remaining_percent: Some([92, 51, 10][index]),
                five_hour_resets_at: Some(Utc::now() + chrono::Duration::hours(2)),
                reset_credits_remaining: Some(index as i64),
                reset_credits: None,
            }),
            stale: index == 2,
            error: None,
            subscription: Some((Utc::now() + chrono::Duration::days(12), Utc::now())),
        });
    }
    state
}
