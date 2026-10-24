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
            for line in platform::read(path)?
                .split(|c| *c == b'\n')
                .filter(|s| !s.is_empty())
            {
                let event: Event = serde_json::from_slice(line)
                    .map_err(|_| anyhow::anyhow!("Invalid experiment log"))?;
                if let (Some(id), Some(usage)) = (event.profile_id, &event.usage)
                    && usage.reset_credits.is_some()
                {
                    result.latest.insert(id, usage.clone());
                }
                if let Some(key) = event.key() {
                    result.completed.insert(key);
                }
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
