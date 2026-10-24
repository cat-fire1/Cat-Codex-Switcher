#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use cat_codex_switcher::{experiment, model, platform, rpc, store, updates};
mod i18n;
mod tray;
mod ui;
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
    if args.iter().any(|a| a == "--self-test") {
        return self_test();
    }
    let value = |flag: &str| args.windows(2).find(|p| p[0] == flag).map(|p| p[1].clone());
    let fixture = args.iter().any(|a| a == "--ui-fixture");
    let shell_fixture = fixture && args.iter().any(|a| a == "--shell-fixture");
    let instance = if fixture && !shell_fixture {
        None
    } else {
        let Some(instance) = platform::Instance::acquire()? else {
            return Ok(());
        };
        Some(instance)
    };
    let options = ui::Options {
        fixture,
        shell_fixture,
        empty: args.iter().any(|a| a == "--empty"),
        language: value("--language").unwrap_or_else(|| "uk".into()),
        theme: value("--theme"),
        scale: value("--scale").and_then(|s| s.parse().ok()),
        page: value("--page").unwrap_or_else(|| "accounts".into()),
        screenshot: if fixture {
            value("--screenshot").map(Into::into)
        } else {
            None
        },
        quit_after_capture: args.iter().any(|a| a == "--quit-after-capture"),
        background: args.iter().any(|a| a == "--background"),
    };
    let image = image::load_from_memory(include_bytes!("../assets/account-switcher-logo.png"))?
        .to_rgba8();
    let icon = egui::IconData {
        width: image.width(),
        height: image.height(),
        rgba: image.into_raw(),
    };
    let fixture_scale = if fixture {
        options.scale.unwrap_or(1.0)
    } else {
        1.0
    };
    let native = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Cat Codex Switcher")
            .with_inner_size([440.0 * fixture_scale, 600.0 * fixture_scale])
            .with_clamp_size_to_monitor_size(false)
            .with_min_inner_size([400.0, 350.0])
            .with_icon(icon),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    eframe::run_native(
        "Cat Codex Switcher",
        native,
        Box::new(move |cc| Ok(Box::new(ui::App::new(cc, instance, options)))),
    )
    .map_err(|e| anyhow::anyhow!("Window initialization failed: {e}"))
}
fn self_test() -> anyhow::Result<()> {
    use anyhow::ensure;
    let root =
        std::env::temp_dir().join(format!("cat-switcher-self-test-{}", uuid::Uuid::new_v4()));
    platform::secure_dir(&root)?;
    let result = (|| -> anyhow::Result<()> {
        let active = root.join("active");
        platform::secure_dir(&active)?;
        let mut s = store::Store::load(root.join("data"), active)?;
        let mut settings = s.settings.clone();
        settings.shows_five_hour_usage = true;
        s.save_settings(settings)?;
        let auth = serde_json::to_vec(
            &serde_json::json!({"tokens":{"account_id":"self-test-fictional"}}),
        )?;
        platform::atomic_write(&s.active_file(), &auth)?;
        s.sync_current()?;
        let id = s
            .registry
            .active_account_id
            .ok_or_else(|| anyhow::anyhow!("Self-test failed to register fixture"))?;
        ensure!(
            platform::read(&s.credential(id))? == auth,
            "Credential copy mismatch"
        );
        ensure!(s.remove(id).is_err(), "Active account removal was allowed");
        let s = store::Store::load(s.root.clone(), s.active.clone())?;
        ensure!(
            s.settings.shows_five_hour_usage && s.registry.active_account_id == Some(id),
            "Persistence failed"
        );
        ensure!(
            updates::repository("")?.is_none(),
            "Empty update source failed"
        );
        Ok(())
    })();
    let cleanup = platform::remove_tree(&root, &std::env::temp_dir());
    let text=match &result { Ok(())=>"PASS: isolated storage, credential registration, atomic writes, protected active account and restart persistence".into(),Err(e)=>format!("FAIL: {e:#}") };
    if let Some(output) = std::env::var_os("CAT_SWITCHER_SELF_TEST_RESULT") {
        std::fs::write(output, &text)?;
    }
    println!("{text}");
    result?;
    cleanup?;
    Ok(())
}
