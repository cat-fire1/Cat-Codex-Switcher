use crate::{model::*, platform, rpc};
use anyhow::{Context, Result, bail, ensure};
use chrono::Utc;
use serde::{Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, atomic::AtomicBool},
};
use uuid::Uuid;

pub struct Store {
    pub root: PathBuf,
    pub active: PathBuf,
    pub registry: Registry,
    pub settings: Settings,
    pub cache: Cache,
    pub warning: Option<String>,
}
#[derive(Clone, PartialEq, Eq)]
pub struct ImportPreview {
    pub titles: Vec<String>,
    pub duplicates: usize,
    pub source: PathBuf,
    fingerprint: [u8; 32],
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
        let accounts = root.join("accounts.json");
        let mut registry: Registry = load_or(&accounts).with_context(|| {
            format!(
                "Saved account list is unreadable. Rename or delete {} to reset it; saved login files stay on disk.",
                accounts.display()
            )
        })?;
        ensure!(
            registry.format_version <= FORMAT,
            "Unsupported data format version"
        );
        // Older registries did not store the user ID; recover it from their saved login.
        for p in &mut registry.accounts {
            if p.user_id.is_none()
                && let Ok(bytes) = platform::read(
                    &root
                        .join("accounts")
                        .join(p.id.to_string())
                        .join("auth.json"),
                )
                && let Ok(actual) = identity(&bytes)
                && actual.account_id == p.account_id
                && (p.email.is_none() || actual.matches(p))
            {
                p.user_id = actual.user_id;
            }
        }
        validate_registry(&registry)?;
        let (settings, settings_warning) =
            load_recovered(&root.join("settings.json"), |value: &Settings| {
                ensure!(
                    value.format_version <= FORMAT,
                    "Unsupported data format version"
                );
                Ok(())
            })?;
        let (cache, cache_warning) =
            load_recovered(&root.join("usage-cache.json"), |value: &Cache| {
                ensure!(
                    value.format_version <= FORMAT,
                    "Unsupported data format version"
                );
                Ok(())
            })?;
        let warnings: Vec<_> = [settings_warning, cache_warning]
            .into_iter()
            .flatten()
            .collect();
        Ok(Self {
            root,
            active,
            registry,
            settings,
            cache,
            warning: (!warnings.is_empty()).then(|| warnings.join("\n")),
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
        ensure!(
            current.user_id.is_some() || current.email.is_some(),
            "Credential user identity unavailable. Sign in again."
        );
        let mut r = self.registry.clone();
        let index = r
            .active_account_id
            .and_then(|id| {
                r.accounts
                    .iter()
                    .position(|p| p.id == id && current.matches(p))
            })
            .or_else(|| r.accounts.iter().position(|p| current.matches(p)));
        let id = if let Some(index) = index {
            let p = &mut r.accounts[index];
            if current.user_id.is_some() {
                p.user_id = current.user_id.clone();
            }
            if current.email.is_some() {
                p.email = current.email.clone();
            }
            p.id
        } else {
            let p = current.profile();
            let id = p.id;
            r.accounts.push(p);
            id
        };
        r.active_account_id = Some(id);
        validate_registry(&r)?;
        platform::atomic_write(&self.credential(id), &bytes)?;
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
        let credential = if self.credential(id).exists() {
            Some(platform::read(&self.credential(id))?)
        } else {
            None
        };
        let original = self.registry.clone();
        let mut r = original.clone();
        r.accounts.retain(|p| p.id != id);
        self.save_registry(r)?;
        if self.home(id).exists()
            && let Err(removal) = platform::remove_tree(&self.home(id), &self.root)
        {
            let restore = (|| -> Result<()> {
                if let Some(bytes) = credential {
                    platform::atomic_write(&self.credential(id), &bytes)?;
                }
                self.save_registry(original)
            })();
            return match restore {
                Ok(()) => Err(removal),
                Err(restore) => Err(anyhow::anyhow!(
                    "{removal}; restoring account also failed: {restore}"
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
        platform::atomic_write_external(&self.active_file(), &bytes)
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
            if p.user_id.is_none() && (p.email.is_none() || actual.matches(p)) {
                p.user_id = actual.user_id.clone();
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
        self.preview_data(source, &data)
    }
    fn preview_data(&self, source: &Path, data: &ImportData) -> Result<ImportPreview> {
        let mut titles = Vec::new();
        let mut duplicates = 0;
        for p in &data.registry.accounts {
            let i = Identity {
                account_id: p.account_id.clone(),
                user_id: p.user_id.clone(),
                email: p.email.clone(),
            };
            if self.registry.accounts.iter().any(|p| i.matches(p)) {
                duplicates += 1;
            } else {
                titles.push(data.registry.label(p));
            }
        }
        Ok(ImportPreview {
            titles,
            duplicates,
            source: source.to_owned(),
            fingerprint: Sha256::digest(serde_json::to_vec(&(
                &data.registry,
                &data.settings,
                &data.cache,
                &data.credentials,
            ))?)
            .into(),
        })
    }
    pub fn import(&mut self, preview: &ImportPreview) -> Result<usize> {
        let source = &preview.source;
        let _original = platform::OriginalGuard::acquire(source)?;
        let data = self.import_data(source)?;
        ensure!(
            self.preview_data(source, &data)? == *preview,
            "Import source changed. Preview it again before confirming."
        );
        let mut registry = self.registry.clone();
        let mut cache = self.cache.clone();
        let mut installed = Vec::new();
        let old_settings = self.settings.clone();
        let old_cache = self.cache.clone();
        let result = (|| -> Result<()> {
            for p in &data.registry.accounts {
                let i = Identity {
                    account_id: p.account_id.clone(),
                    user_id: p.user_id.clone(),
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
fn load_recovered<T: DeserializeOwned + Default>(
    p: &Path,
    validate: impl Fn(&T) -> Result<()>,
) -> Result<(T, Option<String>)> {
    platform::check_path(p)?;
    if !p.exists() {
        return Ok((T::default(), None));
    }
    let bytes = platform::read(p)?;
    let parsed = serde_json::from_slice::<T>(&bytes)
        .map_err(anyhow::Error::from)
        .and_then(|value| {
            validate(&value)?;
            Ok(value)
        });
    match parsed {
        Ok(value) => Ok((value, None)),
        Err(error) => {
            let backup = p.with_file_name(format!(
                "{}.corrupt-{}",
                p.file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("data"),
                Uuid::new_v4()
            ));
            let moved = match fs::rename(p, &backup) {
                Ok(()) => format!("moved to {}", backup.display()),
                Err(rename) => format!("could not be moved: {rename}"),
            };
            Ok((
                T::default(),
                Some(format!(
                    "Ignored invalid {}: {error}. The damaged file was {moved}.",
                    p.display()
                )),
            ))
        }
    }
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
            user_id: p.user_id.clone(),
            email: p.email.clone(),
        };
        // Older versions could save the same user twice after an email change.
        if r.accounts.iter().any(|other| {
            other.id != p.id && i.matches(other) && (p.user_id.is_none() || other.user_id.is_none())
        }) {
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
    let mut activated = false;
    let result = (|| -> Result<()> {
        h.save().context("saveCurrentCredential")?;
        h.activate().context("activateTargetCredential")?;
        activated = true;
        h.verify().context("verifyTargetIdentity")?;
        h.commit().context("commitActiveAccountID")
    })();
    if let Err(e) = result {
        if activated && let Err(r) = h.restore() {
            bail!("{e:#}; restoring original credential also failed: {r:#}");
        }
        // Failed activation is atomic; the original login is still safe to reopen.
        return match h.reopen() {
            Ok(()) => Err(e),
            Err(r) => Err(anyhow::anyhow!(
                "{e:#}; reopenDesktop also failed: {r:#}. Open Desktop manually."
            )),
        };
    }
    h.reopen()
        .context("Account switched; reopenDesktop failed. Open Desktop manually.")
}
pub fn switch(
    store: &mut Store,
    target: Uuid,
    rt: &tokio::runtime::Runtime,
    cancel: Arc<AtomicBool>,
) -> Result<()> {
    ensure!(
        store.registry.accounts.iter().any(|p| p.id == target),
        "Target account missing"
    );
    let runtime = rpc::locate()?;
    let desktop = platform::Desktop::discover()?;
    let original = store.registry.active_account_id;
    struct Operation<'a> {
        store: &'a mut Store,
        target: Uuid,
        original: Option<Uuid>,
        desktop: platform::Desktop,
        runtime: PathBuf,
        rt: &'a tokio::runtime::Runtime,
        cancel: Arc<AtomicBool>,
    }
    impl Handoff for Operation<'_> {
        fn close(&mut self) -> Result<()> {
            self.desktop.close()
        }
        fn save(&mut self) -> Result<()> {
            self.store.save_current(self.original)
        }
        fn activate(&mut self) -> Result<()> {
            let profile = self
                .store
                .registry
                .accounts
                .iter()
                .find(|p| p.id == self.target)
                .context("Target account missing")?;
            // Refresh the saved home so failed verification retains any rotated tokens.
            self.rt.block_on(rpc::verify_login(
                &self.runtime,
                &self.store.home(self.target),
                profile,
                self.cancel.clone(),
            ))?;
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
        runtime,
        rt,
        cancel,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn credential(account_id: &str, user_id: &str, email: Option<&str>) -> Vec<u8> {
        use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
        let claims = serde_json::json!({
            "email": email,
            "https://api.openai.com/auth": {"chatgpt_user_id": user_id}
        });
        serde_json::to_vec(&serde_json::json!({"tokens": {
            "account_id": account_id,
            "id_token": format!("a.{}.b", URL_SAFE_NO_PAD.encode(claims.to_string()))
        }}))
        .unwrap()
    }

    fn saved_account(store: &mut Store, account_id: &str) -> Profile {
        let bytes = credential(account_id, "test-user", None);
        let mut profile = identity(&bytes).unwrap().profile();
        profile.display_name = "Same name".into();
        platform::atomic_write(&store.credential(profile.id), &bytes).unwrap();
        store.add(profile.clone()).unwrap();
        profile
    }

    #[test]
    fn sync_preserves_workspace_users_and_migrates_old_profiles() {
        let temp = std::env::temp_dir();
        let root = temp.join(format!("switcher-identities-{}", Uuid::new_v4()));
        let mut store = Store::load(root.join("store"), root.join("active")).unwrap();
        let alice = credential("workspace", "alice", Some("old@example.test"));
        platform::atomic_write(&store.active_file(), &alice).unwrap();
        store.sync_current().unwrap();
        let alice_id = store.registry.active_account_id.unwrap();

        let mut legacy = store.registry.clone();
        legacy.accounts[0].user_id = None;
        store.save_registry(legacy).unwrap();
        drop(store);
        let mut store = Store::load(root.join("store"), root.join("active")).unwrap();
        assert_eq!(store.registry.accounts[0].user_id.as_deref(), Some("alice"));

        let renamed = credential("workspace", "alice", Some("new@example.test"));
        platform::atomic_write(&store.active_file(), &renamed).unwrap();
        store.sync_current().unwrap();
        assert_eq!(store.registry.accounts.len(), 1);
        assert_eq!(store.registry.active_account_id, Some(alice_id));
        assert_eq!(
            store.registry.accounts[0].email.as_deref(),
            Some("new@example.test")
        );

        let bob = credential("workspace", "bob", None);
        platform::atomic_write(&store.active_file(), &bob).unwrap();
        store.sync_current().unwrap();
        assert_eq!(store.registry.accounts.len(), 2);
        assert_ne!(store.registry.active_account_id, Some(alice_id));
        assert_eq!(
            platform::read(&store.credential(alice_id)).unwrap(),
            renamed
        );

        platform::atomic_write(
            &store.active_file(),
            br#"{"tokens":{"account_id":"workspace"}}"#,
        )
        .unwrap();
        assert!(store.sync_current().is_err());
        assert_eq!(store.registry.accounts.len(), 2);
        assert_eq!(
            platform::read(&store.credential(alice_id)).unwrap(),
            renamed
        );
        platform::remove_tree(&root, &temp).unwrap();
    }

    #[test]
    fn legacy_duplicate_users_remain_accessible_and_removable() {
        let temp = std::env::temp_dir();
        let root = temp.join(format!("switcher-duplicates-{}", Uuid::new_v4()));
        let mut store = Store::load(root.join("store"), root.join("active")).unwrap();
        let old = credential("workspace", "alice", Some("old@example.test"));
        let renamed = credential("workspace", "alice", Some("new@example.test"));
        let mut first = identity(&old).unwrap().profile();
        let mut second = identity(&renamed).unwrap().profile();
        first.user_id = None;
        second.user_id = None;
        platform::atomic_write(&store.credential(first.id), &old).unwrap();
        platform::atomic_write(&store.credential(second.id), &renamed).unwrap();
        platform::atomic_write(&store.active_file(), &renamed).unwrap();
        store
            .save_registry(Registry {
                format_version: FORMAT,
                active_account_id: Some(first.id),
                accounts: vec![first.clone(), second.clone()],
            })
            .unwrap();
        drop(store);

        let mut store = Store::load(root.join("store"), root.join("active")).unwrap();
        store.sync_current().unwrap();
        assert_eq!(store.registry.active_account_id, Some(first.id));
        assert_eq!(store.registry.accounts.len(), 2);
        assert_ne!(
            store.registry.label(&store.registry.accounts[0]),
            store.registry.label(&store.registry.accounts[1])
        );
        store.remove(second.id).unwrap();
        assert_eq!(store.registry.accounts.len(), 1);
        assert_eq!(
            platform::read(&store.credential(first.id)).unwrap(),
            renamed
        );
        platform::remove_tree(&root, &temp).unwrap();
    }

    #[test]
    fn legacy_profile_does_not_adopt_an_unverifiable_user_id() {
        let temp = std::env::temp_dir();
        let root = temp.join(format!("switcher-legacy-identity-{}", Uuid::new_v4()));
        let mut source = Store::load(root.join("source"), root.join("unused")).unwrap();
        let alice = credential("workspace", "alice", Some("alice@example.test"));
        let mut profile = identity(&alice).unwrap().profile();
        profile.user_id = None;
        let bob = credential("workspace", "bob", None);
        platform::atomic_write(&source.credential(profile.id), &bob).unwrap();
        source
            .save_registry(Registry {
                format_version: FORMAT,
                active_account_id: None,
                accounts: vec![profile],
            })
            .unwrap();
        let loaded = Store::load(source.root.clone(), source.active.clone()).unwrap();
        assert!(loaded.registry.accounts[0].user_id.is_none());
        let destination = Store::load(root.join("destination"), root.join("active")).unwrap();
        assert!(destination.preview(&source.root).is_err());
        assert!(destination.registry.accounts.is_empty());
        platform::remove_tree(&root, &temp).unwrap();
    }

    #[test]
    fn corrupt_settings_and_cache_are_recovered_but_accounts_stay_strict() {
        let temp = std::env::temp_dir();
        let root = temp.join(format!("switcher-recovery-{}", Uuid::new_v4()));
        let store_root = root.join("store");
        fs::create_dir_all(&store_root).unwrap();
        fs::write(
            store_root.join("settings.json"),
            br#"{"formatVersion": 999}"#,
        )
        .unwrap();
        fs::write(store_root.join("usage-cache.json"), b"{").unwrap();
        let store = Store::load(store_root.clone(), root.join("active")).unwrap();
        assert_eq!(store.settings.format_version, FORMAT);
        assert!(store.cache.entries.is_empty());
        let warning = store.warning.unwrap();
        assert!(warning.contains("settings.json"));
        assert!(warning.contains("usage-cache.json"));
        let backups = fs::read_dir(&store_root)
            .unwrap()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_name().to_string_lossy().contains(".corrupt-"))
            .count();
        assert_eq!(backups, 2);
        fs::write(store_root.join("accounts.json"), b"{").unwrap();
        let error = match Store::load(store_root, root.join("active")) {
            Ok(_) => panic!("corrupt accounts.json must not be ignored"),
            Err(error) => format!("{error:#}"),
        };
        assert!(error.contains("Rename or delete"));
        platform::remove_tree(&root, &temp).unwrap();
    }

    #[test]
    fn failed_removal_restores_credentials_before_restoring_the_profile() {
        use std::os::windows::fs::OpenOptionsExt;
        use windows::Win32::Storage::FileSystem::{FILE_SHARE_READ, FILE_SHARE_WRITE};

        let temp = std::env::temp_dir();
        let root = temp.join(format!("switcher-removal-{}", Uuid::new_v4()));
        let mut store = Store::load(root.join("store"), root.join("active")).unwrap();
        let active = credential("active", "test-user", None);
        platform::atomic_write(&store.active_file(), &active).unwrap();
        store.sync_current().unwrap();
        let profile = saved_account(&mut store, "inactive");
        let credential = platform::read(&store.credential(profile.id)).unwrap();
        let registry = platform::read(&store.root.join("accounts.json")).unwrap();
        let deleted_first = store.home(profile.id).join("aaa-runtime-data");
        fs::write(&deleted_first, b"disposable data").unwrap();
        let locked_path = store.home(profile.id).join("zzz-locked-runtime-data");
        fs::write(&locked_path, b"locked data").unwrap();
        let locked = fs::OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ.0 | FILE_SHARE_WRITE.0)
            .open(&locked_path)
            .unwrap();
        assert!(store.remove(profile.id).is_err());
        assert!(
            !deleted_first.exists(),
            "The deletion must have partially completed"
        );
        assert_eq!(
            platform::read(&store.credential(profile.id)).unwrap(),
            credential
        );
        assert_eq!(
            platform::read(&store.root.join("accounts.json")).unwrap(),
            registry
        );
        assert_eq!(platform::read(&store.active_file()).unwrap(), active);
        let reloaded = Store::load(store.root.clone(), store.active.clone()).unwrap();
        assert!(
            reloaded
                .registry
                .accounts
                .iter()
                .any(|p| p.id == profile.id)
        );
        drop(locked);
        store.remove(profile.id).unwrap();
        assert!(!store.home(profile.id).exists());
        assert!(!store.registry.accounts.iter().any(|p| p.id == profile.id));
        platform::remove_tree(&root, &temp).unwrap();
    }

    #[test]
    fn import_rejects_changed_identity_with_the_same_title() {
        let temp = std::env::temp_dir();
        let root = temp.join(format!("switcher-import-{}", Uuid::new_v4()));
        let mut destination = Store::load(root.join("destination"), root.join("active")).unwrap();
        let mut source = Store::load(root.join("source"), root.join("unused")).unwrap();
        let mut profile = saved_account(&mut source, "workspace-one");
        let preview = destination.preview(&source.root).unwrap();
        profile.account_id = Some("workspace-two".into());
        platform::atomic_write(
            &source.credential(profile.id),
            &credential("workspace-two", "test-user", None),
        )
        .unwrap();
        let mut registry = source.registry.clone();
        registry.accounts = vec![profile.clone()];
        source.save_registry(registry).unwrap();
        let updated = destination.preview(&source.root).unwrap();
        assert_eq!(preview.titles, updated.titles);
        assert_eq!(preview.duplicates, updated.duplicates);
        assert!(
            destination
                .import(&preview)
                .unwrap_err()
                .to_string()
                .contains("source changed")
        );
        assert!(destination.registry.accounts.is_empty());
        assert!(!destination.home(profile.id).exists());
        assert_eq!(destination.import(&updated).unwrap(), 1);
        assert_eq!(
            destination.registry.accounts[0].account_id,
            profile.account_id
        );
        assert!(!destination.active_file().exists());
        platform::remove_tree(&root, &temp).unwrap();
    }

    #[test]
    fn import_restores_files_when_the_final_registry_write_fails() {
        use std::os::windows::fs::OpenOptionsExt;
        use windows::Win32::Storage::FileSystem::{FILE_SHARE_READ, FILE_SHARE_WRITE};

        let temp = std::env::temp_dir();
        let root = temp.join(format!("switcher-import-{}", Uuid::new_v4()));
        let mut destination = Store::load(root.join("destination"), root.join("active")).unwrap();
        let mut source = Store::load(root.join("source"), root.join("unused")).unwrap();
        let active = credential("original", "test-user", None);
        platform::atomic_write(&destination.active_file(), &active).unwrap();
        destination.sync_current().unwrap();
        destination.save_settings(Settings::default()).unwrap();
        write(
            &destination.root.join("usage-cache.json"),
            &destination.cache,
        )
        .unwrap();
        let imported = saved_account(&mut source, "imported");
        let mut settings = source.settings.clone();
        settings.language = "en".into();
        source.save_settings(settings).unwrap();
        source
            .cache(
                imported.id,
                Usage {
                    remaining_percent: 23,
                    resets_at: Some(Utc::now()),
                    five_hour_remaining_percent: None,
                    five_hour_resets_at: None,
                    reset_credits_remaining: None,
                    reset_credits: None,
                },
            )
            .unwrap();
        let preview = destination.preview(&source.root).unwrap();
        let paths = ["accounts.json", "settings.json", "usage-cache.json"];
        let before: Vec<_> = paths
            .iter()
            .map(|name| platform::read(&destination.root.join(name)).unwrap())
            .collect();
        let source_before = platform::read(&source.credential(imported.id)).unwrap();
        // Denying delete-sharing allows reads but makes atomic replacement fail at commit.
        let locked = fs::OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ.0 | FILE_SHARE_WRITE.0)
            .open(destination.root.join("accounts.json"))
            .unwrap();
        assert!(destination.import(&preview).is_err());
        drop(locked);
        for (name, bytes) in paths.iter().zip(before) {
            assert_eq!(platform::read(&destination.root.join(name)).unwrap(), bytes);
        }
        assert_eq!(platform::read(&destination.active_file()).unwrap(), active);
        assert_eq!(
            platform::read(&source.credential(imported.id)).unwrap(),
            source_before
        );
        assert!(!destination.home(imported.id).exists());
        assert_eq!(destination.settings.language, "uk");
        assert!(destination.cache.entries.is_empty());
        let reloaded = Store::load(destination.root.clone(), destination.active.clone()).unwrap();
        assert_eq!(reloaded.registry.accounts.len(), 1);
        assert_eq!(
            reloaded.registry.active_account_id,
            destination.registry.active_account_id
        );
        // The same preview remains usable after the transient write failure is gone.
        assert_eq!(destination.import(&preview).unwrap(), 1);
        platform::remove_tree(&root, &temp).unwrap();
    }

    #[derive(Default)]
    struct FakeHandoff {
        failures: Vec<&'static str>,
        calls: Vec<&'static str>,
    }
    impl FakeHandoff {
        fn step(&mut self, name: &'static str) -> Result<()> {
            self.calls.push(name);
            ensure!(!self.failures.contains(&name), "failed {name}");
            Ok(())
        }
    }
    impl Handoff for FakeHandoff {
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
            self.step("restore")
        }
        fn reopen(&mut self) -> Result<()> {
            self.step("reopen")
        }
    }

    #[test]
    fn handoff_recovers_each_failure_boundary() {
        for (failure, expected) in [
            ("close", vec!["close"]),
            ("save", vec!["close", "save", "reopen"]),
            ("activate", vec!["close", "save", "activate", "reopen"]),
            (
                "verify",
                vec!["close", "save", "activate", "verify", "restore", "reopen"],
            ),
            (
                "commit",
                vec![
                    "close", "save", "activate", "verify", "commit", "restore", "reopen",
                ],
            ),
        ] {
            let mut h = FakeHandoff {
                failures: vec![failure],
                ..Default::default()
            };
            let error = handoff(&mut h).unwrap_err();
            assert!(format!("{error:#}").contains(&format!("failed {failure}")));
            assert_eq!(h.calls, expected);
        }
    }

    #[test]
    fn handoff_preserves_recovery_errors_and_stops_after_failed_restore() {
        for recovery in ["restore", "reopen"] {
            let mut h = FakeHandoff {
                failures: vec!["verify", recovery],
                ..Default::default()
            };
            let error = format!("{:#}", handoff(&mut h).unwrap_err());
            assert!(error.contains("failed verify"));
            assert!(error.contains(&format!("failed {recovery}")));
            assert_eq!(h.calls.last(), Some(&recovery));
        }
        let mut h = FakeHandoff::default();
        handoff(&mut h).unwrap();
        assert_eq!(
            h.calls,
            ["close", "save", "activate", "verify", "commit", "reopen"]
        );
    }
}
