use crate::{
    experiment::{self, Event, Experiment},
    model::*,
    platform, rpc,
    store::{self, ImportPreview, Store},
    updates,
};
use anyhow::{Context, Result, ensure};
use chrono::{DateTime, Utc};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
use uuid::Uuid;

#[derive(Clone)]
pub struct Row {
    pub profile: Profile,
    pub usage: Option<Usage>,
    pub stale: bool,
    pub fetched_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
    pub subscription: Option<(DateTime<Utc>, DateTime<Utc>)>,
}
#[derive(Clone)]
pub struct Snapshot {
    pub rows: Vec<Row>,
    pub active: Option<Uuid>,
    pub settings: Settings,
    pub busy: bool,
    pub adding: bool,
    pub error: Option<String>,
    pub preview: Option<ImportPreview>,
    pub update_url: Option<String>,
    pub update_status: String,
    pub stage: String,
}
impl Default for Snapshot {
    fn default() -> Self {
        Self {
            rows: Vec::new(),
            active: None,
            settings: Settings::default(),
            busy: true,
            adding: false,
            error: None,
            preview: None,
            update_url: None,
            update_status: String::new(),
            stage: String::new(),
        }
    }
}
pub enum Command {
    Refresh,
    Add,
    Register,
    Switch(Uuid),
    Remove(Uuid),
    Settings(Settings),
    Preview(PathBuf),
    Import(PathBuf),
    CheckUpdates,
    Dismiss,
    Shutdown,
}
pub struct Worker {
    pub commands: mpsc::Sender<Command>,
    pub cancel: Arc<AtomicBool>,
    pub join: Option<std::thread::JoinHandle<()>>,
}
impl Worker {
    pub fn start(
        root: PathBuf,
        active: PathBuf,
        ctx: egui::Context,
        shared: Arc<Mutex<Snapshot>>,
    ) -> Self {
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let token = cancel.clone();
        let join = std::thread::spawn(move || {
            let run = || -> Result<()> {
                let rt = tokio::runtime::Runtime::new()?;
                let store = Store::load(root, active)?;
                let mut engine = Engine {
                    store,
                    state: Snapshot::default(),
                    errors: HashMap::new(),
                    fresh: HashSet::new(),
                    experiment: None,
                    shared: shared.clone(),
                    ctx: ctx.clone(),
                    token,
                    rt,
                };
                if let Err(e) = engine.reset_experiment("resumed") {
                    engine.experiment = None;
                    engine.state.error = Some(format!("{e:#}"));
                }
                engine.state.busy = false;
                engine.publish();
                engine.refresh();
                if engine.store.settings.automatically_check_updates {
                    engine.check_updates();
                }
                let mut next_refresh = Instant::now() + Duration::from_secs(300);
                let mut next_update = Instant::now() + Duration::from_secs(3600);
                loop {
                    match rx.recv_timeout(Duration::from_millis(200)) {
                        Ok(Command::Shutdown) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        Ok(command) => {
                            engine.token.store(false, Ordering::SeqCst);
                            engine.state.busy = true;
                            if !matches!(command, Command::Dismiss) {
                                engine.state.error = None;
                            }
                            engine.publish();
                            let result = engine.command(command);
                            if let Err(e) = result
                                && e.to_string() != "Operation cancelled"
                            {
                                engine.state.error = Some(format!("{e:#}"));
                            }
                            engine.state.busy = false;
                            engine.state.adding = false;
                            engine.state.stage.clear();
                            engine.publish();
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => {
                            let checkpoint = engine
                                .experiment
                                .as_ref()
                                .and_then(|e| {
                                    e.next(
                                        &engine
                                            .store
                                            .registry
                                            .accounts
                                            .iter()
                                            .map(|p| p.id)
                                            .collect(),
                                    )
                                })
                                .is_some_and(|t| t <= Utc::now());
                            if Instant::now() >= next_refresh || checkpoint {
                                engine.token.store(false, Ordering::SeqCst);
                                engine.refresh();
                                next_refresh = Instant::now() + Duration::from_secs(300);
                            }
                            if Instant::now() >= next_update {
                                if engine.store.settings.automatically_check_updates {
                                    engine.check_updates();
                                }
                                next_update = Instant::now() + Duration::from_secs(3600);
                            }
                        }
                    }
                }
                Ok(())
            };
            if let Err(e) = run() {
                // Reject commands before publishing an idle state for a stopped worker.
                drop(rx);
                let s = Snapshot {
                    busy: false,
                    error: Some(format!("{e:#}")),
                    ..Default::default()
                };
                if let Ok(mut latest) = shared.lock() {
                    *latest = s;
                }
                ctx.request_repaint();
            }
        });
        Self {
            commands: tx,
            cancel,
            join: Some(join),
        }
    }
    pub fn send(&self, command: Command) -> Result<()> {
        if !matches!(command, Command::Dismiss) {
            self.cancel.store(true, Ordering::SeqCst);
        }
        self.commands
            .send(command)
            .map_err(|_| anyhow::anyhow!("Background worker stopped. Restart the application."))
    }
}
struct Engine {
    store: Store,
    state: Snapshot,
    errors: HashMap<Uuid, String>,
    fresh: HashSet<Uuid>,
    experiment: Option<Experiment>,
    shared: Arc<Mutex<Snapshot>>,
    ctx: egui::Context,
    token: Arc<AtomicBool>,
    rt: tokio::runtime::Runtime,
}
impl Engine {
    fn log(&self) -> PathBuf {
        self.store.root.join("reset-expiry-experiment.jsonl")
    }
    fn publish(&mut self) {
        self.state.settings = self.store.settings.clone();
        self.state.active = self.store.registry.active_account_id;
        self.state.rows = self
            .store
            .registry
            .accounts
            .iter()
            .map(|p| {
                let cached = self
                    .store
                    .cache
                    .entries
                    .iter()
                    .find(|e| e.profile_id == p.id);
                let path = if Some(p.id) == self.state.active {
                    self.store.active_file()
                } else {
                    self.store.credential(p.id)
                };
                Row {
                    profile: p.clone(),
                    usage: cached.map(|e| e.usage.clone()),
                    stale: self.errors.contains_key(&p.id)
                        || cached.is_some_and(|e| {
                            !self.fresh.contains(&p.id)
                                || Utc::now() - e.fetched_at >= chrono::Duration::minutes(5)
                        }),
                    fetched_at: cached.map(|e| e.fetched_at),
                    error: self.errors.get(&p.id).cloned(),
                    subscription: platform::read(&path).ok().and_then(|b| subscription(&b, p)),
                }
            })
            .collect();
        if let Ok(mut latest) = self.shared.lock() {
            *latest = self.state.clone();
        }
        self.ctx.request_repaint();
    }
    fn reset_experiment(&mut self, event: &str) -> Result<()> {
        if self.store.settings.reset_expiry_experiment_enabled {
            self.experiment = Some(Experiment::load(&self.log())?);
            experiment::append(&self.log(), &[Event::marker(event, Utc::now())])?;
        } else {
            self.experiment = None;
        }
        Ok(())
    }
    fn command(&mut self, command: Command) -> Result<()> {
        let _original = if matches!(
            command,
            Command::Add | Command::Register | Command::Switch(_) | Command::Remove(_)
        ) {
            Some(platform::OriginalGuard::default_location()?)
        } else {
            None
        };
        match command {
            Command::Refresh => self.refresh(),
            Command::Add => {
                self.state.error = None;
                self.state.adding = true;
                self.state.stage = "login".into();
                self.publish();
                let id = Uuid::new_v4();
                let home = self.store.home(id);
                platform::secure_dir(&home)?;
                let result = self.rt.block_on(async {
                    let exe = rpc::locate()?;
                    let mut session = rpc::Session::start(&exe, &home, self.token.clone()).await?;
                    let result = session.login(&home).await;
                    session.stop().await;
                    result
                });
                let result = result.and_then(|identity| {
                    ensure!(!self.token.load(Ordering::SeqCst), "Operation cancelled");
                    let mut profile = identity.profile();
                    profile.id = id;
                    self.store.add(profile)
                });
                if result.is_err() {
                    self.store
                        .discard(id)
                        .context("Failed to remove incomplete login")?;
                }
                result?;
                self.refresh();
            }
            Command::Register => {
                self.store.sync_current()?;
                self.refresh();
            }
            Command::Switch(id) => {
                self.store.sync_current()?;
                if self.store.registry.active_account_id != Some(id) {
                    self.state.stage = "switch".into();
                    self.publish();
                    let result = store::switch(&mut self.store, id, &self.rt, self.token.clone());
                    self.publish();
                    result?;
                }
                self.refresh();
            }
            Command::Remove(id) => {
                self.store.sync_current()?;
                self.store.remove(id)?;
                self.errors.remove(&id);
                self.fresh.remove(&id);
            }
            Command::Settings(s) => {
                updates::repository(&s.update_repository)?;
                let old = self.store.settings.clone();
                let startup_changed = s.launch_at_login != old.launch_at_login;
                if startup_changed {
                    platform::startup(s.launch_at_login)?;
                }
                if let Err(e) = self.store.save_settings(s) {
                    if startup_changed {
                        let _ = platform::startup(old.launch_at_login);
                    }
                    return Err(e);
                }
                if old.update_repository != self.store.settings.update_repository {
                    self.state.update_url = None;
                    self.state.update_status.clear();
                }
                if old.reset_expiry_experiment_enabled
                    != self.store.settings.reset_expiry_experiment_enabled
                {
                    if self.store.settings.reset_expiry_experiment_enabled {
                        self.reset_experiment("enabled")?;
                    } else {
                        self.experiment = None;
                        experiment::append(&self.log(), &[Event::marker("disabled", Utc::now())])?;
                    }
                }
            }
            Command::Preview(path) => {
                self.state.preview = Some(self.store.preview(&path)?);
            }
            Command::Import(path) => {
                let previous = self
                    .state
                    .preview
                    .as_ref()
                    .context("Preview the import before confirming")?;
                ensure!(
                    previous.source == path,
                    "Import path changed. Preview it again before confirming."
                );
                self.store.import(previous)?;
                self.fresh.clear();
                self.state.preview = None;
                self.reset_experiment("resumed")?;
                self.refresh();
            }
            Command::CheckUpdates => self.check_updates(),
            Command::Dismiss => {
                self.state.error = None;
                self.state.preview = None;
            }
            Command::Shutdown => {}
        }
        Ok(())
    }
    fn refresh(&mut self) {
        let _original = match platform::OriginalGuard::default_location() {
            Ok(g) => g,
            Err(e) => {
                self.state.error = Some(format!("{e:#}"));
                self.publish();
                return;
            }
        };
        if let Err(e) = self.store.sync_current() {
            self.state.error = Some(format!("{e:#}"));
            // Saved profiles have independent credentials and can still be refreshed.
        }
        self.state.stage = "refresh".into();
        self.publish();
        let exe = rpc::locate();
        for profile in self.store.registry.accounts.clone() {
            if self.token.load(Ordering::SeqCst) {
                break;
            }
            let result = match &exe {
                Ok(exe) => self.rt.block_on(async {
                    let mut session =
                        rpc::Session::start(exe, &self.store.home(profile.id), self.token.clone())
                            .await?;
                    let result = session.usage().await;
                    session.stop().await;
                    result
                }),
                Err(_) => Err(anyhow::anyhow!(
                    "Codex runtime not found. Set CODEX_CLI_PATH."
                )),
            };
            if self.token.load(Ordering::SeqCst) {
                break;
            }
            let (usage, error) = match result {
                Ok(usage) => {
                    self.errors.remove(&profile.id);
                    if let Err(e) = self.store.cache(profile.id, usage.clone()) {
                        self.fresh.remove(&profile.id);
                        self.state.error = Some(format!("{e:#}"));
                    } else {
                        self.fresh.insert(profile.id);
                    }
                    (Some(usage), None)
                }
                Err(e) => {
                    let text = format!("{e:#}");
                    self.errors.insert(profile.id, text.clone());
                    (None, Some(text))
                }
            };
            if let Some(experiment) = &mut self.experiment {
                let events = experiment.observe(&profile, usage.as_ref(), error, Utc::now());
                if let Err(e) = experiment::append(&self.log(), &events) {
                    self.experiment = None;
                    self.state.error = Some(format!("{e:#}"));
                }
            }
            self.publish();
        }
        self.state.stage.clear();
        self.publish();
    }
    fn check_updates(&mut self) {
        if self.store.settings.update_repository.trim().is_empty() {
            self.state.update_url = None;
            self.state.update_status = "update_unconfigured".into();
            self.publish();
            return;
        }
        self.state.update_status = "checking".into();
        self.publish();
        match self
            .rt
            .block_on(updates::check(&self.store.settings.update_repository))
        {
            Ok(url) => {
                self.state.update_status = if url.is_some() {
                    "update_available"
                } else {
                    "up_to_date"
                }
                .into();
                self.state.update_url = url;
            }
            Err(_) => {
                self.state.update_status = "update_failed".into();
                self.state.update_url = None;
            }
        }
        self.publish();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cached_usage_is_stale_until_refreshed_in_this_run() {
        let profile = Identity {
            account_id: Some("saved-id".into()),
            email: None,
        }
        .profile();
        let id = profile.id;
        let fetched_at = Utc::now();
        let root = std::env::temp_dir().join(Uuid::new_v4().to_string());
        let mut engine = Engine {
            store: Store {
                root: root.join("store"),
                active: root.join("active"),
                registry: Registry {
                    accounts: vec![profile],
                    active_account_id: Some(id),
                    ..Default::default()
                },
                settings: Settings::default(),
                cache: Cache {
                    entries: vec![CacheEntry {
                        profile_id: id,
                        fetched_at,
                        usage: Usage {
                            remaining_percent: 42,
                            resets_at: fetched_at,
                            five_hour_remaining_percent: None,
                            five_hour_resets_at: None,
                            reset_credits_remaining: None,
                            reset_credits: None,
                        },
                    }],
                    ..Default::default()
                },
            },
            state: Snapshot::default(),
            errors: HashMap::new(),
            fresh: HashSet::new(),
            experiment: None,
            shared: Arc::new(Mutex::new(Snapshot::default())),
            ctx: egui::Context::default(),
            token: Arc::new(AtomicBool::new(false)),
            rt: tokio::runtime::Runtime::new().unwrap(),
        };
        engine.publish();
        assert!(engine.state.rows[0].stale);
        assert_eq!(engine.state.rows[0].fetched_at, Some(fetched_at));
        engine.fresh.insert(id);
        engine.publish();
        assert!(!engine.state.rows[0].stale);
        engine.store.cache.entries[0].fetched_at -= chrono::Duration::minutes(6);
        engine.publish();
        assert!(engine.state.rows[0].stale);
        engine.store.cache.entries[0].fetched_at = fetched_at;
        engine.errors.insert(id, "Refresh failed".into());
        engine.publish();
        assert!(engine.state.rows[0].stale);
        assert_eq!(
            engine.state.rows[0]
                .usage
                .as_ref()
                .unwrap()
                .remaining_percent,
            42
        );
    }

    #[test]
    fn invalid_active_login_still_refreshes_saved_profiles() {
        // Isolate runtime discovery from the user's environment and real accounts.
        if std::env::var_os("SWITCHER_REFRESH_TEST_CHILD").is_none() {
            let missing = std::env::temp_dir().join(Uuid::new_v4().to_string());
            let status = platform::hidden_command(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "worker::tests::invalid_active_login_still_refreshes_saved_profiles",
                ])
                .env("SWITCHER_REFRESH_TEST_CHILD", "1")
                .env("CODEX_CLI_PATH", missing.join("missing.exe"))
                .env("CODEX_SWITCHER_DATA_HOME", &missing)
                .status()
                .unwrap();
            assert!(status.success());
            return;
        }
        let temp = std::env::temp_dir();
        let root = temp.join(format!("switcher-test-{}", Uuid::new_v4()));
        let mut store = Store::load(root.join("store"), root.join("active")).unwrap();
        let profiles: Vec<_> = ["first", "second"]
            .into_iter()
            .map(|id| {
                Identity {
                    account_id: Some(id.into()),
                    email: None,
                }
                .profile()
            })
            .collect();
        store
            .save_registry(Registry {
                format_version: FORMAT,
                active_account_id: Some(profiles[0].id),
                accounts: profiles.clone(),
            })
            .unwrap();
        platform::atomic_write(&store.active_file(), b"invalid JSON").unwrap();
        for profile in &profiles {
            store
                .cache(
                    profile.id,
                    Usage {
                        remaining_percent: 42,
                        resets_at: Utc::now(),
                        five_hour_remaining_percent: None,
                        five_hour_resets_at: None,
                        reset_credits_remaining: None,
                        reset_credits: None,
                    },
                )
                .unwrap();
        }
        let mut engine = Engine {
            store,
            state: Snapshot::default(),
            errors: HashMap::new(),
            fresh: HashSet::new(),
            experiment: None,
            shared: Arc::new(Mutex::new(Snapshot::default())),
            ctx: egui::Context::default(),
            token: Arc::new(AtomicBool::new(false)),
            rt: tokio::runtime::Runtime::new().unwrap(),
        };
        engine.refresh();
        assert!(
            engine
                .state
                .error
                .as_deref()
                .unwrap()
                .contains("Invalid credential JSON")
        );
        assert_eq!(engine.state.rows.len(), 2);
        for row in &engine.state.rows {
            assert!(row.stale);
            assert!(
                row.error
                    .as_deref()
                    .unwrap()
                    .contains("Codex runtime not found")
            );
            assert_eq!(row.usage.as_ref().unwrap().remaining_percent, 42);
        }
        assert_eq!(
            platform::read(&engine.store.active_file()).unwrap(),
            b"invalid JSON"
        );
        drop(engine);
        platform::remove_tree(&root, &temp).unwrap();
    }
}
