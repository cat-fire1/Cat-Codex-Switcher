use crate::{i18n, platform, worker::Snapshot};
use anyhow::Result;
use std::sync::{Arc, Mutex};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc,
};
pub struct Shell {
    stop: Arc<AtomicBool>,
    opened: Arc<AtomicBool>,
    quitting: Arc<AtomicBool>,
    join: Option<std::thread::JoinHandle<()>>,
}
impl Shell {
    pub fn start(
        ctx: egui::Context,
        window: isize,
        activation: Option<isize>,
        state: Arc<Mutex<Snapshot>>,
    ) -> Result<Self> {
        let stop = Arc::new(AtomicBool::new(false));
        let done = stop.clone();
        let opened = Arc::new(AtomicBool::new(false));
        let signal = opened.clone();
        let quitting = Arc::new(AtomicBool::new(false));
        let exit_signal = quitting.clone();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let join = std::thread::spawn(move || {
            use windows::Win32::{Foundation::*, System::Threading::*, UI::WindowsAndMessaging::*};
            let mut tray = match Tray::new(ctx.clone()) {
                Ok(t) => t,
                Err(e) => {
                    let _ = ready_tx.send(Err(e.to_string()));
                    return;
                }
            };
            let hwnd = HWND(window as *mut std::ffi::c_void);
            let _ = ready_tx.send(Ok(()));
            while !done.load(Ordering::SeqCst) {
                unsafe {
                    let mut msg = MSG::default();
                    while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                        let _ = TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                }
                if let Ok(snapshot) = state.lock() {
                    let lang = i18n::language(&snapshot.settings.language);
                    let _ = tray.update(&snapshot, &lang);
                }
                let mut actions = tray.events();
                if activation.is_some_and(|h| unsafe {
                    WaitForSingleObject(HANDLE(h as *mut std::ffi::c_void), 0) == WAIT_OBJECT_0
                }) {
                    actions.push(Action::Open);
                }
                for action in actions {
                    match action {
                        Action::Open => {
                            signal.store(true, Ordering::SeqCst);
                            unsafe {
                                let _ = ShowWindowAsync(hwnd, SW_RESTORE);
                                let _ = SetForegroundWindow(hwnd);
                            }
                            ctx.request_repaint();
                        }
                        Action::Quit => {
                            if state.lock().is_ok_and(|s| !s.busy && !s.adding) {
                                exit_signal.store(true, Ordering::SeqCst);
                                unsafe {
                                    let _ = ShowWindowAsync(hwnd, SW_SHOWNOACTIVATE);
                                }
                                ctx.request_repaint();
                            }
                        }
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        });
        match ready_rx.recv_timeout(std::time::Duration::from_secs(5)) {
            Ok(Ok(())) => Ok(Self {
                stop,
                opened,
                quitting,
                join: Some(join),
            }),
            other => {
                stop.store(true, Ordering::SeqCst);
                let _ = join.join();
                anyhow::bail!("Windows tray initialization failed: {other:?}");
            }
        }
    }
    pub fn take_open(&self) -> bool {
        self.opened.swap(false, Ordering::SeqCst)
    }
    pub fn take_quit(&self) -> bool {
        self.quitting.swap(false, Ordering::SeqCst)
    }
}
impl Drop for Shell {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}
use tray_icon::{
    TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuEvent, MenuItem},
};
pub struct Tray {
    icon: TrayIcon,
    quit: MenuItem,
    actions: Arc<Mutex<Vec<Action>>>,
    last: Option<(Option<i32>, bool, String)>,
}
pub enum Action {
    Open,
    Quit,
}
impl Tray {
    pub fn new(ctx: egui::Context) -> Result<Self> {
        let menu = Menu::new();
        let open = MenuItem::new("Cat Codex Switcher", true, None);
        let quit = MenuItem::new("Quit", true, None);
        menu.append(&open)?;
        menu.append(&quit)?;
        let icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .with_tooltip("Cat Codex Switcher")
            .with_icon(icon(None, platform::taskbar_dark())?)
            .build()?;
        let actions = Arc::new(Mutex::new(Vec::new()));
        let pending = actions.clone();
        let c = ctx.clone();
        TrayIconEvent::set_event_handler(Some(move |event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: tray_icon::MouseButton::Left,
                    button_state: tray_icon::MouseButtonState::Up,
                    ..
                }
            ) {
                if let Ok(mut a) = pending.lock() {
                    a.push(Action::Open);
                }
                c.request_repaint();
            }
        }));
        let pending = actions.clone();
        let open_id = open.id().clone();
        let quit_id = quit.id().clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            if let Ok(mut a) = pending.lock() {
                if event.id == quit_id {
                    a.push(Action::Quit);
                } else if event.id == open_id {
                    a.push(Action::Open);
                }
            }
            ctx.request_repaint();
        }));
        Ok(Self {
            icon,
            quit,
            actions,
            last: None,
        })
    }
    pub fn update(&mut self, s: &Snapshot, lang: &str) -> Result<()> {
        self.quit.set_text(i18n::t(lang, "quit"));
        self.quit.set_enabled(!s.busy && !s.adding);
        let active = s.rows.iter().find(|r| Some(r.profile.id) == s.active);
        let percentage = if s.settings.shows_menu_bar_percentage {
            active.and_then(|r| r.usage.as_ref()).map(|u| {
                if s.settings.status_bar_usage_window == "fiveHour" {
                    u.five_hour_remaining_percent.unwrap_or(u.remaining_percent)
                } else {
                    u.remaining_percent
                }
            })
        } else {
            None
        };
        let tooltip = if let Some(row) = active {
            format!(
                "{}{}",
                row.profile.title(),
                percentage.map(|p| format!(" — {p}%")).unwrap_or_default()
            )
        } else {
            "Cat Codex Switcher".into()
        };
        let tooltip: String = tooltip.chars().take(100).collect();
        let key = (percentage, platform::taskbar_dark(), tooltip);
        if self.last.as_ref() != Some(&key) {
            self.icon.set_icon(Some(icon(key.0, key.1)?))?;
            self.icon.set_tooltip(Some(&key.2))?;
            self.last = Some(key);
        }
        Ok(())
    }
    pub fn events(&self) -> Vec<Action> {
        self.actions
            .lock()
            .map(|mut a| std::mem::take(&mut *a))
            .unwrap_or_default()
    }
}
fn icon(percent: Option<i32>, dark: bool) -> Result<tray_icon::Icon> {
    let Some(percent) = percent else {
        let bytes: &[u8] = if dark {
            include_bytes!("../assets/account-switcher-logo-white.png")
        } else {
            include_bytes!("../assets/account-switcher-logo.png")
        };
        let image = image::load_from_memory(bytes)?
            .resize_exact(32, 32, image::imageops::FilterType::Lanczos3)
            .to_rgba8();
        return Ok(tray_icon::Icon::from_rgba(image.into_raw(), 32, 32)?);
    };
    Ok(tray_icon::Icon::from_rgba(
        number_rgba(percent, dark),
        32,
        32,
    )?)
}
fn number_rgba(percent: i32, dark: bool) -> Vec<u8> {
    let digits = [
        [7, 5, 5, 5, 7],
        [2, 6, 2, 2, 7],
        [7, 1, 7, 4, 7],
        [7, 1, 7, 1, 7],
        [5, 5, 7, 1, 1],
        [7, 4, 7, 1, 7],
        [7, 4, 7, 5, 7],
        [7, 1, 1, 1, 1],
        [7, 5, 7, 5, 7],
        [7, 5, 7, 1, 7],
    ];
    let text = percent.clamp(0, 100).to_string();
    let scale = match text.len() {
        1 => 5,
        2 => 4,
        _ => 3,
    };
    let gap = 2;
    let width = text.len() * 3 * scale + (text.len() - 1) * gap;
    let left = (32 - width) / 2;
    let top = (32 - 5 * scale) / 2;
    let mut rgba = vec![0u8; 32 * 32 * 4];
    let ink = if dark { 255 } else { 0 };
    for (index, digit) in text.bytes().enumerate() {
        for (y, row) in digits[(digit - b'0') as usize].iter().enumerate() {
            for x in 0..3 {
                if row & (1 << (2 - x)) != 0 {
                    for dy in 0..scale {
                        for dx in 0..scale {
                            let i = ((top + y * scale + dy) * 32
                                + left
                                + index * (3 * scale + gap)
                                + x * scale
                                + dx)
                                * 4;
                            rgba[i..i + 4].copy_from_slice(&[ink, ink, ink, 255]);
                        }
                    }
                }
            }
        }
    }
    rgba
}
