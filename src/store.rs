use crate::{model::*, platform};
use anyhow::{Context, Result, bail, ensure};
use chrono::Utc;
use serde::{Serialize, de::DeserializeOwned};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};
use uuid::Uuid;

pub struct Store {
    pub root: PathBuf,
    pub active: PathBuf,
    pub registry: Registry,
    pub settings: Settings,
    pub cache: Cache,
}
#[derive(Clone)]
pub struct ImportPreview {
    pub titles: Vec<String>,
    pub duplicates: usize,
    pub source: PathBuf,
}
struct ImportData {
    registry: Registry,
    settings: Settings,
    cache: Cache,
    credentials: Vec<(Uuid, Vec<u8>)>,
}
impl Store {
    pub fn load(root: PathBuf, active: PathBuf) -> Result<Self> {
        platform::secure_dir(&root)?;
        let registry: Registry = load_or(&root.join("accounts.json"))?;
        let settings: Settings = load_or(&root.join("settings.json"))?;
        let cache: Cache = load_or(&root.join("usage-cache.json"))?;
        ensure!(
            registry.format_version <= FORMAT
                && settings.format_version <= FORMAT
                && cache.format_version <= FORMAT,
            "Unsupported data format version"
        );
        validate_registry(&registry)?;
        Ok(Self {
            root,
            active,
            registry,
            settings,
            cache,
        })
    }
    pub fn home(&self, id: Uuid) -> PathBuf {
        self.root.join("accounts").join(id.to_string())
    }
    pub fn credential(&self, id: Uuid) -> PathBuf {
        self.home(id).join("auth.json")
    }
    pub fn active_file(&self) -> PathBuf {
        self.active.join("auth.json")
    }
    pub fn save_registry(&mut self, mut registry: Registry) -> Result<()> {
        registry.format_version = FORMAT;
        validate_registry(&registry)?;
        write(&self.root.join("accounts.json"), &registry)?;
        self.registry = registry;
        Ok(())
    }
    pub fn save_settings(&mut self, settings: Settings) -> Result<()> {
        write(&self.root.join("settings.json"), &settings)?;
        self.settings = settings;
        Ok(())
    }
    pub fn cache(&mut self, id: Uuid, usage: Usage) -> Result<()> {
        let mut cache = self.cache.clone();
        cache.format_version = FORMAT;
        cache.entries.retain(|e| e.profile_id != id);
        cache.entries.push(CacheEntry {
            profile_id: id,
            usage,
            fetched_at: Utc::now(),
        });
        write(&self.root.join("usage-cache.json"), &cache)?;
        self.cache = cache;
        Ok(())
    }
    pub fn sync_current(&mut self) -> Result<()> {
        platform::check_path(&self.active_file())?;
        if !self.active_file().exists() {
            if self.registry.active_account_id.is_some() {
                let mut r = self.registry.clone();
                r.active_account_id = None;
                self.save_registry(r)?;
            }
            return Ok(());
        }
        let bytes = platform::read(&self.active_file())?;
        let current = identity(&bytes)?;
        let mut r = self.registry.clone();
        let id = if let Some(p) = r.accounts.iter().find(|p| current.matches(p)) {
            p.id
        } else {
            let p = current.profile();
            let id = p.id;
            r.accounts.push(p);
            id
        };
        platform::atomic_write(&self.credential(id), &bytes)?;
        r.active_account_id = Some(id);
        self.save_registry(r)
    }
    pub fn add(&mut self, profile: Profile) -> Result<()> {
        let current = identity(&platform::read(&self.credential(profile.id))?)?;
        ensure!(
            current.matches(&profile),
            "Saved login identity does not match profile"
        );
        ensure!(
            !self.registry.accounts.iter().any(|p| current.matches(p)),
            "This account is already saved"
        );
        let mut r = self.registry.clone();
        r.accounts.push(profile);
        self.save_registry(r)
    }
    pub fn discard(&self, id: Uuid) -> Result<()> {
        ensure!(
            !self.registry.accounts.iter().any(|p| p.id == id),
            "Cannot discard a registered account"
        );
        if self.home(id).exists() {
            platform::remove_tree(&self.home(id), &self.root)?;
        }
        Ok(())
    }
    pub fn remove(&mut self, id: Uuid) -> Result<()> {
        ensure!(
            self.registry.active_account_id != Some(id),
            "The active account cannot be removed"
        );
        ensure!(
            self.registry.accounts.iter().any(|p| p.id == id),
            "Account not found"
        );
        // Validate the complete tree before publishing the removal.
        platform::check_path(&self.home(id))?;
        let original = self.registry.clone();
        let mut r = original.clone();
        r.accounts.retain(|p| p.id != id);
        self.save_registry(r)?;
        if self.home(id).exists()
            && let Err(removal) = platform::remove_tree(&self.home(id), &self.root)
        {
            return match self.save_registry(original) {
                Ok(()) => Err(removal),
                Err(restore) => Err(anyhow::anyhow!(
                    "{removal}; restoring account list also failed: {restore}"
                )),
            };
        }
        let mut cache = self.cache.clone();
        cache.entries.retain(|e| e.profile_id != id);
        write(&self.root.join("usage-cache.json"), &cache)?;
        self.cache = cache;
        Ok(())
    }
    pub fn save_current(&self, original: Option<Uuid>) -> Result<()> {
        if let Some(id) = original {
            let profile = self
                .registry
                .accounts
                .iter()
                .find(|p| p.id == id)
                .context("Active account missing")?;
            let bytes = platform::read(&self.active_file())?;
            ensure!(
                identity(&bytes)?.matches(profile),
                "Current login changed. Register it before switching; no saved credential was overwritten."
            );
            platform::atomic_write(&self.credential(id), &bytes)
        } else {
            ensure!(
                !self.active_file().exists(),
                "Register the current login before switching"
            );
            Ok(())
        }
    }
    pub fn activate(&self, id: Uuid) -> Result<()> {
        let bytes = platform::read(&self.credential(id))?;
        let profile = self
            .registry
            .accounts
            .iter()
            .find(|p| p.id == id)
            .context("Account not found")?;
        ensure!(
            identity(&bytes)?.matches(profile),
            "Target credential identity mismatch"
        );
        platform::atomic_write(&self.active_file(), &bytes)
    }
    pub fn restore(&self, original: Option<Uuid>) -> Result<()> {
        if let Some(id) = original {
            self.activate(id)
        } else {
            platform::check_path(&self.active_file())?;
            if self.active_file().exists() {
                fs::remove_file(self.active_file())?;
            }
            Ok(())
        }
    }
    pub fn verify(&self, id: Uuid) -> Result<()> {
        let profile = self
            .registry
            .accounts
            .iter()
            .find(|p| p.id == id)
            .context("Account not found")?;
        ensure!(
            identity(&platform::read(&self.active_file())?)?.matches(profile),
            "Target identity verification failed"
        );
        Ok(())
    }
    pub fn commit(&mut self, id: Uuid) -> Result<()> {
        let mut r = self.registry.clone();
        r.active_account_id = Some(id);
        r.accounts
            .iter_mut()
            .find(|p| p.id == id)
            .context("Account not found")?
            .last_used_at = Some(Utc::now());
        self.save_registry(r)
    }
    fn import_data(&self, source: &Path) -> Result<ImportData> {
        platform::check_path(source)?;
        ensure!(
            fs::canonicalize(source)? != fs::canonicalize(&self.root)?,
            "Cannot import this app's own data"
        );
        let mut registry: Registry =
            serde_json::from_slice(&platform::read(&source.join("accounts.json"))?)
                .context("Invalid import registry")?;
        ensure!(
            registry.format_version <= FORMAT,
            "Unsupported import format"
        );
        let settings: Settings = load_or(&source.join("settings.json"))?;
        let cache: Cache = load_or(&source.join("usage-cache.json"))?;
        ensure!(
            settings.format_version <= FORMAT && cache.format_version <= FORMAT,
            "Unsupported import format"
        );
        let mut credentials = Vec::new();
        for p in &mut registry.accounts {
            let lower = source
                .join("accounts")
                .join(p.id.to_string())
                .join("auth.json");
            let upper = source
                .join("accounts")
                .join(p.id.to_string().to_uppercase())
                .join("auth.json");
            let bytes = platform::read(if lower.exists() { &lower } else { &upper })?;
            let actual = identity(&bytes)?;
            if p.account_id.is_none() {
                p.account_id = actual.account_id.clone();
            }
            ensure!(actual.matches(p), "Imported credential identity mismatch");
            credentials.push((p.id, bytes));
        }
        validate_registry(&registry)?;
        Ok(ImportData {
            registry,
            settings,
            cache,
            credentials,
        })
    }
    pub fn preview(&self, source: &Path) -> Result<ImportPreview> {
        let _original = platform::OriginalGuard::acquire(source)?;
        let data = self.import_data(source)?;
        let mut titles = Vec::new();
        let mut duplicates = 0;
        for p in &data.registry.accounts {
            let i = Identity {
                account_id: p.account_id.clone(),
                email: p.email.clone(),
            };
            if self.registry.accounts.iter().any(|p| i.matches(p)) {
                duplicates += 1;
            } else {
                titles.push(p.title().to_owned());
            }
        }
        Ok(ImportPreview {
            titles,
            duplicates,
            source: source.to_owned(),
        })
    }
    pub fn import(&mut self, source: &Path) -> Result<usize> {
        let _original = platform::OriginalGuard::acquire(source)?;
        let data = self.import_data(source)?;
        let mut registry = self.registry.clone();
        let mut cache = self.cache.clone();
        let mut installed = Vec::new();
        let old_settings = self.settings.clone();
        let old_cache = self.cache.clone();
        let result = (|| -> Result<()> {
            for p in &data.registry.accounts {
                let i = Identity {
                    account_id: p.account_id.clone(),
                    email: p.email.clone(),
                };
                if registry.accounts.iter().any(|p| i.matches(p)) {
                    continue;
                }
                let mut p = p.clone();
                let source_id = p.id;
                if registry.accounts.iter().any(|a| a.id == p.id) || self.home(p.id).exists() {
                    p.id = Uuid::new_v4();
                }
                installed.push(p.id);
                let bytes = &data
                    .credentials
                    .iter()
                    .find(|(id, _)| *id == source_id)
                    .context("Missing imported credential")?
                    .1;
                platform::atomic_write(&self.credential(p.id), bytes)?;
                if let Some(entry) = data
                    .cache
                    .entries
                    .iter()
                    .find(|e| e.profile_id == source_id)
                {
                    let mut entry = entry.clone();
                    entry.profile_id = p.id;
                    cache.entries.push(entry);
                }
                registry.accounts.push(p);
            }
            let mut settings = data.settings.clone();
            settings.format_version = FORMAT;
            settings.launch_at_login = old_settings.launch_at_login;
            settings.automatically_check_updates = old_settings.automatically_check_updates;
            settings.update_repository = old_settings.update_repository.clone();
            settings.reset_expiry_experiment_enabled = old_settings.reset_expiry_experiment_enabled;
            settings.language = match settings.language.as_str() {
                "en" | "english" => "en",
                "ru" => "ru",
                "uk" => "uk",
                "system" => "system",
                _ => "uk",
            }
            .into();
            // Determine selection from the actual login, never the imported activeAccountID.
            registry.active_account_id = if self.active_file().exists() {
                let actual = identity(&platform::read(&self.active_file())?)?;
                registry
                    .accounts
                    .iter()
                    .find(|p| actual.matches(p))
                    .map(|p| p.id)
            } else {
                None
            };
            write(&self.root.join("usage-cache.json"), &cache)?;
            self.save_settings(settings)?;
            self.save_registry(registry)?;
            self.cache = cache;
            Ok(())
        })();
        if let Err(e) = result {
            let recovery = (|| -> Result<()> {
                self.save_settings(old_settings)?;
                write(&self.root.join("usage-cache.json"), &old_cache)?;
                for id in &installed {
                    self.discard(*id)?;
                }
                Ok(())
            })();
            return match recovery {
                Ok(()) => Err(e),
                Err(r) => Err(anyhow::anyhow!("{e}; import cleanup also failed: {r}")),
            };
        }
        Ok(installed.len())
    }
}
pub fn write(path: &Path, value: &impl Serialize) -> Result<()> {
    platform::atomic_write(path, &serde_json::to_vec_pretty(value)?)
}
fn load_or<T: DeserializeOwned + Default>(p: &Path) -> Result<T> {
    platform::check_path(p)?;
    if !p.exists() {
        return Ok(T::default());
    }
    serde_json::from_slice(&platform::read(p)?).context("Invalid local data format")
}
fn validate_registry(r: &Registry) -> Result<()> {
    let mut ids = HashSet::new();
    for p in &r.accounts {
        ensure!(ids.insert(p.id), "Duplicate profile UUID");
        ensure!(
            p.account_id.is_some() || p.email.is_some(),
            "Profile identity missing"
        );
        let i = Identity {
            account_id: p.account_id.clone(),
            email: p.email.clone(),
        };
        if r.accounts
            .iter()
            .any(|other| other.id != p.id && i.matches(other))
        {
            bail!("Duplicate account identity");
        }
    }
    ensure!(
        r.active_account_id.is_none_or(|id| ids.contains(&id)),
        "Active profile missing"
    );
    Ok(())
}

