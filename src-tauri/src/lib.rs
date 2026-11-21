pub mod experiment;
pub mod model;
pub mod platform;
pub mod rpc;
pub mod store;
mod tray;
pub mod updates;
pub mod worker;

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tauri::{
    Emitter, Manager,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

pub struct AppState {
    pub worker: Arc<worker::Worker>,
    pub shared: Arc<std::sync::Mutex<worker::Snapshot>>,
    shutdown_complete: Arc<AtomicBool>,
}

fn show_main(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn update_tray(app: &tauri::AppHandle, shared: &std::sync::Mutex<worker::Snapshot>) {
    let snapshot = match shared.lock() {
        Ok(s) => s.clone(),
        Err(_) => return,
    };
    if let Some(tray) = app.tray_by_id("main-tray") {
        let _ = tray.set_tooltip(Some(tray::tooltip(&snapshot)));
        let icon = if snapshot.settings.shows_menu_bar_percentage {
            tray::percentage(&snapshot).map(|(value, stale)| tray::number_icon(value, stale))
        } else {
            None
        };
        let _ = tray.set_icon(icon.or_else(|| app.default_window_icon().cloned()));
    }
}

#[tauri::command]
fn get_snapshot(state: tauri::State<'_, AppState>) -> Result<worker::Snapshot, String> {
    state
        .shared
        .lock()
        .map(|s| s.clone())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn refresh(state: tauri::State<'_, AppState>) -> Result<(), String> {
    state
        .worker
        .send(worker::Command::Refresh)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn add_account(state: tauri::State<'_, AppState>) -> Result<(), String> {
    state
        .worker
        .send(worker::Command::Add)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn cancel_login(state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.worker.cancel_login().map_err(|e| e.to_string())
}

#[tauri::command]
fn register_current(state: tauri::State<'_, AppState>) -> Result<(), String> {
    state
        .worker
        .send(worker::Command::Register)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn switch_account(id: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let uid = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    state
        .worker
        .send(worker::Command::Switch(uid))
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn remove_account(id: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let uid = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    state
        .worker
        .send(worker::Command::Remove(uid))
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn save_settings(
    settings: model::Settings,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    state
        .worker
        .send(worker::Command::Settings(settings))
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn preview_import(path: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let pb = std::path::PathBuf::from(path);
    state
        .worker
        .send(worker::Command::Preview(pb))
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn confirm_import(path: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let pb = std::path::PathBuf::from(path);
    state
        .worker
        .send(worker::Command::Import(pb))
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn check_updates(state: tauri::State<'_, AppState>) -> Result<(), String> {
    state
        .worker
        .send(worker::Command::CheckUpdates)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn dismiss_error(state: tauri::State<'_, AppState>) -> Result<(), String> {
    state
        .worker
        .send(worker::Command::Dismiss)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn select_folder() -> Result<Option<String>, String> {
    let folder = rfd::FileDialog::new()
        .set_directory(platform::local_data())
        .pick_folder();
    Ok(folder.map(|p| p.display().to_string()))
}

#[tauri::command]
fn open_external_url(url: String) -> Result<(), String> {
    updates::validate_release_url(&url)
        .and_then(|()| platform::browser(&url))
        .map_err(|e| e.to_string())
}

pub fn run() {
    let _instance = match platform::Instance::acquire() {
        Ok(Some(inst)) => inst,
        Ok(None) => return,
        Err(e) => {
            eprintln!("Failed to acquire instance: {e:#}");
            return;
        }
    };

    let shared = Arc::new(std::sync::Mutex::new(worker::Snapshot::default()));
    let shared_for_worker = shared.clone();
    let shutdown_complete = Arc::new(AtomicBool::new(false));

    tauri::Builder::default()
        .setup(move |app| {
            let handle = app.handle().clone();
            let tray_state = shared.clone();
            let notifier = Arc::new(move || {
                let _ = handle.emit("state-update", ());
                // Never wait on the UI thread while the worker holds its dispatch gate.
                let app = handle.clone();
                let state = tray_state.clone();
                let _ = handle.run_on_main_thread(move || update_tray(&app, &state));
            });

            let root = platform::local_data().join("Cat Codex Switcher");
            let active = platform::active_home();
            let worker = worker::Worker::start(root, active, notifier, shared_for_worker);

            app.manage(AppState {
                worker: Arc::new(worker),
                shared: shared.clone(),
                shutdown_complete: shutdown_complete.clone(),
            });

            let activation_app = app.handle().clone();
            platform::watch_activation(shutdown_complete, move || {
                show_main(&activation_app);
                if let Some(state) = activation_app.try_state::<AppState>() {
                    let _ = state.worker.send(worker::Command::Refresh);
                }
            });

            // Set up tray icon & menu
            let quit_i = MenuItem::with_id(app, "quit", "Вихід", true, None::<&str>)?;
            let show_i = MenuItem::with_id(app, "show", "Відкрити", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_i, &quit_i])?;

            if let Some(icon) = app.default_window_icon() {
                let _tray = TrayIconBuilder::with_id("main-tray")
                    .icon(icon.clone())
                    .menu(&menu)
                    .show_menu_on_left_click(false)
                    .tooltip("Cat Codex Switcher")
                    .on_menu_event(move |app, event| match event.id().as_ref() {
                        "quit" => {
                            app.exit(0);
                        }
                        "show" => {
                            show_main(app);
                            let _ = app
                                .state::<AppState>()
                                .worker
                                .send(worker::Command::Refresh);
                        }
                        _ => {}
                    })
                    .on_tray_icon_event(|tray, event| {
                        if let TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        } = event
                        {
                            let app = tray.app_handle();
                            show_main(app);
                            let _ = app
                                .state::<AppState>()
                                .worker
                                .send(worker::Command::Refresh);
                        }
                    })
                    .build(app)?;
            }
            update_tray(app.handle(), &shared);

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            refresh,
            add_account,
            cancel_login,
            register_current,
            switch_account,
            remove_account,
            save_settings,
            preview_import,
            confirm_import,
            check_updates,
            dismiss_error,
            select_folder,
            open_external_url
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                let state = app.state::<AppState>();
                if state.shutdown_complete.load(Ordering::SeqCst) {
                    return;
                }
                api.prevent_exit();
                if state.worker.begin_shutdown() {
                    let worker = state.worker.clone();
                    let complete = state.shutdown_complete.clone();
                    let app = app.clone();
                    std::thread::spawn(move || {
                        worker.join();
                        complete.store(true, Ordering::SeqCst);
                        app.exit(0);
                    });
                } else {
                    show_main(app);
                }
            }
        });
}
