#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use cat_codex_switcher::{experiment, model, platform, rpc, store, updates};
mod i18n;
mod tray;
mod ui;
mod windows_style;
mod worker;

fn main() {
    if let Err(e) = run() {
        let message = format!("{e:#}");
        eprintln!("{message}");
        unsafe {
            windows::Win32::UI::WindowsAndMessaging::MessageBoxW(
                None,
                windows::core::PCWSTR(platform::wide(&message).as_ptr()),
                windows::core::w!("Cat Codex Switcher"),
                windows::Win32::UI::WindowsAndMessaging::MB_ICONERROR,
            );
        }
        std::process::exit(1);
    }
}
fn run() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let background = args.iter().any(|a| a == "--background");
    let Some(instance) = platform::Instance::acquire()? else {
        return Ok(());
    };
    let image =
        image::load_from_memory(include_bytes!("../assets/account-switcher-logo.png"))?.to_rgba8();
    let icon = egui::IconData {
        width: image.width(),
        height: image.height(),
        rgba: image.into_raw(),
    };
    let native = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Cat Codex Switcher")
            .with_inner_size([520.0, 440.0])
            .with_clamp_size_to_monitor_size(false)
            .with_resizable(false)
            .with_maximize_button(false)
            .with_icon(icon),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    eframe::run_native(
        "Cat Codex Switcher",
        native,
        Box::new(move |cc| Ok(Box::new(ui::App::new(cc, Some(instance), background)))),
    )
    .map_err(|e| anyhow::anyhow!("Window initialization failed: {e}"))
}
