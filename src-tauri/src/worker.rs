use crate::{
    experiment::{self, Event, Experiment},
    model::*,
    platform, rpc,
    store::{self, ImportPreview, Store},
    updates,
};
use anyhow::{Context, Result, ensure};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
use uuid::Uuid;

pub type Notifier = Arc<dyn Fn() + Send + Sync>;

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    pub profile: Profile,
    pub title: String,
    pub usage: Option<Usage>,
    pub stale: bool,
    pub fetched_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
    pub subscription: Option<(DateTime<Utc>, DateTime<Utc>)>,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub rows: Vec<Row>,
    pub active: Option<Uuid>,
    pub settings: Settings,
    pub busy: bool,
    pub adding: bool,
    pub refreshing: bool,
    pub error: Option<String>,
    pub preview: Option<ImportPreview>,
    pub update_url: Option<String>,
    pub update_status: String,
}
impl Default for Snapshot {
    fn default() -> Self {
        Self {
            rows: Vec::new(),
            active: None,
            settings: Settings::default(),
            busy: true,
            adding: false,
            refreshing: false,
            error: None,
            preview: None,
            update_url: None,
            update_status: String::new(),
        }
    }
}
impl Snapshot {
    pub fn can_quit(&self) -> bool {
        !self.adding && (!self.busy || self.refreshing)
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
    commands: mpsc::Sender<Command>,
    pub cancel: Arc<AtomicBool>,
    pub pending: Arc<AtomicUsize>,
    join: Mutex<Option<std::thread::JoinHandle<()>>>,
    shared: Arc<Mutex<Snapshot>>,
    gate: Arc<Mutex<()>>,
    stopping: Arc<AtomicBool>,
}
fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(text) = payload.downcast_ref::<&str>() {
        (*text).to_owned()
    } else if let Some(text) = payload.downcast_ref::<String>() {
        text.clone()
    } else {
        "unknown panic".into()
    }
}
impl Worker {
    pub fn start(
        root: PathBuf,
        active: PathBuf,
        notifier: Notifier,
        shared: Arc<Mutex<Snapshot>>,
    ) -> Self {
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let pending = Arc::new(AtomicUsize::new(0));
        let queued = pending.clone();
        let token = cancel.clone();
        let latest = shared.clone();
        let gate = Arc::new(Mutex::new(()));
        let worker_gate = gate.clone();
        let stopping = Arc::new(AtomicBool::new(false));
        let worker_stopping = stopping.clone();
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
                    refresh_failed: false,
                    pending: queued,
                    shared: shared.clone(),
                    notifier: notifier.clone(),
                    token,
                    rt,
                    subscriptions: None,
                };
                if let Err(e) = engine.reset_experiment("resumed") {
                    engine.state.error = Some(format!("{e:#}"));
                }
                if let Some(warning) = engine.store.warning.clone() {
                    engine.state.error = Some(match engine.state.error.take() {
                        Some(existing) => format!("{warning}\n{existing}"),
                        None => warning,
                    });
                }
                if engine.store.settings.launch_at_login
                    && let Err(e) = platform::startup(true)
                {
                    let message = format!("Could not repair launch-at-login: {e:#}");
                    engine.state.error = Some(match engine.state.error.take() {
                        Some(existing) => format!("{existing}\n{message}"),
                        None => message,
                    });
                }
                engine.state.busy = true;
                engine.publish();
                engine.refresh();
                if engine.store.settings.automatically_check_updates
                    && !engine.token.load(Ordering::SeqCst)
                {
                    engine.check_updates();
                }
                engine.state.busy = false;
                engine.publish();
                let mut next_refresh = Instant::now() + Duration::from_secs(300);
                let mut next_update = Instant::now() + Duration::from_secs(3600);
                loop {
                    match rx.recv_timeout(Duration::from_millis(200)) {
                        Ok(Command::Shutdown) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        Ok(command) => {
                            let dispatch = worker_gate.lock().unwrap();
                            if worker_stopping.load(Ordering::SeqCst) {
                                break;
                            }
                            engine.token.store(false, Ordering::SeqCst);
                            engine.state.busy = true;
                            if !matches!(command, Command::Dismiss) {
                                engine.state.error = None;
                                engine.refresh_failed = false;
                            }
                            engine.publish();
                            engine.pending.fetch_sub(1, Ordering::SeqCst);
                            drop(dispatch);
                            let result = engine.command(command);
                            if let Err(e) = result
                                && e.to_string() != "Operation cancelled"
                            {
                                engine.state.error = Some(format!("{e:#}"));
                            }
                            engine.state.busy = false;
                            engine.state.adding = false;
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
                            let refresh_due = Instant::now() >= next_refresh || checkpoint;
                            let update_due = Instant::now() >= next_update;
                            if refresh_due || update_due {
                                let dispatch = worker_gate.lock().unwrap();
                                if worker_stopping.load(Ordering::SeqCst) {
                                    break;
                                }
                                engine.state.busy = true;
                                engine.token.store(false, Ordering::SeqCst);
                                engine.publish();
                                drop(dispatch);
                                if refresh_due {
                                    engine.refresh();
                                    next_refresh = Instant::now() + Duration::from_secs(300);
                                }
                                if update_due {
                                    if engine.store.settings.automatically_check_updates
                                        && !engine.token.load(Ordering::SeqCst)
                                    {
                                        engine.check_updates();
                                    }
                                    next_update = Instant::now() + Duration::from_secs(3600);
                                }
                                engine.state.busy = false;
                                engine.publish();
                            }
                        }
                    }
                }
                Ok(())
            };
            let error = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(run)) {
                Ok(Ok(())) => None,
                Ok(Err(e)) => Some(format!("{e:#}")),
                Err(payload) => Some(format!(
                    "Background worker crashed: {}",
                    panic_message(payload)
                )),
            };
            if let Some(error) = error {
                // Reject commands before publishing an idle state for a stopped worker.
                drop(rx);
                let s = Snapshot {
                    busy: false,
                    error: Some(error),
                    ..Default::default()
                };
                if let Ok(mut latest) = shared.lock() {
                    *latest = s;
                }
                (notifier)();
            }
        });
        Self {
            commands: tx,
            cancel,
            pending,
            join: Mutex::new(Some(join)),
            shared: latest,
            gate,
            stopping,
        }
    }
    pub fn send(&self, command: Command) -> Result<()> {
        let _dispatch = self.gate.lock().unwrap();
        ensure!(
            !self.stopping.load(Ordering::SeqCst),
            "Application is shutting down"
        );
        if !matches!(command, Command::Dismiss) {
            let snapshot = self.shared.lock().unwrap();
            ensure!(
                self.pending.load(Ordering::SeqCst) == 0 && (!snapshot.busy || snapshot.refreshing),
                "Another operation is already running"
            );
        }
        if !matches!(command, Command::Dismiss) {
            self.cancel.store(true, Ordering::SeqCst);
        }
        self.pending.fetch_add(1, Ordering::SeqCst);
        self.commands.send(command).map_err(|_| {
            self.pending.fetch_sub(1, Ordering::SeqCst);
            anyhow::anyhow!("Background worker stopped. Restart the application.")
        })
    }

    pub fn cancel_login(&self) -> Result<()> {
        let _dispatch = self.gate.lock().unwrap();
        ensure!(
            self.shared.lock().unwrap().adding,
            "No browser login is running"
        );
        self.cancel.store(true, Ordering::SeqCst);
        Ok(())
    }

    pub fn begin_shutdown(&self) -> bool {
        let _dispatch = self.gate.lock().unwrap();
        if self.stopping.load(Ordering::SeqCst) {
            return false;
        }
        let stopped = self
            .join
            .lock()
            .unwrap()
            .as_ref()
            .is_none_or(|join| join.is_finished());
        if !stopped
            && (self.pending.load(Ordering::SeqCst) > 0 || !self.shared.lock().unwrap().can_quit())
        {
            return false;
        }
        self.stopping.store(true, Ordering::SeqCst);
        self.cancel.store(true, Ordering::SeqCst);
        let _ = self.commands.send(Command::Shutdown);
        true
    }

    pub fn join(&self) {
        if let Some(join) = self.join.lock().unwrap().take() {
            let _ = join.join();
        }
    }
}
type Subscription = Option<(DateTime<Utc>, DateTime<Utc>)>;
struct Engine {
    store: Store,
    state: Snapshot,
    errors: HashMap<Uuid, String>,
    fresh: HashSet<Uuid>,
    experiment: Option<Experiment>,
    refresh_failed: bool,
    pending: Arc<AtomicUsize>,
    shared: Arc<Mutex<Snapshot>>,
    notifier: Notifier,
    token: Arc<AtomicBool>,
    rt: tokio::runtime::Runtime,
    // Only lives during one refresh; never retains credential bytes or tokens.
    subscriptions: Option<HashMap<Uuid, Subscription>>,
}
impl Engine {
    fn publish_batch(&mut self, operation: impl FnOnce(&mut Self)) {
        self.subscriptions = Some(HashMap::new());
        operation(self);
        self.subscriptions = None;
    }
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
                let load_subscription =
                    || platform::read(&path).ok().and_then(|b| subscription(&b, p));
                let subscription = match &mut self.subscriptions {
                    Some(cache) => *cache.entry(p.id).or_insert_with(load_subscription),
                    None => load_subscription(),
                };
                Row {
                    profile: p.clone(),
                    title: self.store.registry.label(p),
                    usage: cached.map(|e| e.usage.clone()),
                    stale: self.errors.contains_key(&p.id)
                        || cached.is_some_and(|e| {
                            !self.fresh.contains(&p.id)
                                || Utc::now() - e.fetched_at >= chrono::Duration::minutes(5)
                        }),
                    fetched_at: cached.map(|e| e.fetched_at),
                    error: self.errors.get(&p.id).cloned(),
                    subscription,
                }
            })
            .collect();
        if let Ok(mut latest) = self.shared.lock() {
            let queued = self.pending.load(Ordering::SeqCst) > 0;
            self.state.busy |= queued;
            let mut snapshot = self.state.clone();
            snapshot.refreshing &= !queued;
            *latest = snapshot;
        }
        (self.notifier)();
    }
    fn reset_experiment(&mut self, event: &str) -> Result<()> {
        if self.store.settings.reset_expiry_experiment_enabled {
            let result = Experiment::load(&self.log()).and_then(|experiment| {
                experiment::append(&self.log(), &[Event::marker(event, Utc::now())])?;
                Ok(experiment)
            });
            match result {
                Ok(experiment) => self.experiment = Some(experiment),
                Err(error) => return Err(self.stop_experiment(error)),
            }
        } else {
            self.experiment = None;
        }
        Ok(())
    }
    fn stop_experiment(&mut self, error: anyhow::Error) -> anyhow::Error {
        self.experiment = None;
        let mut settings = self.store.settings.clone();
        settings.reset_expiry_experiment_enabled = false;
        if let Err(save) = self.store.save_settings(settings.clone()) {
            self.store.settings = settings;
            anyhow::anyhow!(
                "Reset experiment stopped: {error:#}; could not save disabled setting: {save:#}"
            )
        } else {
            error.context("Reset experiment stopped")
        }
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
                if let Err(e) = result {
                    return match self.store.discard(id) {
                        Ok(()) => Err(e),
                        Err(cleanup) => Err(anyhow::anyhow!("{e:#}; cleanup failed: {cleanup:#}")),
                    };
                }
                self.state.adding = false;
                self.refresh();
            }
            Command::Register => {
                self.store.sync_current()?;
                self.refresh();
            }
            Command::Switch(id) => {
                self.store.sync_current()?;
                if self.store.registry.active_account_id != Some(id) {
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
        self.state.refreshing = true;
        self.publish();
        let exe = rpc::locate();
        self.refresh_with(|rt, home, profile, token| match &exe {
            Ok(exe) => rt.block_on(async {
                let mut session = rpc::Session::start(exe, home, token).await?;
                let result = session.usage(profile).await;
                session.stop().await;
                result
            }),
            Err(_) => Err(anyhow::anyhow!(
                "Codex runtime not found. Set CODEX_CLI_PATH."
            )),
        });
        self.state.refreshing = false;
        self.publish();
    }
    fn refresh_with(
        &mut self,
        read_usage: impl FnMut(
            &tokio::runtime::Runtime,
            &Path,
            &Profile,
            Arc<AtomicBool>,
        ) -> Result<Usage>,
    ) {
        self.publish_batch(|engine| engine.refresh_profiles(read_usage));
    }
    fn refresh_profiles(
        &mut self,
        mut read_usage: impl FnMut(
            &tokio::runtime::Runtime,
            &Path,
            &Profile,
            Arc<AtomicBool>,
        ) -> Result<Usage>,
    ) {
        if self.refresh_failed {
            self.refresh_failed = false;
            self.state.error = None;
        }
        let _original = match platform::OriginalGuard::default_location() {
            Ok(g) => g,
            Err(e) => {
                self.refresh_error(e);
                self.publish();
                return;
            }
        };
        if let Err(e) = self.store.sync_current() {
            self.refresh_error(e);
            // Saved profiles have independent credentials and can still be refreshed.
        }
        self.publish();
        for profile in self.store.registry.accounts.clone() {
            if self.token.load(Ordering::SeqCst) {
                break;
            }
            let active = self.store.registry.active_account_id == Some(profile.id)
                && self.store.verify(profile.id).is_ok();
            let home = if active {
                self.store.active.clone()
            } else {
                self.store.home(profile.id)
            };
            let mut result = read_usage(&self.rt, &home, &profile, self.token.clone());
            // Preserve rotated tokens even when the read fails or is cancelled.
            if active && let Err(e) = self.store.save_current(Some(profile.id)) {
                let message = format!("{e:#}");
                self.refresh_error(anyhow::anyhow!(message.clone()));
                result = Err(anyhow::anyhow!(message));
            }
            // The runtime can rotate tokens even on error/cancellation. Re-read
            // this profile's subscription rather than serving pre-rotation data.
            if let Some(cache) = &mut self.subscriptions {
                cache.remove(&profile.id);
            }
            if self.token.load(Ordering::SeqCst) {
                break;
            }
            let (usage, error) = match result {
                Ok(usage) => {
                    self.errors.remove(&profile.id);
                    if let Err(e) = self.store.cache(profile.id, usage.clone()) {
                        self.fresh.remove(&profile.id);
                        self.refresh_error(e);
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
                    let error = self.stop_experiment(e);
                    self.refresh_error(error);
                }
            }
            self.publish();
        }
        self.publish();
    }
    fn refresh_error(&mut self, error: anyhow::Error) {
        self.state.error = Some(format!("{error:#}"));
        self.refresh_failed = true;
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
