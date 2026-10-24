use cat_codex_switcher::rpc::Session;
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
fn run(mode: &str, cancelled: bool) -> anyhow::Result<Option<i32>> {
    let temp = tempfile::tempdir()?;
    std::fs::write(temp.path().join("fixture-mode"), mode)?;
    let cancel = Arc::new(AtomicBool::new(false));
    let token = cancel.clone();
    let thread = if cancelled {
        Some(std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(150));
            token.store(true, Ordering::SeqCst);
        }))
    } else {
        None
    };
    let result = tokio::runtime::Runtime::new()?.block_on(async {
        let mut s = Session::start(
            Path::new(env!("CARGO_BIN_EXE_rpc-fixture")),
            temp.path(),
            cancel,
        )
        .await?;
        let result = s.usage().await.map(|u| Some(u.remaining_percent));
        s.stop().await;
        result
    });
    if let Some(t) = thread {
        t.join().unwrap();
    }
    result
}
#[test]
fn real_child_rpc_reads_usage_and_ignores_unrelated_notifications() {
    assert_eq!(run("early_notification", false).unwrap(), Some(70));
}
#[test]
fn malformed_transport_is_rejected() {
    assert!(
        run("malformed", false)
            .unwrap_err()
            .to_string()
            .contains("Malformed")
    );
}
#[test]
fn child_exit_is_reported() {
    assert!(
        run("closed", false)
            .unwrap_err()
            .to_string()
            .contains("closed")
    );
}
#[test]
fn remote_error_text_never_exposes_secrets() {
    let e = run("remote_error", false).unwrap_err().to_string();
    assert!(e.contains("-123"));
    assert!(!e.contains("SECRET_TOKEN"));
}
#[test]
fn cancellation_interrupts_blocked_rpc() {
    let start = Instant::now();
    assert!(
        run("timeout", true)
            .unwrap_err()
            .to_string()
            .contains("cancelled")
    );
    assert!(start.elapsed() < Duration::from_secs(5));
}
#[test]
fn timeout_terminates_child_without_hanging() {
    let start = Instant::now();
    assert!(
        run("timeout", false)
            .unwrap_err()
            .to_string()
            .contains("timed out")
    );
    assert!(start.elapsed() < Duration::from_secs(25));
}
fn login(mode: &str) -> anyhow::Result<String> {
    let temp = tempfile::tempdir()?;
    std::fs::write(temp.path().join("fixture-mode"), mode)?;
    let cancel = Arc::new(AtomicBool::new(false));
    let token = cancel.clone();
    let thread = if mode == "login_cancel" {
        Some(std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            token.store(true, Ordering::SeqCst);
        }))
    } else {
        None
    };
    let result = tokio::runtime::Runtime::new()?.block_on(async {
        let mut s = Session::start(
            Path::new(env!("CARGO_BIN_EXE_rpc-fixture")),
            temp.path(),
            cancel,
        )
        .await?;
        let result = s
            .login_with(temp.path(), |url| {
                assert_eq!(url, "https://example.invalid/signin");
                Ok(())
            })
            .await;
        s.stop().await;
        result.map(|i| i.account_id.unwrap())
    });
    if let Some(t) = thread {
        t.join().unwrap();
    }
    result
}
#[test]
fn login_handles_completion_before_start_response() {
    assert_eq!(login("login_early").unwrap(), "fixture-account");
}
#[test]
fn login_rejects_rpc_and_credential_identity_mismatch() {
    assert!(
        login("login_mismatch")
            .unwrap_err()
            .to_string()
            .contains("mismatch")
    );
}
#[test]
fn pending_login_can_be_cancelled() {
    assert!(
        login("login_cancel")
            .unwrap_err()
            .to_string()
            .contains("cancelled")
    );
}
