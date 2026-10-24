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
    collections::HashMap,
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
    pub fn send(&self, command: Command) {
        if !matches!(command, Command::Dismiss) {
            self.cancel.store(true, Ordering::SeqCst);
        }
        let _ = self.commands.send(command);
    }
}
struct Engine {
    store: Store,
    state: Snapshot,
    errors: HashMap<Uuid, String>,
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
                let path = if Some(p.id) == self.state.active {
                    self.store.active_file()
                } else {
                    self.store.credential(p.id)
                };
                Row {
                    profile: p.clone(),
                    usage: self
                        .store
                        .cache
                        .entries
                        .iter()
                        .find(|e| e.profile_id == p.id)
                        .map(|e| e.usage.clone()),
                    stale: self.errors.contains_key(&p.id),
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
                    let result = store::switch(&mut self.store, id);
                    self.publish();
                    result?;
                }
                self.refresh();
            }
            Command::Remove(id) => {
                self.store.sync_current()?;
                self.store.remove(id)?;
                self.errors.remove(&id);
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
                let current = self.store.preview(&path)?;
                ensure!(
                    previous.source == path
                        && previous.titles == current.titles
                        && previous.duplicates == current.duplicates,
                    "Import source changed. Preview it again before confirming."
                );
                self.store.import(&path)?;
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
            self.publish();
            return;
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
                        self.state.error = Some(format!("{e:#}"));
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
