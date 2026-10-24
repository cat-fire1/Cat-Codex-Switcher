use crate::{
    experiment::{self, Experiment},
    model::*,
    platform,
    store::{Handoff, Store, handoff},
    updates,
};
use anyhow::{Result, bail};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

fn credential(id: &str, email: &str) -> Vec<u8> {
    let claims = json!({"email":email,"https://api.openai.com/auth":{"chatgpt_account_id":id,"chatgpt_plan_type":"plus","chatgpt_subscription_active_until":"2026-12-20T00:00:00Z","chatgpt_subscription_last_checked":"2026-10-24T00:00:00Z"}});
    serde_json::to_vec(&json!({"tokens":{"account_id":id,"id_token":format!("e30.{}.fixture",URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).unwrap()))}})).unwrap()
}
fn store(temp: &tempfile::TempDir, name: &str) -> Store {
    Store::load(temp.path().join(name), temp.path().join("active")).unwrap()
}
fn add(s: &mut Store, id: &str, email: &str) -> Profile {
    let bytes = credential(id, email);
    let p = identity(&bytes).unwrap().profile();
    platform::atomic_write(&s.credential(p.id), &bytes).unwrap();
    s.add(p.clone()).unwrap();
    p
}
fn usage() -> Usage {
    normalize(&json!({"rateLimits":{"primary":{"usedPercent":25.5,"windowDurationMins":300,"resetsAt":1795000000},"secondary":{"usedPercent":110,"windowDurationMins":10080,"resetsAt":1796000000}}})).unwrap()
}