pub trait Handoff {
    fn close(&mut self) -> Result<()>;
    fn save(&mut self) -> Result<()>;
    fn activate(&mut self) -> Result<()>;
    fn verify(&mut self) -> Result<()>;
    fn commit(&mut self) -> Result<()>;
    fn restore(&mut self) -> Result<()>;
    fn reopen(&mut self) -> Result<()>;
}
pub fn handoff(h: &mut impl Handoff) -> Result<()> {
    h.close().context("closeDesktop")?;
    h.save().context("saveCurrentCredential")?;
    h.activate().context("activateTargetCredential")?;
    for stage in ["verifyTargetIdentity", "commitActiveAccountID"] {
        let result = if stage == "verifyTargetIdentity" {
            h.verify()
        } else {
            h.commit()
        };
        if let Err(e) = result {
            return match h.restore() {
                Ok(()) => Err(e.context(stage)),
                Err(r) => Err(anyhow::anyhow!(
                    "{stage}: {e}; restoring original credential also failed: {r}"
                )),
            };
        }
    }
    h.reopen()
        .context("Account switched; reopenDesktop failed. Open Desktop manually.")
}
pub fn switch(store: &mut Store, target: Uuid) -> Result<()> {
    ensure!(
        store.registry.accounts.iter().any(|p| p.id == target),
        "Target account missing"
    );
    let desktop = platform::Desktop::discover()?;
    let original = store.registry.active_account_id;
    struct Operation<'a> {
        store: &'a mut Store,
        target: Uuid,
        original: Option<Uuid>,
        desktop: platform::Desktop,
    }
    impl Handoff for Operation<'_> {
        fn close(&mut self) -> Result<()> {
            self.desktop.close()
        }
        fn save(&mut self) -> Result<()> {
            self.store.save_current(self.original)
        }
        fn activate(&mut self) -> Result<()> {
            self.store.activate(self.target)
        }
        fn verify(&mut self) -> Result<()> {
            self.store.verify(self.target)
        }
        fn commit(&mut self) -> Result<()> {
            self.store.commit(self.target)
        }
        fn restore(&mut self) -> Result<()> {
            self.store.restore(self.original)
        }
        fn reopen(&mut self) -> Result<()> {
            self.desktop.reopen()
        }
    }
    handoff(&mut Operation {
        store,
        target,
        original,
        desktop,
    })
}
