use crate::{
    model::{Credit, Profile, Usage},
    platform,
};
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    io::Write,
    path::Path,
};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub mode: String,
    pub reset_attempted: bool,
    pub timestamp: DateTime<Utc>,
    pub event: String,
    #[serde(rename = "profileID")]
    pub profile_id: Option<Uuid>,
    pub account: Option<String>,
    pub usage: Option<Usage>,
    pub error: Option<String>,
    #[serde(rename = "creditID")]
    pub credit_id: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub checkpoint_minutes: Option<i64>,
    pub scheduled_at: Option<DateTime<Utc>>,
    pub delay_seconds: Option<i64>,
    pub decision: Option<String>,
}
impl Event {
    pub fn marker(event: &str, now: DateTime<Utc>) -> Self {
        Self {
            mode: "dry_run".into(),
            reset_attempted: false,
            timestamp: now,
            event: event.into(),
            profile_id: None,
            account: None,
            usage: None,
            error: None,
            credit_id: None,
            expires_at: None,
            checkpoint_minutes: None,
            scheduled_at: None,
            delay_seconds: None,
            decision: None,
        }
    }
    fn key(&self) -> Option<String> {
        Some(format!(
            "{}/{}/{}/{}",
            self.profile_id?,
            self.credit_id.as_ref()?,
            self.expires_at?.timestamp(),
            self.checkpoint_minutes?
        ))
    }
}
#[derive(Default)]
pub struct Experiment {
    latest: HashMap<Uuid, Usage>,
    completed: HashSet<String>,
}
impl Experiment {
    pub fn load(path: &Path) -> Result<Self> {
        let mut result = Self::default();
        platform::check_path(path)?;
        if path.exists() {
            let bytes = platform::read(path)?;
            let mut offset = 0;
            for line in bytes.split_inclusive(|c| *c == b'\n') {
                let event: Event = match serde_json::from_slice(line) {
                    Ok(event) => event,
                    Err(_) if line.iter().all(u8::is_ascii_whitespace) => {
                        offset += line.len();
                        continue;
                    }
                    Err(e) if !line.ends_with(b"\n") && e.is_eof() => {
                        // Keep the interrupted tail before repairing the append-only log.
                        let backup =
                            path.with_extension(format!("jsonl.corrupt-{}", Uuid::new_v4()));
                        platform::atomic_write(&backup, line)?;
                        platform::atomic_write(path, &bytes[..offset])?;
                        break;
                    }
                    Err(_) => anyhow::bail!("Invalid experiment log"),
                };
                if let (Some(id), Some(usage)) = (event.profile_id, &event.usage)
                    && usage.reset_credits.is_some()
                {
                    result.latest.insert(id, usage.clone());
                }
                if let Some(key) = event.key() {
                    result.completed.insert(key);
                }
                offset += line.len();
            }
            if offset == bytes.len() && !bytes.is_empty() && !bytes.ends_with(b"\n") {
                let mut terminated = bytes;
                terminated.push(b'\n');
                platform::atomic_write(path, &terminated)?;
            }
        }
        Ok(result)
    }
    pub fn next(&self, ids: &HashSet<Uuid>) -> Option<DateTime<Utc>> {
        self.latest
            .iter()
            .filter(|(id, _)| ids.contains(id))
            .flat_map(|(id, u)| {
                u.reset_credits
                    .iter()
                    .flatten()
                    .filter(|c| c.status == "available" && c.reset_type == "codexRateLimits")
                    .flat_map(move |c| {
                        [10, 5, 1].into_iter().filter_map(move |m| {
                            let event = checkpoint(*id, c, m, Utc::now())?;
                            if self.completed.contains(&event.key()?) {
                                None
                            } else {
                                event.scheduled_at
                            }
                        })
                    })
            })
            .min()
    }
    pub fn observe(
        &mut self,
        profile: &Profile,
        usage: Option<&Usage>,
        error: Option<String>,
        now: DateTime<Utc>,
    ) -> Vec<Event> {
        let mut snapshot = Event::marker(
            if usage.is_some() {
                "snapshot"
            } else {
                "read_failed"
            },
            now,
        );
        snapshot.profile_id = Some(profile.id);
        snapshot.account = Some(profile.display_name.clone());
        snapshot.usage = usage.cloned();
        snapshot.error = error.clone();
        let mut result = vec![snapshot];
        let current = usage
            .and_then(|u| u.reset_credits.as_ref())
            .cloned()
            .unwrap_or_default();
        let mut candidates = current.clone();
        if let Some(previous) = self
            .latest
            .get(&profile.id)
            .and_then(|u| u.reset_credits.as_ref())
        {
            candidates.extend(
                previous
                    .iter()
                    .filter(|c| !current.iter().any(|n| n.id == c.id))
                    .cloned(),
            );
        }
        for credit in candidates
            .iter()
            .filter(|c| c.reset_type == "codexRateLimits")
        {
            for (index, minutes) in [10, 5, 1].into_iter().enumerate() {
                let Some(mut e) = checkpoint(profile.id, credit, minutes, now) else {
                    continue;
                };
                let Some(key) = e.key() else {
                    continue;
                };
                if self.completed.contains(&key) || e.scheduled_at.is_none_or(|at| now < at) {
                    continue;
                }
                let next = credit.expires_at.expect("checkpoint has expiry")
                    - chrono::Duration::minutes([5, 1, 0][index]);
                e.account = Some(profile.display_name.clone());
                e.usage = usage.cloned();
                e.error = error.clone();
                e.decision = Some(
                    if now >= next {
                        "missed"
                    } else if usage.is_none() {
                        "read_failed"
                    } else if usage.is_some_and(|u| u.reset_credits.is_none()) {
                        "details_unavailable"
                    } else if !current.iter().any(|c| c.id == credit.id)
                        || credit.status != "available"
                    {
                        "credit_unavailable"
                    } else {
                        "would_attempt"
                    }
                    .into(),
                );
                self.completed.insert(key);
                result.push(e);
            }
        }
        if let Some(u) = usage.filter(|u| u.reset_credits.is_some()) {
            self.latest.insert(profile.id, u.clone());
        }
        result
    }
}
fn checkpoint(id: Uuid, credit: &Credit, minutes: i64, now: DateTime<Utc>) -> Option<Event> {
    let at = credit.expires_at? - chrono::Duration::minutes(minutes);
    let mut e = Event::marker("checkpoint", now);
    e.profile_id = Some(id);
    e.credit_id = Some(credit.id.clone());
    e.expires_at = credit.expires_at;
    e.checkpoint_minutes = Some(minutes);
    e.scheduled_at = Some(at);
    e.delay_seconds = Some((now - at).num_seconds().max(0));
    Some(e)
}
pub fn append(path: &Path, events: &[Event]) -> Result<()> {
    platform::check_path(path)?;
    if !path.exists() {
        platform::atomic_write(path, b"")?;
    }
    platform::restrict(path)?;
    let mut file = std::fs::OpenOptions::new().append(true).open(path)?;
    for event in events {
        file.write_all(&serde_json::to_vec(event)?)?;
        file.write_all(b"\n")?;
    }
    file.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interrupted_tail_is_preserved_and_checkpoints_survive_repair() {
        let temp = std::env::temp_dir();
        let root = temp.join(format!("switcher-log-{}", Uuid::new_v4()));
        let path = root.join("experiment.jsonl");
        let mut event = Event::marker("checkpoint", Utc::now());
        event.profile_id = Some(Uuid::new_v4());
        event.credit_id = Some("credit".into());
        event.expires_at = Some(Utc::now());
        event.checkpoint_minutes = Some(10);
        append(&path, &[event.clone()]).unwrap();
        let prefix = platform::read(&path).unwrap();
        let tail = br#"{"event":"snapshot","timestamp":"#;
        let mut interrupted = prefix.clone();
        interrupted.extend_from_slice(tail);
        platform::atomic_write(&path, &interrupted).unwrap();
        let recovered = Experiment::load(&path).unwrap();
        assert!(recovered.completed.contains(&event.key().unwrap()));
        assert_eq!(platform::read(&path).unwrap(), prefix);
        let backup = std::fs::read_dir(&root)
            .unwrap()
            .map(|e| e.unwrap().path())
            .find(|p| p != &path)
            .unwrap();
        assert_eq!(platform::read(&backup).unwrap(), tail);
        append(&path, &[Event::marker("resumed", Utc::now())]).unwrap();
        assert!(
            Experiment::load(&path)
                .unwrap()
                .completed
                .contains(&event.key().unwrap())
        );
        platform::remove_tree(&root, &temp).unwrap();
    }

    #[test]
    fn complete_final_event_gets_a_separator_but_corrupt_complete_lines_are_rejected() {
        let temp = std::env::temp_dir();
        let root = temp.join(format!("switcher-log-{}", Uuid::new_v4()));
        let path = root.join("experiment.jsonl");
        let event = serde_json::to_vec(&Event::marker("enabled", Utc::now())).unwrap();
        platform::atomic_write(&path, &event).unwrap();
        Experiment::load(&path).unwrap();
        append(&path, &[Event::marker("resumed", Utc::now())]).unwrap();
        let bytes = platform::read(&path).unwrap();
        assert_eq!(
            bytes
                .split(|b| *b == b'\n')
                .filter(|s| !s.is_empty())
                .count(),
            2
        );
        Experiment::load(&path).unwrap();
        let corrupt = b"{broken}\n";
        platform::atomic_write(&path, corrupt).unwrap();
        assert!(Experiment::load(&path).is_err());
        assert_eq!(platform::read(&path).unwrap(), corrupt);
        platform::remove_tree(&root, &temp).unwrap();
    }
}
