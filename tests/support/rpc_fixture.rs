use serde_json::{Value, json};
use std::io::{self, BufRead, Write};
fn main() {
    let args: Vec<_> = std::env::args().collect();
    if args.iter().any(|a| a == "--activate-fixture") {
        assert!(
            cat_codex_switcher::platform::Instance::acquire()
                .unwrap()
                .is_none()
        );
        return;
    }
    if args.get(1).is_some_and(|a| a == "--original-guard-fixture") {
        let result =
            cat_codex_switcher::platform::OriginalGuard::acquire(std::path::Path::new(&args[2]));
        std::process::exit(if result.is_err() { 42 } else { 0 });
    }
    if !std::env::args().any(|a| a == "app-server") {
        desktop_window();
        return;
    }
    let home =
        std::path::PathBuf::from(std::env::var_os("CODEX_HOME").expect("isolated fixture home"));
    let mode = std::fs::read_to_string(home.join("fixture-mode")).expect("fixture marker required");
    for line in io::stdin().lock().lines() {
        let v: Value = serde_json::from_str(&line.expect("stdin")).expect("request JSON");
        if v.get("id").is_none() {
            continue;
        }
        let method = v["method"].as_str().unwrap_or("");
        if method == "initialize" {
            if mode == "malformed" {
                println!("not json");
                io::stdout().flush().unwrap();
                continue;
            }
            if mode == "closed" {
                return;
            }
            if mode == "timeout" {
                std::thread::sleep(std::time::Duration::from_secs(60));
                return;
            }
        }
        if mode == "remote_error" {
            println!(
                "{}",
                json!({"id":v["id"],"error":{"code":-123,"message":"SECRET_TOKEN_MUST_NOT_LEAK"}})
            );
        } else {
            if mode == "early_notification" {
                println!(
                    "{}",
                    json!({"method":"account/login/completed","params":{"success":true}})
                );
            }
            if method == "account/login/start" {
                std::fs::write(
                    home.join("auth.json"),
                    br#"{"tokens":{"account_id":"fixture-account"}}"#,
                )
                .unwrap();
                if mode != "login_cancel" {
                    println!(
                        "{}",
                        json!({"method":"account/login/completed","params":{"success":true}})
                    );
                }
            }
            let result = if method == "account/login/start" {
                json!({"authUrl":"https://example.invalid/signin"})
            } else if method == "account/read" {
                json!({"account":{"accountId":if mode=="login_mismatch" { "wrong-account" } else { "fixture-account" }}})
            } else if method == "account/rateLimits/read" {
                json!({"rateLimits":{"secondary":{"usedPercent":30,"windowDurationMins":10080,"resetsAt":1796000000}}})
            } else {
                json!({})
            };
            println!("{}", json!({"id":v["id"],"result":result}));
        }
        io::stdout().flush().unwrap();
    }
}
fn desktop_window() {
    use windows::{
        Win32::{Foundation::*, UI::WindowsAndMessaging::*},
        core::w,
    };
    unsafe extern "system" fn proc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if message == WM_DESTROY {
            unsafe {
                PostQuitMessage(0);
            }
            return LRESULT(0);
        }
        unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
    }
    unsafe {
        let class = WNDCLASSW {
            lpfnWndProc: Some(proc),
            lpszClassName: w!("CatSwitcherIsolatedDesktopFixture"),
            ..Default::default()
        };
        RegisterClassW(&class);
        let _window = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class.lpszClassName,
            w!("Cat Switcher isolated test window"),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            -32000,
            -32000,
            200,
            120,
            None,
            None,
            None,
            None,
        )
        .expect("fixture window");
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