#[test]
fn identity_requires_same_workspace_even_with_same_email() {
    let p = identity(&credential("workspace-a", "CAT@example.invalid"))
        .unwrap()
        .profile();
    assert!(
        identity(&credential("workspace-a", "cat@example.invalid"))
            .unwrap()
            .matches(&p)
    );
    assert!(
        !identity(&credential("workspace-b", "cat@example.invalid"))
            .unwrap()
            .matches(&p)
    );
    assert!(
        !Identity {
            account_id: None,
            email: Some("cat@example.invalid".into())
        }
        .matches(&p)
    );
}
#[test]
fn credentials_errors_do_not_disclose_input() {
    let e = identity(b"secret_token_NOT_JSON").unwrap_err().to_string();
    assert!(!e.contains("secret_token"));
    assert!(identity(br#"{"tokens":{}}"#).is_err());
}
#[test]
fn subscription_is_dated_identity_checked_snapshot() {
    let bytes = credential("a", "cat@example.invalid");
    let mut p = identity(&bytes).unwrap().profile();
    assert!(subscription(&bytes, &p).is_some());
    p.account_id = Some("b".into());
    assert!(subscription(&bytes, &p).is_none());
}
#[test]
fn exact_usage_windows_round_and_clamp() {
    let u = usage();
    assert_eq!(u.remaining_percent, 0);
    assert_eq!(u.five_hour_remaining_percent, Some(75));
    let v = json!({"rateLimitsByLimitId":{"codex":{"primary":{"usedPercent":-3,"windowDurationMins":299,"resetsAt":1795000000},"secondary":{"usedPercent":-20,"windowDurationMins":8640,"resetsAt":1796000000}}}});
    let u = normalize(&v).unwrap();
    assert_eq!(u.remaining_percent, 100);
    assert_eq!(u.five_hour_remaining_percent, None);
    assert!(normalize(&json!({"rateLimits":{"primary":{"usedPercent":20,"windowDurationMins":300,"resetsAt":1795000000}}})).is_err());
}
#[test]
fn modern_codex_bucket_takes_precedence() {
    let v = json!({"rateLimitsByLimitId":{"codex":{}},"rateLimits":{"secondary":{"usedPercent":20,"windowDurationMins":10080,"resetsAt":1795000000}}});
    assert!(normalize(&v).is_err());
}
#[test]
fn invalid_credit_details_fail_instead_of_inventing_values() {
    let mut v = json!({"rateLimits":{"secondary":{"usedPercent":20,"windowDurationMins":10080,"resetsAt":1795000000}},"rateLimitResetCredits":{"availableCount":-1,"credits":[{"id":"c","status":"available","resetType":"codexRateLimits","expiresAt":null}]}});
    let u = normalize(&v).unwrap();
    assert_eq!(u.reset_credits_remaining, None);
    assert_eq!(u.reset_credits.unwrap().len(), 1);
    v["rateLimitResetCredits"]["credits"][0]["expiresAt"] = json!("bad");
    assert!(normalize(&v).is_err());
}
#[test]
fn registration_persistence_and_active_removal_guard() {
    let temp = tempfile::tempdir().unwrap();
    let mut s = store(&temp, "data");
    let bytes = credential("a", "cat@example.invalid");
    platform::atomic_write(&s.active_file(), &bytes).unwrap();
    s.sync_current().unwrap();
    let id = s.registry.active_account_id.unwrap();
    assert_eq!(platform::read(&s.credential(id)).unwrap(), bytes);
    assert!(s.remove(id).is_err());
    s.cache(id, usage()).unwrap();
    let loaded = store(&temp, "data");
    assert_eq!(loaded.cache.entries.len(), 1);
    assert_eq!(loaded.registry.active_account_id, Some(id));
}
#[test]
fn save_current_refuses_identity_mismatch_without_overwrite() {
    let temp = tempfile::tempdir().unwrap();
    let mut s = store(&temp, "data");
    let p = add(&mut s, "a", "cat@example.invalid");
    platform::atomic_write(&s.active_file(), &credential("b", "other@example.invalid")).unwrap();
    assert!(s.save_current(Some(p.id)).is_err());
    assert_eq!(
        identity(&platform::read(&s.credential(p.id)).unwrap())
            .unwrap()
            .account_id
            .as_deref(),
        Some("a")
    );
}
#[test]
fn duplicate_login_and_missing_target_are_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let mut s = store(&temp, "data");
    let _p = add(&mut s, "a", "cat@example.invalid");
    let p = identity(&credential("a", "cat@example.invalid"))
        .unwrap()
        .profile();
    platform::atomic_write(&s.credential(p.id), &credential("a", "cat@example.invalid")).unwrap();
    assert!(s.add(p).is_err());
    assert!(s.activate(Uuid::new_v4()).is_err());
}
#[test]
fn restoration_covers_original_and_first_activation() {
    let temp = tempfile::tempdir().unwrap();
    let mut s = store(&temp, "data");
    let a = add(&mut s, "a", "a@example.invalid");
    let b = add(&mut s, "b", "b@example.invalid");
    s.activate(a.id).unwrap();
    s.commit(a.id).unwrap();
    s.activate(b.id).unwrap();
    s.verify(b.id).unwrap();
    s.restore(Some(a.id)).unwrap();
    s.verify(a.id).unwrap();
    s.restore(None).unwrap();
    assert!(!s.active_file().exists());
}
struct Fake {
    calls: Vec<&'static str>,
    fail: Option<&'static str>,
    restore_fails: bool,
}
impl Fake {
    fn step(&mut self, s: &'static str) -> Result<()> {
        self.calls.push(s);
        if self.fail == Some(s) {
            bail!("fixture failure");
        }
        Ok(())
    }
}
impl Handoff for Fake {
    fn close(&mut self) -> Result<()> {
        self.step("close")
    }
    fn save(&mut self) -> Result<()> {
        self.step("save")
    }
    fn activate(&mut self) -> Result<()> {
        self.step("activate")
    }
    fn verify(&mut self) -> Result<()> {
        self.step("verify")
    }
    fn commit(&mut self) -> Result<()> {
        self.step("commit")
    }
    fn restore(&mut self) -> Result<()> {
        self.calls.push("restore");
        if self.restore_fails {
            bail!("restore fixture failure");
        }
        Ok(())
    }
    fn reopen(&mut self) -> Result<()> {
        self.step("reopen")
    }
}
#[test]
fn switch_order_and_every_failure_boundary() {
    let stages = ["close", "save", "activate", "verify", "commit", "reopen"];
    let mut ok = Fake {
        calls: vec![],
        fail: None,
        restore_fails: false,
    };
    handoff(&mut ok).unwrap();
    assert_eq!(ok.calls, stages);
    for (index, stage) in stages.iter().enumerate() {
        let mut fake = Fake {
            calls: vec![],
            fail: Some(stage),
            restore_fails: false,
        };
        assert!(handoff(&mut fake).is_err());
        let mut expected = stages[..=index].to_vec();
        if ["verify", "commit"].contains(stage) {
            expected.push("restore");
        }
        assert_eq!(fake.calls, expected);
    }
}
#[test]
fn restoration_error_preserves_original_stage() {
    let mut fake = Fake {
        calls: vec![],
        fail: Some("verify"),
        restore_fails: true,
    };
    let e = handoff(&mut fake).unwrap_err().to_string();
    assert!(
        e.contains("verifyTargetIdentity")
            && e.contains("fixture failure")
            && e.contains("restore fixture failure")
    );
}
#[test]
fn import_preview_validates_every_profile_then_skips_duplicates() {
    let temp = tempfile::tempdir().unwrap();
    let mut source = store(&temp, "source");
    let a = add(&mut source, "a", "a@example.invalid");
    let _b = add(&mut source, "b", "b@example.invalid");
    source.cache(a.id, usage()).unwrap();
    let mut dest = store(&temp, "dest");
    add(&mut dest, "a", "a@example.invalid");
    let preview = dest.preview(&source.root).unwrap();
    assert_eq!(preview.duplicates, 1);
    assert_eq!(preview.titles, vec!["b@example.invalid"]);
    assert_eq!(dest.import(&source.root).unwrap(), 1);
    assert_eq!(dest.registry.accounts.len(), 2);
    assert_eq!(dest.import(&source.root).unwrap(), 0);
    assert!(!dest.active_file().exists());
    assert_eq!(dest.registry.active_account_id, None);
}
#[test]
fn invalid_import_changes_nothing_and_keeps_source() {
    let temp = tempfile::tempdir().unwrap();
    let mut source = store(&temp, "source");
    let p = add(&mut source, "a", "a@example.invalid");
    platform::atomic_write(
        &source.credential(p.id),
        &credential("wrong", "a@example.invalid"),
    )
    .unwrap();
    let mut dest = store(&temp, "dest");
    let before = std::fs::read(source.root.join("accounts.json")).unwrap();
    assert!(dest.preview(&source.root).is_err());
    assert!(dest.import(&source.root).is_err());
    assert!(dest.registry.accounts.is_empty());
    assert!(!dest.root.join("accounts").exists());
    assert_eq!(
        std::fs::read(source.root.join("accounts.json")).unwrap(),
        before
    );
}
#[test]
fn import_copies_cache_maps_language_and_preserves_local_integrations() {
    let temp = tempfile::tempdir().unwrap();
    let mut source = store(&temp, "source");
    let p = add(&mut source, "a", "a@example.invalid");
    source.cache(p.id, usage()).unwrap();
    let mut settings = source.settings.clone();
    settings.language = "simplifiedChinese".into();
    settings.launch_at_login = true;
    settings.reset_expiry_experiment_enabled = true;
    settings.update_repository = "upstream/original".into();
    source.save_settings(settings).unwrap();
    let mut dest = store(&temp, "dest");
    let mut settings = dest.settings.clone();
    settings.update_repository = "cat/personal".into();
    dest.save_settings(settings).unwrap();
    dest.import(&source.root).unwrap();
    assert_eq!(dest.settings.language, "uk");
    assert!(!dest.settings.launch_at_login);
    assert!(!dest.settings.reset_expiry_experiment_enabled);
    assert_eq!(dest.settings.update_repository, "cat/personal");
    assert_eq!(dest.cache.entries.len(), 1);
}
#[test]
fn import_does_not_select_source_active_flag_or_write_actual_login() {
    let temp = tempfile::tempdir().unwrap();
    let mut source = store(&temp, "source");
    let a = add(&mut source, "a", "a@example.invalid");
    let b = add(&mut source, "b", "b@example.invalid");
    source.commit(b.id).unwrap();
    let actual = credential("a", "a@example.invalid");
    platform::atomic_write(&source.active_file(), &actual).unwrap();
    let mut dest = store(&temp, "dest");
    dest.import(&source.root).unwrap();
    assert_eq!(dest.registry.active_account_id, Some(a.id));
    assert_eq!(platform::read(&dest.active_file()).unwrap(), actual);
}
#[test]
fn atomic_write_cleans_temporary_files_and_replaces() {
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("private/data.json");
    platform::atomic_write(&file, b"one").unwrap();
    platform::atomic_write(&file, b"two").unwrap();
    assert_eq!(platform::read(&file).unwrap(), b"two");
    assert_eq!(
        std::fs::read_dir(file.parent().unwrap()).unwrap().count(),
        1
    );
}
#[test]
fn future_format_and_corrupt_registry_are_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let s = store(&temp, "data");
    platform::atomic_write(
        &s.root.join("accounts.json"),
        br#"{"formatVersion":999,"activeAccountID":null,"accounts":[]}"#,
    )
    .unwrap();
    assert!(Store::load(s.root, s.active).is_err());
}
#[test]
fn junctions_are_rejected_before_any_write_or_deletion() {
    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("target");
    std::fs::create_dir(&target).unwrap();
    let link = temp.path().join("junction");
    let output = platform::hidden_command("cmd.exe")
        .args(["/c", "mklink", "/J"])
        .arg(&link)
        .arg(&target)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(platform::atomic_write(&link.join("auth.json"), b"secret").is_err());
    assert!(!target.join("auth.json").exists());
    assert!(platform::remove_tree(&link, temp.path()).is_err());
    std::fs::remove_dir(link).unwrap();
}
#[test]
fn expiry_checkpoints_persist_and_never_consume_resets() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("log.jsonl");
    let now = Utc::now();
    let p = Identity {
        account_id: Some("a".into()),
        email: None,
    }
    .profile();
    let mut u = usage();
    u.reset_credits = Some(vec![Credit {
        id: "credit".into(),
        status: "available".into(),
        reset_type: "codexRateLimits".into(),
        expires_at: Some(now + chrono::Duration::minutes(10)),
    }]);
    let mut exp = Experiment::default();
    let events = exp.observe(&p, Some(&u), None, now);
    assert_eq!(events.len(), 2);
    assert_eq!(events[1].decision.as_deref(), Some("would_attempt"));
    assert!(
        events
            .iter()
            .all(|e| !e.reset_attempted && e.mode == "dry_run")
    );
    experiment::append(&path, &events).unwrap();
    let mut restored = Experiment::load(&path).unwrap();
    assert_eq!(restored.observe(&p, Some(&u), None, now).len(), 1);
    let events = restored.observe(
        &p,
        None,
        Some("offline".into()),
        now + chrono::Duration::minutes(5),
    );
    assert_eq!(events[1].decision.as_deref(), Some("read_failed"));
}
#[test]
fn expired_checkpoints_are_marked_missed() {
    let now = Utc::now();
    let p = Identity {
        account_id: Some("a".into()),
        email: None,
    }
    .profile();
    let mut u = usage();
    u.reset_credits = Some(vec![Credit {
        id: "c".into(),
        status: "available".into(),
        reset_type: "codexRateLimits".into(),
        expires_at: Some(now - chrono::Duration::seconds(1)),
    }]);
    let events = Experiment::default().observe(&p, Some(&u), None, now);
    assert_eq!(events.len(), 4);
    assert!(
        events[1..]
            .iter()
            .all(|e| e.decision.as_deref() == Some("missed"))
    );
}
#[test]
fn update_filter_requires_personal_windows_stable_asset() {
    let valid = json!({"draft":false,"prerelease":false,"tag_name":"v0.2.0","assets":[{"name":"Cat-Codex-Switcher.exe"}]});
    let mut draft = valid.clone();
    draft["draft"] = json!(true);
    draft["tag_name"] = json!("v99.0.0");
    let mut wrong = valid.clone();
    wrong["assets"] = json!([{"name":"Codex-Account-Switcher-windows-x64.exe"}]);
    wrong["tag_name"] = json!("v98.0.0");
    assert_eq!(
        updates::latest(&[valid, draft, wrong]).unwrap().0,
        [0, 2, 0]
    );
    assert!(updates::repository("").unwrap().is_none());
    assert!(updates::repository("https://github.com/cat/repo").is_err());
    assert!(updates::repository("cat/repo").is_ok());
}
#[test]
fn failed_import_commit_restores_settings_cache_and_removes_staged_credentials() {
    let temp = tempfile::tempdir().unwrap();
    let mut source = store(&temp, "source");
    add(&mut source, "a", "a@example.invalid");
    let mut dest = store(&temp, "dest");
    let settings = dest.settings.clone();
    std::fs::create_dir(dest.root.join("accounts.json")).unwrap();
    assert!(dest.import(&source.root).is_err());
    assert!(dest.registry.accounts.is_empty());
    assert!(dest.settings == settings);
    assert_eq!(
        std::fs::read_dir(dest.root.join("accounts"))
            .unwrap()
            .count(),
        0
    );
}
#[test]
fn current_user_acl_has_no_inherited_or_other_user_entries() {
    use windows::{
        Win32::{
            Foundation::*,
            Security::{Authorization::*, *},
        },
        core::PCWSTR,
    };
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("private/auth.json");
    platform::atomic_write(&file, b"fixture").unwrap();
    unsafe {
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        let mut dacl = std::ptr::null_mut();
        GetNamedSecurityInfoW(
            PCWSTR(platform::wide(&file).as_ptr()),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            None,
            None,
            Some(&mut dacl),
            None,
            &mut descriptor,
        )
        .ok()
        .unwrap();
        assert!(!dacl.is_null());
        assert_eq!((*dacl).AceCount, 1);
        let mut ace = std::ptr::null_mut();
        GetAce(dacl, 0, &mut ace).unwrap();
        let ace = &*ace.cast::<ACCESS_ALLOWED_ACE>();
        assert_eq!(ace.Header.AceFlags & INHERITED_ACE.0 as u8, 0);
        let sid = PSID((&ace.SidStart as *const u32).cast_mut().cast());
        let mut text = windows::core::PWSTR::null();
        ConvertSidToStringSidW(sid, &mut text).unwrap();
        assert_eq!(text.to_string().unwrap(), platform::current_sid().unwrap());
        LocalFree(HLOCAL(text.0.cast()));
        LocalFree(HLOCAL(descriptor.0));
    }
}
#[test]
fn deleting_profile_with_nested_junction_restores_registry() {
    let temp = tempfile::tempdir().unwrap();
    let mut s = store(&temp, "data");
    let p = add(&mut s, "a", "a@example.invalid");
    let outside = temp.path().join("outside");
    std::fs::create_dir(&outside).unwrap();
    std::fs::write(outside.join("keep.txt"), b"keep").unwrap();
    let link = s.home(p.id).join("linked");
    let output = platform::hidden_command("cmd.exe")
        .args(["/c", "mklink", "/J"])
        .arg(&link)
        .arg(&outside)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(s.remove(p.id).is_err());
    assert_eq!(s.registry.accounts.len(), 1);
    assert!(outside.join("keep.txt").exists());
    std::fs::remove_dir(link).unwrap();
}
#[test]
fn service_fractional_timestamps_are_supported() {
    let v = json!({"rateLimits":{"secondary":{"usedPercent":20,"windowDurationMins":10080,"resetsAt":1795000000.5}},"rateLimitResetCredits":{"credits":[{"id":"c","status":"available","resetType":"codexRateLimits","expiresAt":1796000000.25}]}});
    let u = normalize(&v).unwrap();
    assert_eq!(u.resets_at.timestamp_subsec_millis(), 500);
    assert_eq!(
        u.reset_credits.unwrap()[0]
            .expires_at
            .unwrap()
            .timestamp_subsec_millis(),
        250
    );
}
