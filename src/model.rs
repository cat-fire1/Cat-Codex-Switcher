use anyhow::{Result, bail};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

pub const FORMAT: u32 = 1;
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: Uuid,
    pub display_name: String,
    pub email: Option<String>,
    #[serde(rename = "accountID")]
    pub account_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
}
impl Profile {
    pub fn title(&self) -> &str {
        self.email.as_deref().unwrap_or(&self.display_name)
    }
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Registry {
    #[serde(default = "format")]
    pub format_version: u32,
    #[serde(rename = "activeAccountID")]
    pub active_account_id: Option<Uuid>,
    pub accounts: Vec<Profile>,
}
fn format() -> u32 {
    FORMAT
}
#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub format_version: u32,
    pub language: String,
    pub shows_menu_bar_percentage: bool,
    pub shows_five_hour_usage: bool,
    pub status_bar_usage_window: String,
    pub reset_expiry_experiment_enabled: bool,
    pub launch_at_login: bool,
    pub automatically_check_updates: bool,
    pub update_repository: String,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            format_version: FORMAT,
            language: "uk".into(),
            shows_menu_bar_percentage: true,
            shows_five_hour_usage: false,
            status_bar_usage_window: "fiveHour".into(),
            reset_expiry_experiment_enabled: false,
            launch_at_login: false,
            automatically_check_updates: true,
            update_repository: String::new(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Credit {
    pub id: String,
    pub status: String,
    pub reset_type: String,
    pub expires_at: Option<DateTime<Utc>>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    pub remaining_percent: i32,
    pub resets_at: DateTime<Utc>,
    pub five_hour_remaining_percent: Option<i32>,
    pub five_hour_resets_at: Option<DateTime<Utc>>,
    pub reset_credits_remaining: Option<i64>,
    pub reset_credits: Option<Vec<Credit>>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheEntry {
    #[serde(rename = "profileID")]
    pub profile_id: Uuid,
    pub usage: Usage,
    pub fetched_at: DateTime<Utc>,
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cache {
    #[serde(default = "format")]
    pub format_version: u32,
    pub entries: Vec<CacheEntry>,
}
#[derive(Clone, Debug)]
pub struct Identity {
    pub account_id: Option<String>,
    pub email: Option<String>,
}
impl Identity {
    pub fn matches(&self, p: &Profile) -> bool {
        match (&self.account_id, &p.account_id) {
            (Some(a), Some(b)) => {
                a == b
                    && match (&self.email, &p.email) {
                        (Some(a), Some(b)) => a.eq_ignore_ascii_case(b),
                        _ => true,
                    }
            }
            (None, None) => {
                matches!((&self.email, &p.email), (Some(a), Some(b)) if a.eq_ignore_ascii_case(b))
            }
            _ => false,
        }
    }
    pub fn profile(&self) -> Profile {
        Profile {
            id: Uuid::new_v4(),
            display_name: self
                .email
                .as_deref()
                .and_then(|e| e.split('@').next())
                .unwrap_or("Codex Account")
                .into(),
            email: self.email.clone(),
            account_id: self.account_id.clone(),
            created_at: Utc::now(),
            last_used_at: None,
        }
    }
}
fn nonempty(v: Option<&str>) -> Option<String> {
    v.filter(|s| !s.trim().is_empty()).map(str::to_owned)
}
pub fn claims(auth: &Value) -> Option<Value> {
    let parts: Vec<_> = auth
        .pointer("/tokens/id_token")?
        .as_str()?
        .split('.')
        .collect();
    if parts.len() != 3 {
        return None;
    }
    serde_json::from_slice(
        &URL_SAFE_NO_PAD
            .decode(parts[1].trim_end_matches('='))
            .ok()?,
    )
    .ok()
}
pub fn identity(bytes: &[u8]) -> Result<Identity> {
    // Never expose JSON decoder diagnostics: they can contain credential bytes.
    let auth: Value =
        serde_json::from_slice(bytes).map_err(|_| anyhow::anyhow!("Invalid credential JSON"))?;
    let payload = claims(&auth);
    let account_id =
        nonempty(auth.pointer("/tokens/account_id").and_then(Value::as_str)).or_else(|| {
            nonempty(
                payload
                    .as_ref()?
                    .get("https://api.openai.com/auth")?
                    .get("chatgpt_account_id")?
                    .as_str(),
            )
        });
    let email = nonempty(
        payload
            .as_ref()
            .and_then(|p| p.get("email"))
            .and_then(Value::as_str),
    );
    if account_id.is_none() {
        bail!("Credential identity unavailable");
    }
    Ok(Identity { account_id, email })
}
pub fn subscription(bytes: &[u8], profile: &Profile) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
    if !identity(bytes).ok()?.matches(profile) {
        return None;
    }
    let auth: Value = serde_json::from_slice(bytes).ok()?;
    let payload = claims(&auth)?;
    let a = payload.get("https://api.openai.com/auth")?;
    if a.get("chatgpt_plan_type")
        .and_then(Value::as_str)
        .is_some_and(|p| p.eq_ignore_ascii_case("free"))
    {
        return None;
    }
    if profile.account_id.as_deref() != a.get("chatgpt_account_id").and_then(Value::as_str) {
        return None;
    }
    Some((
        a.get("chatgpt_subscription_active_until")?
            .as_str()?
            .parse()
            .ok()?,
        a.get("chatgpt_subscription_last_checked")?
            .as_str()?
            .parse()
            .ok()?,
    ))
}
pub fn normalize(v: &Value) -> Result<Usage> {
    let bucket = v
        .pointer("/rateLimitsByLimitId/codex")
        .or_else(|| v.get("rateLimits"))
        .ok_or_else(|| anyhow::anyhow!("Weekly usage unavailable"))?;
    let windows: Vec<_> = ["primary", "secondary"]
        .iter()
        .filter_map(|k| {
            let w = bucket.get(k)?;
            let duration = w
                .get("windowDurationMins")
                .or_else(|| w.get("durationMinutes"))?
                .as_i64()?;
            let used = w.get("usedPercent")?.as_f64()?;
            let at = service_date(w.get("resetsAt")?)?;
            Some((
                duration,
                (100.0 - used).round().clamp(0.0, 100.0) as i32,
                at,
            ))
        })
        .collect();
    let weekly = windows
        .iter()
        .filter(|w| (8640..=11520).contains(&w.0))
        .max_by_key(|w| w.0)
        .ok_or_else(|| anyhow::anyhow!("Weekly usage unavailable"))?;
    let five = windows.iter().find(|w| w.0 == 300);
    let credits = v
        .pointer("/rateLimitResetCredits/credits")
        .filter(|v| !v.is_null())
        .map(|rows| -> Result<Vec<Credit>> {
            rows.as_array()
                .ok_or_else(|| anyhow::anyhow!("Invalid reset credits"))?
                .iter()
                .map(|r| {
                    let text = |k| {
                        r.get(k)
                            .and_then(Value::as_str)
                            .filter(|s| !s.is_empty())
                            .map(str::to_owned)
                            .ok_or_else(|| anyhow::anyhow!("Invalid reset credit"))
                    };
                    let expires_at = match r.get("expiresAt") {
                        None | Some(Value::Null) => None,
                        Some(n) => Some(
                            service_date(n)
                                .ok_or_else(|| anyhow::anyhow!("Invalid credit expiry"))?,
                        ),
                    };
                    Ok(Credit {
                        id: text("id")?,
                        status: text("status")?,
                        reset_type: text("resetType")?,
                        expires_at,
                    })
                })
                .collect()
        })
        .transpose()?;
    Ok(Usage {
        remaining_percent: weekly.1,
        resets_at: weekly.2,
        five_hour_remaining_percent: five.map(|w| w.1),
        five_hour_resets_at: five.map(|w| w.2),
        reset_credits_remaining: v
            .pointer("/rateLimitResetCredits/availableCount")
            .and_then(Value::as_i64)
            .filter(|n| *n >= 0),
        reset_credits: credits,
    })
}
fn service_date(v: &Value) -> Option<DateTime<Utc>> {
    let seconds = v.as_f64()?;
    if !seconds.is_finite() || seconds < i64::MIN as f64 || seconds >= i64::MAX as f64 {
        return None;
    }
    let whole = seconds.floor();
    let nanos = ((seconds - whole) * 1_000_000_000.0).floor() as u32;
    DateTime::from_timestamp(whole as i64, nanos)
}
