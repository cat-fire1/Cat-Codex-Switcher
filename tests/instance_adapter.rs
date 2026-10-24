use cat_codex_switcher::platform::{self, Instance, OriginalGuard};
use std::time::{Duration, Instant};
#[test]
fn duplicate_process_signals_existing_instance_without_opening_accounts() {
    let instance = Instance::acquire()
        .unwrap()
        .expect("Close Cat Codex Switcher before running singleton integration tests");
    let status = platform::hidden_command(env!("CARGO_BIN_EXE_rpc-fixture"))
        .arg("--activate-fixture")
        .status()
        .unwrap();
    assert!(status.success());
    let start = Instant::now();
    while !instance.activated() {
        assert!(start.elapsed() < Duration::from_secs(3));
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(!instance.activated());
}
#[test]
fn original_mutex_prevents_concurrent_access_to_source_data() {
    let temp = tempfile::tempdir().unwrap();
    let guard = OriginalGuard::acquire(temp.path()).unwrap();
    let status = platform::hidden_command(env!("CARGO_BIN_EXE_rpc-fixture"))
        .arg("--original-guard-fixture")
        .arg(temp.path())
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(42));
    drop(guard);
    let status = platform::hidden_command(env!("CARGO_BIN_EXE_rpc-fixture"))
        .arg("--original-guard-fixture")
        .arg(temp.path())
        .status()
        .unwrap();
    assert!(status.success());
}
