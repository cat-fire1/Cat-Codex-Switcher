use cat_codex_switcher::platform;
use std::time::{Duration, Instant};
use windows::Win32::{Foundation::*, UI::WindowsAndMessaging::*};
fn find(pid: u32, class: Option<&str>) -> Option<HWND> {
    struct Search {
        pid: u32,
        class: Option<Vec<u16>>,
        found: Option<HWND>,
    }
    unsafe extern "system" fn visit(hwnd: HWND, data: LPARAM) -> BOOL {
        let search = unsafe { &mut *(data.0 as *mut Search) };
        let mut pid = 0;
        unsafe {
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
        }
        if pid == search.pid {
            let mut class = [0u16; 256];
            let len = unsafe { GetClassNameW(hwnd, &mut class) };
            let matches = search
                .class
                .as_ref()
                .is_none_or(|expected| expected[..expected.len() - 1] == class[..len as usize]);
            let mut title = [0u16; 256];
            let n = unsafe { GetWindowTextW(hwnd, &mut title) };
            if matches
                && (search.class.is_some()
                    || String::from_utf16_lossy(&title[..n as usize]) == "Cat Codex Switcher")
            {
                search.found = Some(hwnd);
                return false.into();
            }
        }
        true.into()
    }
    let mut search = Search {
        pid,
        class: class.map(platform::wide),
        found: None,
    };
    unsafe {
        let _ = EnumWindows(Some(visit), LPARAM((&mut search as *mut Search) as isize));
    }
    search.found
}
#[track_caller]
fn wait(mut check: impl FnMut() -> bool) {
    let start = Instant::now();
    while !check() {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "Fixture lifecycle timed out"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}
#[test]
fn close_to_tray_duplicate_activation_and_tray_click_reopen_fixture_window() {
    let exe = env!("CARGO_BIN_EXE_Cat-Codex-Switcher");
    let args = ["--ui-fixture", "--shell-fixture"];
    let mut child = platform::hidden_command(exe)
        .args(args)
        .arg("--background")
        .spawn()
        .unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        wait(|| find(child.id(), None).is_some());
        let window = find(child.id(), None).unwrap();
        std::thread::sleep(Duration::from_millis(800));
        assert!(
            unsafe { !IsWindowVisible(window).as_bool() },
            "Background launch must stay hidden"
        );
        let status = platform::hidden_command(exe).args(args).status().unwrap();
        assert!(status.success());
        wait(|| unsafe { IsWindowVisible(window).as_bool() });
        unsafe {
            PostMessageW(window, WM_CLOSE, WPARAM(0), LPARAM(0)).unwrap();
        }
        wait(|| unsafe { !IsWindowVisible(window).as_bool() });
        let mut duplicate = platform::hidden_command(exe).args(args).spawn().unwrap();
        wait(|| duplicate.try_wait().unwrap().is_some());
        assert!(duplicate.wait().unwrap().success());
        wait(|| unsafe { IsWindowVisible(window).as_bool() });
        unsafe {
            PostMessageW(window, WM_CLOSE, WPARAM(0), LPARAM(0)).unwrap();
        }
        wait(|| unsafe { !IsWindowVisible(window).as_bool() });
        let tray = find(child.id(), Some("tray_icon_app")).unwrap();
        unsafe {
            SendMessageW(tray, 6002, WPARAM(0), LPARAM(WM_LBUTTONUP as isize));
        }
        wait(|| unsafe { IsWindowVisible(window).as_bool() });
        unsafe {
            PostMessageW(window, WM_CLOSE, WPARAM(0), LPARAM(0)).unwrap();
        }
        wait(|| unsafe { !IsWindowVisible(window).as_bool() });
        // Cargo.lock pins muda 0.15: the first menu has ID 1000, Open 1001 and Quit 1002.
        unsafe {
            SendMessageW(tray, WM_COMMAND, WPARAM(1002), LPARAM(0));
        }
        wait(|| child.try_wait().unwrap().is_some());
        assert!(child.wait().unwrap().success());
    }));
    if let Some(hwnd) = find(child.id(), None) {
        let thread = unsafe { GetWindowThreadProcessId(hwnd, None) };
        unsafe {
            let _ = PostThreadMessageW(thread, WM_QUIT, WPARAM(0), LPARAM(0));
        }
    }
    let started = Instant::now();
    while child.try_wait().unwrap().is_none() && started.elapsed() < Duration::from_secs(3) {
        std::thread::sleep(Duration::from_millis(20));
    }
    let _ = child.kill();
    let _ = child.wait();
    result.unwrap();
}
