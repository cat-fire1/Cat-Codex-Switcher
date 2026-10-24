use crate::windows_style::{self as win, Palette};
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
    fitted_height: f32,
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
    pub five_hour: bool,
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
        win::install_fonts(&cc.egui_ctx);
        win::configure(&cc.egui_ctx);
        let mut local_error = None;
        let mut state = if options.fixture {
            fixture(&options.language, options.empty)
        } else {
            Snapshot::default()
        };
        if options.fixture {
            state.settings.shows_five_hour_usage = options.five_hour;
        }
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
            fitted_height: 0.0,
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
    fn header(&mut self, ui: &mut egui::Ui, lang: &str) {
        egui::Frame::new()
            .inner_margin(egui::Margin {
                left: 20,
                right: 20,
                top: 16,
                bottom: 16,
            })
            .show(ui, |ui| {
                ui.set_width(380.0);
                if self.page == Page::Accounts {
                    ui.horizontal(|ui| {
                        ui.label(win::title(i18n::t(lang, "accounts"), 20.0));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_enabled_ui(!self.state.busy, |ui| {
                                let settings = win::button(ui, "", Some('\u{e713}'))
                                    .on_hover_text(i18n::t(lang, "settings"));
                                settings.widget_info(|| {
                                    egui::WidgetInfo::labeled(
                                        egui::WidgetType::Button,
                                        true,
                                        i18n::t(lang, "settings"),
                                    )
                                });
                                if self.state.update_url.is_some() {
                                    ui.painter().circle_filled(
                                        settings.rect.right_top() + egui::vec2(-7.0, 7.0),
                                        2.5,
                                        Palette::current(ui).accent,
                                    );
                                }
                                if settings.clicked() {
                                    self.page = Page::Settings;
                                }
                                if win::button(ui, i18n::t(lang, "manage"), Some('\u{e716}'))
                                    .clicked()
                                {
                                    self.page = Page::Manage;
                                }
                            });
                        });
                    });
                } else {
                    let title = match self.page {
                        Page::Settings => i18n::t(lang, "settings").to_owned(),
                        Page::Switch(id) | Page::Remove(id) => {
                            let name = self
                                .state
                                .rows
                                .iter()
                                .find(|r| r.profile.id == id)
                                .map(|r| r.profile.display_name.as_str())
                                .unwrap_or("");
                            i18n::t(
                                lang,
                                if matches!(self.page, Page::Remove(_)) {
                                    "remove_title"
                                } else {
                                    "switch_title"
                                },
                            )
                            .replace("%@", name)
                        }
                        _ => i18n::t(lang, "accounts").to_owned(),
                    };
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled_ui(!self.state.busy, |ui| {
                                win::button(ui, "", Some('\u{e72b}'))
                            })
                            .inner
                            .on_hover_text(i18n::t(lang, "back"))
                            .clicked()
                        {
                            self.page = if matches!(self.page, Page::Remove(_)) {
                                Page::Manage
                            } else {
                                Page::Accounts
                            };
                        }
                        ui.add_space(4.0);
                        win::ellipsis(ui, &title, ui.available_width(), 20.0, true, false);
                    });
                }
            });
    }
    fn accounts(&mut self, ui: &mut egui::Ui, lang: &str, manage: bool) {
        let p = Palette::current(ui);
        egui::Frame::new()
            .inner_margin(egui::Margin {
                left: 20,
                right: 20,
                top: 0,
                bottom: 20,
            })
            .show(ui, |ui| {
                if !manage && !self.state.rows.is_empty() && self.state.active.is_none() {
                    win::muted(ui, i18n::t(lang, "active_unconfirmed"), 11.0);
                    if win::button(ui, i18n::t(lang, "manage"), None).clicked() {
                        self.page = Page::Manage;
                    }
                    ui.add_space(8.0);
                }
                egui::Frame::new()
                    .fill(p.list)
                    .stroke(egui::Stroke::new(1.0_f32, p.line))
                    .corner_radius(4)
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.spacing_mut().item_spacing.y = 0.0;
                        if self.state.rows.is_empty() {
                            egui::Frame::new()
                                .inner_margin(egui::Margin::symmetric(12, 24))
                                .show(ui, |ui| {
                                    win::muted(ui, i18n::t(lang, "empty"), 12.0);
                                });
                        }
                        for row in self.state.rows.clone() {
                            let active = self.state.active == Some(row.profile.id);
                            let row_height = if manage {
                                56.0
                            } else if self.state.settings.shows_five_hour_usage
                                && row
                                    .usage
                                    .as_ref()
                                    .is_some_and(|u| u.five_hour_remaining_percent.is_some())
                            {
                                94.0
                            } else {
                                74.0
                            };
                            let (rect, response) = ui.allocate_exact_size(
                                egui::vec2(ui.available_width(), row_height),
                                if manage {
                                    egui::Sense::hover()
                                } else {
                                    egui::Sense::click()
                                },
                            );
                            let fill = if !manage && response.hovered() {
                                p.hover
                            } else if !manage && active {
                                p.selected
                            } else {
                                Color32::TRANSPARENT
                            };
                            ui.painter().rect_filled(rect, 0, fill);
                            if !manage && active {
                                let stripe = egui::Rect::from_center_size(
                                    egui::pos2(rect.left() + 1.5, rect.center().y),
                                    egui::vec2(3.0, 24.0),
                                );
                                ui.painter().rect_filled(stripe, 0, p.accent);
                            }
                            if !manage && response.has_focus() {
                                ui.painter().rect_stroke(
                                    rect,
                                    0,
                                    egui::Stroke::new(1.0_f32, p.accent),
                                    egui::StrokeKind::Inside,
                                );
                            }
                            response.widget_info(|| {
                                egui::WidgetInfo::selected(
                                    egui::WidgetType::Button,
                                    !self.state.busy,
                                    active,
                                    row.profile.title(),
                                )
                            });
                            if !manage && response.clicked() && !active && !self.state.busy {
                                self.page = Page::Switch(row.profile.id);
                            }
                            let opacity = if self.state.busy && !manage {
                                0.45
                            } else {
                                1.0
                            };
                            let content = rect.shrink2(egui::vec2(16.0, 12.0));
                            let avatar = egui::Rect::from_center_size(
                                egui::pos2(content.left() + 16.0, rect.center().y),
                                egui::vec2(32.0, 32.0),
                            );
                            ui.painter()
                                .rect_filled(avatar, 4, p.hover.gamma_multiply(opacity));
                            let initials = initials(&row.profile.display_name);
                            ui.painter().text(
                                avatar.center(),
                                egui::Align2::CENTER_CENTER,
                                initials,
                                win::semibold(11.0),
                                p.ink.gamma_multiply(opacity),
                            );
                            let detail = egui::Rect::from_min_max(
                                egui::pos2(content.left() + 44.0, content.top()),
                                content.max,
                            );
                            let mut detail_ui = ui.new_child(
                                egui::UiBuilder::new()
                                    .id_salt(row.profile.id)
                                    .max_rect(detail)
                                    .layout(egui::Layout::top_down(egui::Align::Min)),
                            );
                            {
                                let ui = &mut detail_ui;
                                ui.set_clip_rect(rect.intersect(ui.clip_rect()));

                                ui.set_opacity(opacity);
                                ui.spacing_mut().interact_size.y = 0.0;
                                if manage {
                                    ui.allocate_ui_with_layout(
                                        egui::vec2(detail.width(), 32.0),
                                        egui::Layout::left_to_right(egui::Align::Center),
                                        |ui| {
                                            let action_width = if active { 54.0 } else { 38.0 };
                                            win::ellipsis(
                                                ui,
                                                row.profile.title(),
                                                (detail.width() - action_width - 8.0).max(30.0),
                                                14.0,
                                                true,
                                                false,
                                            );
                                            if active {
                                                win::muted(ui, i18n::t(lang, "active"), 11.0);
                                            } else if ui
                                                .add_enabled_ui(!self.state.busy, |ui| {
                                                    win::button(ui, "", Some('\u{e74d}'))
                                                })
                                                .inner
                                                .on_hover_text(i18n::t(lang, "remove"))
                                                .clicked()
                                            {
                                                self.page = Page::Remove(row.profile.id);
                                            }
                                        },
                                    );
                                } else {
                                    let reset = if !self.state.settings.shows_five_hour_usage {
                                        row.usage
                                            .as_ref()
                                            .map(|u| reset_text(u.resets_at, lang, true))
                                            .unwrap_or_default()
                                    } else {
                                        String::new()
                                    };
                                    let reset_width = if reset.is_empty() {
                                        0.0
                                    } else {
                                        ui.painter()
                                            .layout_no_wrap(
                                                reset.clone(),
                                                egui::FontId::proportional(10.5),
                                                p.muted,
                                            )
                                            .size()
                                            .x
                                            + 7.0
                                    };
                                    ui.horizontal(|ui| {
                                        win::ellipsis(
                                            ui,
                                            row.profile.title(),
                                            (detail.width() - reset_width).max(30.0),
                                            14.0,
                                            true,
                                            false,
                                        );
                                        if !reset.is_empty() {
                                            ui.label(
                                                RichText::new(reset).size(10.5).color(p.muted),
                                            );
                                        }
                                    });
                                    if let Some(usage) = &row.usage {
                                        if self.state.settings.shows_five_hour_usage
                                            && let (Some(five), Some(at)) = (
                                                usage.five_hour_remaining_percent,
                                                usage.five_hour_resets_at,
                                            )
                                        {
                                            usage_line(
                                                ui,
                                                i18n::t(lang, "five_hour"),
                                                five,
                                                Some(reset_text(at, lang, false)),
                                                lang,
                                            );
                                        }
                                        usage_line(
                                            ui,
                                            i18n::t(
                                                lang,
                                                if self.state.settings.shows_five_hour_usage {
                                                    "weekly"
                                                } else {
                                                    "usage"
                                                },
                                            ),
                                            usage.remaining_percent,
                                            if self.state.settings.shows_five_hour_usage {
                                                Some(reset_text(usage.resets_at, lang, true))
                                            } else {
                                                None
                                            },
                                            lang,
                                        );
                                    } else {
                                        win::muted(ui, i18n::t(lang, "unavailable"), 10.5)
                                            .on_hover_text(row.error.as_deref().unwrap_or(""));
                                    }
                                    account_info(ui, &row, lang);
                                }
                            }
                            if row.stale {
                                response.on_hover_text(format!(
                                    "{}\n{}",
                                    i18n::t(lang, "stale"),
                                    row.error.as_deref().unwrap_or("")
                                ));
                            } else if let Some(error) = &row.error {
                                response.on_hover_text(error);
                            }
                        }
                    });
                if manage {
                    ui.add_space(20.0);
                    ui.horizontal(|ui| {
                        let add = ui
                            .add_enabled_ui(self.state.adding || !self.state.busy, |ui| {
                                win::button(
                                    ui,
                                    i18n::t(lang, if self.state.adding { "cancel" } else { "add" }),
                                    Some(if self.state.adding {
                                        '\u{e711}'
                                    } else {
                                        '\u{e710}'
                                    }),
                                )
                            })
                            .inner;
                        if add.clicked() {
                            if self.state.adding {
                                if let Some(worker) = &self.worker {
                                    worker.cancel.store(true, Ordering::SeqCst);
                                }
                            } else {
                                self.send(Command::Add);
                            }
                        }
                        win::ellipsis(
                            ui,
                            i18n::t(
                                lang,
                                if self.state.adding {
                                    "login"
                                } else {
                                    "browser_hint"
                                },
                            ),
                            ui.available_width(),
                            12.0,
                            false,
                            true,
                        );
                    });
                    ui.add_space(8.0);
                    if ui
                        .add_enabled_ui(!self.state.busy && !self.state.adding, |ui| {
                            win::button(ui, i18n::t(lang, "register"), None)
                        })
                        .inner
                        .clicked()
                    {
                        self.send(Command::Register);
                    }
                    ui.add_space(12.0);
                    egui::CollapsingHeader::new(i18n::t(lang, "import")).show(ui, |ui| {
                        win::muted(ui, i18n::t(lang, "import_hint"), 10.5);
                        ui.add_space(8.0);
                        ui.add(
                            egui::TextEdit::singleline(&mut self.import_path)
                                .desired_width(ui.available_width()),
                        );
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            if ui
                                .add_enabled_ui(!self.state.busy, |ui| {
                                    win::button(ui, i18n::t(lang, "choose_folder"), None)
                                })
                                .inner
                                .clicked()
                                && let Some(path) = rfd::FileDialog::new()
                                    .set_directory(&self.import_path)
                                    .pick_folder()
                            {
                                self.import_path = path.display().to_string();
                            }
                            if ui
                                .add_enabled_ui(!self.state.busy, |ui| {
                                    win::button(ui, i18n::t(lang, "preview"), None)
                                })
                                .inner
                                .clicked()
                            {
                                self.send(Command::Preview(PathBuf::from(&self.import_path)));
                            }
                        });
                    });
                }
            });
    }
    fn settings(&mut self, ui: &mut egui::Ui, lang: &str) {
        let mut s = self.state.settings.clone();
        egui::Frame::new()
            .inner_margin(egui::Margin {
                left: 20,
                right: 20,
                top: 0,
                bottom: 20,
            })
            .show(ui, |ui| {
                ui.label(win::title(i18n::t(lang, "general"), 14.0));
                win::toggle(ui, &mut s.launch_at_login, i18n::t(lang, "startup"));
                win::toggle(
                    ui,
                    &mut s.shows_menu_bar_percentage,
                    i18n::t(lang, "percentage"),
                );
                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), 48.0),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.label(i18n::t(lang, "tray_window"));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            egui::ComboBox::from_id_salt("tray_window")
                                .width(160.0)
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
                    },
                );
                win::toggle(
                    ui,
                    &mut s.shows_five_hour_usage,
                    i18n::t(lang, "five_toggle"),
                );
                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), 48.0),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.label(i18n::t(lang, "language"));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            egui::ComboBox::from_id_salt("language")
                                .width(160.0)
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
                    },
                );
                win::section(ui, i18n::t(lang, "experimental"));
                win::toggle(
                    ui,
                    &mut s.reset_expiry_experiment_enabled,
                    i18n::t(lang, "experiment"),
                );
                win::muted(ui, i18n::t(lang, "experiment_hint"), 10.5);
                ui.add_space(8.0);
                if win::button(ui, i18n::t(lang, "open_log"), None).clicked()
                    && !self.fixture
                    && let Err(e) =
                        platform::open(platform::local_data().join("Cat Codex Switcher"))
                {
                    self.local_error = Some(e.to_string());
                }
                win::section(ui, i18n::t(lang, "updates"));
                win::toggle(
                    ui,
                    &mut s.automatically_check_updates,
                    i18n::t(lang, "auto_update"),
                );
                let status = if self.state.settings.update_repository.is_empty() {
                    "update_unconfigured"
                } else {
                    &self.state.update_status
                };
                win::muted(ui, i18n::t(lang, status), 10.5);
                ui.add_space(8.0);
                egui::CollapsingHeader::new(i18n::t(lang, "repository")).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.repository)
                                .hint_text("owner/repository")
                                .desired_width(ui.available_width() - 100.0),
                        );
                        if win::button(ui, i18n::t(lang, "save"), None).clicked() {
                            s.update_repository = self.repository.trim().into();
                        }
                    });
                });
                ui.add_space(12.0);
                win::rule(ui);
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!(
                            "{} {}",
                            i18n::t(lang, "version"),
                            env!("CARGO_PKG_VERSION")
                        ))
                        .size(12.0),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if let Some(url) = self.state.update_url.clone() {
                            if win::button(ui, i18n::t(lang, "download"), None).clicked()
                                && let Err(e) = platform::browser(&url)
                            {
                                self.local_error = Some(e.to_string());
                            }
                        } else if ui
                            .add_enabled_ui(self.state.update_status != "checking", |ui| {
                                win::button(ui, i18n::t(lang, "check"), None)
                            })
                            .inner
                            .clicked()
                        {
                            self.send(Command::CheckUpdates);
                        }
                    });
                });
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
        if !self.state.rows.iter().any(|r| r.profile.id == id) {
            self.page = Page::Accounts;
            return;
        }
        egui::Frame::new()
            .inner_margin(egui::Margin {
                left: 20,
                right: 20,
                top: 0,
                bottom: 20,
            })
            .show(ui, |ui| {
                win::muted(
                    ui,
                    i18n::t(lang, if remove { "remove_hint" } else { "switch_hint" }),
                    13.0,
                );
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let p = Palette::current(ui);
                        let confirm = ui
                            .scope(|ui| {
                                ui.visuals_mut().widgets.inactive.bg_stroke =
                                    egui::Stroke::new(1.0_f32, p.accent);
                                ui.add_enabled_ui(!self.state.busy, |ui| {
                                    win::button(
                                        ui,
                                        i18n::t(
                                            lang,
                                            if remove { "remove" } else { "switch_action" },
                                        ),
                                        None,
                                    )
                                })
                                .inner
                            })
                            .inner;
                        let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                        if (confirm.clicked() || enter) && !self.state.busy {
                            self.send(if remove {
                                Command::Remove(id)
                            } else {
                                Command::Switch(id)
                            });
                            self.page = if remove { Page::Manage } else { Page::Accounts };
                        }
                        if win::button(ui, i18n::t(lang, "cancel"), None).clicked() {
                            self.page = if remove { Page::Manage } else { Page::Accounts };
                        }
                    });
                });
            });
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
        win::configure(ctx);
        let maximum = if self.fixture {
            900.0
        } else {
            ctx.input(|i| {
                i.viewport()
                    .monitor_size
                    .map(|s| s.y - 90.0)
                    .unwrap_or(800.0)
            })
        };
        let panel = egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(Palette::current_from_context(ctx).surface))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        ui.set_width(420.0);
                        if let Some(error) = self
                            .local_error
                            .clone()
                            .or_else(|| self.state.error.clone())
                        {
                            egui::Frame::new()
                                .inner_margin(egui::Margin::symmetric(12, 8))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        win::muted(ui, error, 11.0);
                                        if win::button(ui, "×", None).clicked() {
                                            self.local_error = None;
                                            self.send(Command::Dismiss);
                                        }
                                    });
                                });
                            win::rule(ui);
                        }
                        self.header(ui, &lang);
                        match self.page {
                            Page::Accounts => self.accounts(ui, &lang, false),
                            Page::Manage => self.accounts(ui, &lang, true),
                            Page::Settings => {
                                ui.add_enabled_ui(!self.state.busy, |ui| self.settings(ui, &lang));
                            }
                            Page::Switch(id) => self.confirmation(ui, &lang, id, false),
                            Page::Remove(id) => self.confirmation(ui, &lang, id, true),
                        }
                    })
            });
        let height = panel.inner.content_size.y.clamp(130.0, maximum);
        if (height - self.fitted_height).abs() > 0.5 {
            self.fitted_height = height;
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(420.0, height)));
        }
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
fn initials(name: &str) -> String {
    let value: String = name
        .split_whitespace()
        .take(2)
        .filter_map(|s| s.chars().next())
        .flat_map(char::to_uppercase)
        .collect();
    if value.is_empty() { "?".into() } else { value }
}
fn reset_text(at: DateTime<Utc>, lang: &str, date: bool) -> String {
    format!(
        "{} {}",
        i18n::t(lang, "resets"),
        at.with_timezone(&Local)
            .format(if date { "%m-%d %H:%M" } else { "%H:%M" })
    )
}
fn usage_line(ui: &mut egui::Ui, title: &str, percent: i32, reset: Option<String>, lang: &str) {
    let p = Palette::current(ui);
    ui.add_space(5.0);
    let right = if let Some(reset) = reset.as_ref() {
        format!("{percent}%  {reset}")
    } else {
        format!("{percent}{}", i18n::t(lang, "left"))
    };
    let size = if reset.is_some() { 9.5 } else { 10.5 };
    let font = egui::FontId::proportional(size);
    let label_font = egui::FontId::proportional(10.5);
    let label_width = ui
        .painter()
        .layout_no_wrap(title.into(), label_font.clone(), p.muted)
        .size()
        .x;
    let right_width = ui
        .painter()
        .layout_no_wrap(right.clone(), font.clone(), p.muted)
        .size()
        .x;
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 14.0), egui::Sense::hover());
    ui.painter().text(
        rect.left_center(),
        egui::Align2::LEFT_CENTER,
        title,
        label_font,
        p.muted,
    );
    ui.painter().text(
        rect.right_center(),
        egui::Align2::RIGHT_CENTER,
        right,
        font,
        p.muted,
    );
    let track = egui::Rect::from_min_max(
        egui::pos2(rect.left() + label_width + 7.0, rect.center().y - 2.0),
        egui::pos2(
            (rect.right() - right_width - 7.0).max(rect.left() + label_width + 7.0),
            rect.center().y + 2.0,
        ),
    );
    ui.painter().rect_filled(track, 1, p.line);
    ui.painter().rect_filled(
        egui::Rect::from_min_size(
            track.min,
            egui::vec2(track.width() * percent as f32 / 100.0, 4.0),
        ),
        1,
        p.accent,
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::ProgressIndicator,
            true,
            format!("{title} {percent}%"),
        )
    });
}
fn account_info(ui: &mut egui::Ui, row: &crate::worker::Row, lang: &str) {
    let p = Palette::current(ui);
    let date = row
        .subscription
        .map(|(until, _)| until.with_timezone(&Local).format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| i18n::t(lang, "unknown").into());
    let left = format!("{}: {date}", i18n::t(lang, "subscription"));
    let right = row
        .usage
        .as_ref()
        .map(|u| {
            format!(
                "{}: {}",
                i18n::t(lang, "credits"),
                u.reset_credits_remaining
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| i18n::t(lang, "unknown").into())
            )
        })
        .unwrap_or_default();
    let font = egui::FontId::proportional(10.5);
    let right_width = ui
        .painter()
        .layout_no_wrap(right.clone(), font.clone(), p.muted)
        .size()
        .x;
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 14.0), egui::Sense::hover());
    let mut job = egui::text::LayoutJob::simple(
        left,
        font.clone(),
        p.muted,
        (rect.width() - right_width - 8.0).max(0.0),
    );
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    let galley = ui.painter().layout_job(job);
    ui.painter().galley(
        egui::pos2(rect.left(), rect.center().y - galley.size().y / 2.0),
        galley,
        p.muted,
    );
    ui.painter().text(
        rect.right_center(),
        egui::Align2::RIGHT_CENTER,
        right,
        font,
        p.muted,
    );
    if let Some((_, checked)) = row.subscription {
        response.on_hover_text(format!(
            "{} {}",
            i18n::t(lang, "snapshot_hint"),
            checked.with_timezone(&Local).format("%Y-%m-%d %H:%M")
        ));
    }
}
pub fn fixture(language: &str, empty: bool) -> Snapshot {
    let mut state = Snapshot {
        busy: false,
        ..Default::default()
    };
    state.settings.language = language.into();
    state.settings.shows_five_hour_usage = false;
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

#[cfg(test)]
mod layout_tests {
    use super::*;
    fn app() -> App {
        let state = fixture("en", false);
        App {
            shared: Arc::new(Mutex::new(state.clone())),
            state,
            worker: None,
            _instance: None,
            tray: None,
            page: Page::Accounts,
            repository: String::new(),
            import_path: String::new(),
            exit: false,
            initial_hide: false,
            fixture: true,
            screenshot: None,
            capture_frames: 0,
            quit_after_capture: false,
            local_error: None,
            fitted_height: 0.0,
        }
    }
    fn draw(app: &mut App, ctx: &egui::Context, events: Vec<egui::Event>) -> egui::Rect {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(420.0, 900.0),
            )),
            events,
            ..Default::default()
        };
        let mut used = egui::Rect::NOTHING;
        let _ = ctx.run(input, |ctx| {
            win::configure(ctx);
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ctx, |ui| {
                    let lang = i18n::language(&app.state.settings.language);
                    app.header(ui, &lang);
                    match app.page {
                        Page::Accounts => app.accounts(ui, &lang, false),
                        Page::Manage => app.accounts(ui, &lang, true),
                        Page::Settings => app.settings(ui, &lang),
                        Page::Switch(id) => app.confirmation(ui, &lang, id, false),
                        Page::Remove(id) => app.confirmation(ui, &lang, id, true),
                    }
                    used = ui.min_rect();
                });
        });
        used
    }
    fn click(app: &mut App, ctx: &egui::Context, pos: egui::Pos2) {
        draw(app, ctx, vec![egui::Event::PointerMoved(pos)]);
        for pressed in [true, false] {
            draw(
                app,
                ctx,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
    }
    #[test]
    fn original_navigation_and_entire_account_row_hit_targets_work() {
        let ctx = egui::Context::default();
        win::install_fonts(&ctx);
        let mut app = app();
        draw(&mut app, &ctx, vec![]);
        click(&mut app, &ctx, egui::pos2(300.0, 173.0));
        assert!(matches!(app.page,Page::Switch(id) if id==app.state.rows[1].profile.id));
        click(&mut app, &ctx, egui::pos2(36.0, 32.0));
        assert!(app.page == Page::Accounts);
        click(&mut app, &ctx, egui::pos2(380.0, 32.0));
        assert!(app.page == Page::Settings);
        click(&mut app, &ctx, egui::pos2(36.0, 32.0));
        click(&mut app, &ctx, egui::pos2(310.0, 32.0));
        assert!(app.page == Page::Manage);
        click(&mut app, &ctx, egui::pos2(363.0, 149.0));
        assert!(matches!(app.page,Page::Remove(id) if id==app.state.rows[1].profile.id));
        click(&mut app, &ctx, egui::pos2(36.0, 32.0));
        assert!(app.page == Page::Manage);
    }
    #[test]
    fn active_row_is_not_switchable_and_busy_rows_ignore_clicks() {
        let ctx = egui::Context::default();
        win::install_fonts(&ctx);
        let mut app = app();
        draw(&mut app, &ctx, vec![]);
        click(&mut app, &ctx, egui::pos2(200.0, 100.0));
        assert!(app.page == Page::Accounts);
        app.state.busy = true;
        click(&mut app, &ctx, egui::pos2(200.0, 173.0));
        assert!(app.page == Page::Accounts);
    }
    #[test]
    fn compact_screens_fit_the_source_window_width_in_all_languages() {
        let ctx = egui::Context::default();
        win::install_fonts(&ctx);
        let mut app = app();
        for lang in ["uk", "en", "ru"] {
            app.state.settings.language = lang.into();
            for page in [
                Page::Accounts,
                Page::Manage,
                Page::Settings,
                Page::Switch(app.state.rows[1].profile.id),
                Page::Remove(app.state.rows[1].profile.id),
            ] {
                app.page = page;
                for _ in 0..2 {
                    let rect = draw(&mut app, &ctx, vec![]);
                    assert!(
                        rect.width() <= 421.0,
                        "Page extends beyond the source's 420px window"
                    );
                }
            }
        }
    }
}
