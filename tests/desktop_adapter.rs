use cat_codex_switcher::platform::{self, Desktop};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
fn wait_ready() {
    use windows::{
        Win32::UI::WindowsAndMessaging::{FindWindowW, IsWindowVisible},
        core::w,
    };
    let started = Instant::now();
    loop {
        if let Ok(hwnd) = unsafe { FindWindowW(w!("CatSwitcherIsolatedDesktopFixture"), None) }
            && unsafe { IsWindowVisible(hwnd).as_bool() }
        {
            return;
        }
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "Fixture window did not become ready"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}
#[test]
fn exact_path_matching_close_and_reopen_use_only_fixture_process() {
    let executable = PathBuf::from(env!("CARGO_BIN_EXE_rpc-fixture"));
    let desktop = Desktop {
        executable: executable.clone(),
        app_id: None,
    };
    let mut child = platform::hidden_command(&executable).spawn().unwrap();
    let result = std::panic::catch_unwind(|| {
        let start = Instant::now();
        while desktop.processes().unwrap().is_empty() {
            assert!(start.elapsed() < Duration::from_secs(5));
            std::thread::sleep(Duration::from_millis(20));
        }
        wait_ready();
        assert!(desktop.processes().unwrap().contains(&child.id()));
        let fake = Desktop {
            executable: executable.parent().unwrap().join("not-this-fixture.exe"),
            app_id: None,
        };
        assert!(fake.processes().unwrap().is_empty());
        desktop.close().unwrap();
        assert!(desktop.processes().unwrap().is_empty());
        for _ in 0..5 {
            desktop.reopen().unwrap();
            wait_ready();
            desktop.close().unwrap();
            assert!(desktop.processes().unwrap().is_empty());
        }
    });
    let _ = child.kill();
    let _ = child.wait();
    if result.is_err() {
        let _ = desktop.close();
    }
    result.unwrap();
}
