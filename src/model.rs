use anyhow::{Result, bail, ensure};
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
    #[serde(default, rename = "userID", skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
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
impl Registry {
    pub fn label(&self, profile: &Profile) -> String {
        let title = profile.title();
        let others: Vec<_> = self
            .accounts
            .iter()
            .filter(|other| other.id != profile.id && other.title().eq_ignore_ascii_case(title))
            .collect();
        if others.is_empty() {
            return title.to_owned();
        }
        if let Some(account) = profile.account_id.as_deref().map(short_id)
            && others
                .iter()
                .all(|other| other.account_id.as_deref().map(short_id).as_ref() != Some(&account))
        {
            return format!("{title} ({account})");
        }
        if let Some(user) = profile.user_id.as_deref().map(short_id)
            && others
                .iter()
                .all(|other| other.user_id.as_deref().map(short_id).as_ref() != Some(&user))
        {
            return format!("{title} ({user})");
        }
        format!("{title} ({})", profile.id)
    }
}
fn short_id(value: &str) -> String {
    let chars: Vec<_> = value.chars().collect();
    if chars.len() <= 8 {
        value.to_owned()
    } else {
        chars[chars.len() - 6..].iter().collect()
    }
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
    pub resets_at: Option<DateTime<Utc>>,
    pub five_hour_remaining_percent: Option<i32>,
    pub five_hour_resets_at: Option<DateTime<Utc>>,
    pub reset_credits_remaining: Option<i64>,
    pub reset_credits: Option<Vec<Credit>>,
}
impl Usage {
    pub(crate) fn validate(&self) -> Result<()> {
        ensure!(
            (0..=100).contains(&self.remaining_percent)
                && self
                    .five_hour_remaining_percent
                    .is_none_or(|value| (0..=100).contains(&value)),
            "Usage percentages must be between 0 and 100"
        );
        ensure!(
            self.reset_credits_remaining.is_none_or(|value| value >= 0),
            "Available reset credits must not be negative"
        );
        Ok(())
    }
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
impl Cache {
    pub(crate) fn validate(&self) -> Result<()> {
        ensure!(
            self.format_version <= FORMAT,
            "Unsupported data format version"
        );
        for entry in &self.entries {
            entry.usage.validate()?;
        }
        Ok(())
    }
}
#[derive(Clone, Debug)]
pub struct Identity {
    pub account_id: Option<String>,
    pub user_id: Option<String>,
    pub email: Option<String>,
}
impl Identity {
    pub fn matches(&self, p: &Profile) -> bool {
        if self.account_id != p.account_id {
            return false;
        }
        match (&self.user_id, &p.user_id) {
            (Some(a), Some(b)) => a == b,
            _ => matches!((&self.email, &p.email), (Some(a), Some(b)) if a.eq_ignore_ascii_case(b)),
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
            user_id: self.user_id.clone(),
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
    let email = payload.as_ref().and_then(|p| {
        nonempty(p.get("email").and_then(Value::as_str)).or_else(|| {
            nonempty(
                p.pointer("/https:~1~1api.openai.com~1profile/email")
                    .and_then(Value::as_str),
            )
        })
    });
    let user_id = payload.as_ref().and_then(|p| {
        let auth = p.get("https://api.openai.com/auth")?;
        nonempty(auth.get("chatgpt_user_id").and_then(Value::as_str))
            .or_else(|| nonempty(auth.get("user_id").and_then(Value::as_str)))
    });
    if account_id.is_none() {
        bail!("Credential identity unavailable");
    }
    Ok(Identity {
        account_id,
        user_id,
        email,
    })
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
            let at = match w.get("resetsAt") {
                None | Some(Value::Null) => None,
                Some(value) => Some(service_date(value)?),
            };
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
        five_hour_resets_at: five.and_then(|w| w.2),
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn usage_validation_accepts_boundaries_and_rejects_invalid_values() {
        let mut usage: Usage = serde_json::from_value(json!({"remainingPercent": 0})).unwrap();
        for weekly in [0, 100] {
            for five_hour in [None, Some(0), Some(100)] {
                for credits in [None, Some(0), Some(i64::MAX)] {
                    usage.remaining_percent = weekly;
                    usage.five_hour_remaining_percent = five_hour;
                    usage.reset_credits_remaining = credits;
                    usage.validate().unwrap();
                }
            }
        }
        for value in [-1, 101, i32::MIN, i32::MAX] {
            usage.remaining_percent = value;
            assert!(usage.validate().is_err());
            usage.remaining_percent = 50;
            usage.five_hour_remaining_percent = Some(value);
            assert!(usage.validate().is_err());
            usage.five_hour_remaining_percent = None;
        }
        usage.reset_credits_remaining = Some(-1);
        assert!(usage.validate().is_err());
    }

    fn window(minutes: i64, used: f64, reset: f64) -> Value {
        json!({"windowDurationMins": minutes, "usedPercent": used, "resetsAt": reset})
    }

    #[test]
    fn usage_selects_windows_by_duration_and_prefers_the_codex_bucket() {
        let five = window(300, 12.25, 1_800_000_000.25);
        let weekly = window(10080, 73.6, 1_800_100_000.5);
        for (primary, secondary) in [(five.clone(), weekly.clone()), (weekly, five)] {
            let usage = normalize(&json!({
                "rateLimitsByLimitId": {"codex": {"primary": primary, "secondary": secondary}},
                "rateLimits": {"primary": window(10080, 99.0, 1.0)}
            }))
            .unwrap();
            assert_eq!(usage.remaining_percent, 26);
            assert_eq!(usage.five_hour_remaining_percent, Some(88));
            assert_eq!(
                usage.resets_at,
                DateTime::from_timestamp(1_800_100_000, 500_000_000)
            );
            assert_eq!(
                usage.five_hour_resets_at,
                DateTime::from_timestamp(1_800_000_000, 250_000_000)
            );
        }
        let usage = normalize(&json!({"rateLimits": {
            "primary": {"durationMinutes": 10080, "usedPercent": -5, "resetsAt": 1},
            "secondary": window(299, 30.0, 2.0)
        }}))
        .unwrap();
        assert_eq!(usage.remaining_percent, 100);
        assert!(usage.five_hour_remaining_percent.is_none());
        assert!(usage.five_hour_resets_at.is_none());
        assert_eq!(
            normalize(&json!({"rateLimits": {"primary": window(10080, 120.0, 1.0)}}))
                .unwrap()
                .remaining_percent,
            0
        );
    }

    #[test]
    fn usage_rejects_missing_or_invalid_weekly_windows() {
        for bucket in [
            Value::Null,
            json!({"primary": window(300, 10.0, 1.0)}),
            json!({"primary": window(10080, 10.0, 1e30)}),
            json!({"primary": {"windowDurationMins": 10080, "usedPercent": "bad", "resetsAt": 1}}),
        ] {
            assert_eq!(
                normalize(&json!({"rateLimits": bucket}))
                    .unwrap_err()
                    .to_string(),
                "Weekly usage unavailable"
            );
        }
    }

    #[test]
    fn usage_keeps_percentages_when_reset_dates_are_unknown() {
        let usage = normalize(&json!({"rateLimits": {
            "primary": {"windowDurationMins": 10080, "usedPercent": 40, "resetsAt": null},
            "secondary": {"windowDurationMins": 300, "usedPercent": 10}
        }}))
        .unwrap();
        assert_eq!(usage.remaining_percent, 60);
        assert_eq!(usage.resets_at, None);
        assert_eq!(usage.five_hour_remaining_percent, Some(90));
        assert_eq!(usage.five_hour_resets_at, None);

        let mut saved = serde_json::to_value(&usage).unwrap();
        saved["resetsAt"] = json!("2026-11-01T12:00:00Z");
        let old: Usage = serde_json::from_value(saved).unwrap();
        assert!(old.resets_at.is_some());
    }

    #[test]
    fn reset_credits_preserve_unknown_counts_and_validate_details() {
        let mut response = json!({"rateLimits": {"primary": window(10080, 30.0, 1.0)}});
        let unknown = normalize(&response).unwrap();
        assert!(unknown.reset_credits_remaining.is_none());
        assert!(unknown.reset_credits.is_none());
        for count in [Value::Null, json!(-1), json!("bad")] {
            response["rateLimitResetCredits"] = json!({"availableCount": count, "credits": null});
            let usage = normalize(&response).unwrap();
            assert!(usage.reset_credits_remaining.is_none());
            assert!(usage.reset_credits.is_none());
        }
        response["rateLimitResetCredits"] = json!({"availableCount": 0, "credits": []});
        let empty = normalize(&response).unwrap();
        assert_eq!(empty.reset_credits_remaining, Some(0));
        assert!(empty.reset_credits.unwrap().is_empty());
        let credit = json!({"id":"credit-1", "status":"available", "resetType":"codexRateLimits", "expiresAt": 1_800_000_000.25});
        response["rateLimitResetCredits"] =
            json!({"availableCount": 3, "credits": [credit.clone()]});
        let usage = normalize(&response).unwrap();
        assert_eq!(usage.reset_credits_remaining, Some(3));
        let details = usage.reset_credits.unwrap();
        assert_eq!(details[0].id, "credit-1");
        assert_eq!(
            details[0].expires_at,
            DateTime::from_timestamp(1_800_000_000, 250_000_000)
        );
        let mut invalid_credit = credit;
        invalid_credit["expiresAt"] = json!("bad");
        for details in [json!({}), json!([{}]), json!([invalid_credit])] {
            response["rateLimitResetCredits"]["credits"] = details;
            assert!(normalize(&response).is_err());
        }
    }

    fn credential(account: &str) -> Value {
        let payload = json!({
            "email": "Person@example.test",
            "https://api.openai.com/auth": {
                "chatgpt_account_id": account, "chatgpt_plan_type": "plus",
                "chatgpt_subscription_active_until": "2026-11-01T12:00:00Z",
                "chatgpt_subscription_last_checked": "2026-10-25T10:00:00Z"
            }
        });
        // Synthetic unsigned JWT: these tests check local claims, not authentication.
        json!({"tokens": {"id_token": format!("e30.{}.fake", URL_SAFE_NO_PAD.encode(serde_json::to_vec(&payload).unwrap()))}})
    }

    #[test]
    fn jwt_claims_supply_identity_and_keep_workspaces_separate() {
        let auth = credential("workspace-one");
        let bytes = serde_json::to_vec(&auth).unwrap();
        let first = identity(&bytes).unwrap();
        assert_eq!(first.account_id.as_deref(), Some("workspace-one"));
        assert_eq!(first.email.as_deref(), Some("Person@example.test"));
        let mut profile = first.profile();
        profile.email = Some("person@EXAMPLE.test".into());
        assert!(first.matches(&profile));
        let second = identity(&serde_json::to_vec(&credential("workspace-two")).unwrap()).unwrap();
        assert!(!second.matches(&profile));
        let dates = subscription(&bytes, &profile).unwrap();
        assert_eq!(
            dates.0,
            "2026-11-01T12:00:00Z".parse::<DateTime<Utc>>().unwrap()
        );
        assert_eq!(
            dates.1,
            "2026-10-25T10:00:00Z".parse::<DateTime<Utc>>().unwrap()
        );
        profile.account_id = second.account_id;
        assert!(subscription(&bytes, &profile).is_none());
        let mut explicit = auth;
        explicit["tokens"]["account_id"] = json!("explicit-workspace");
        let bytes = serde_json::to_vec(&explicit).unwrap();
        let actual = identity(&bytes).unwrap();
        assert_eq!(actual.account_id.as_deref(), Some("explicit-workspace"));
        assert!(subscription(&bytes, &actual.profile()).is_none());
    }

    #[test]
    fn user_ids_take_priority_and_missing_identity_never_matches() {
        let first = Identity {
            account_id: Some("workspace".into()),
            user_id: Some("alice".into()),
            email: Some("old@example.test".into()),
        };
        let mut changed_email = first.clone();
        changed_email.email = Some("new@example.test".into());
        assert!(changed_email.matches(&first.profile()));
        let mut other = first.clone();
        other.user_id = Some("bob".into());
        other.email = None;
        assert!(!other.matches(&first.profile()));
        other.user_id = None;
        assert!(!other.matches(&first.profile()));
    }

    #[test]
    fn colliding_emails_get_distinct_workspace_labels() {
        let first = Identity {
            account_id: Some("team-one".into()),
            user_id: None,
            email: Some("person@example.test".into()),
        }
        .profile();
        let second = Identity {
            account_id: Some("team-two".into()),
            user_id: None,
            email: Some("person@example.test".into()),
        }
        .profile();
        let mut registry = Registry {
            accounts: vec![first.clone(), second.clone()],
            ..Default::default()
        };
        let one = registry.label(&first);
        let two = registry.label(&second);
        assert_ne!(one, two);
        assert!(one.contains("team-one"));
        assert!(two.contains("team-two"));
        registry.accounts.pop();
        assert_eq!(registry.label(&first), "person@example.test");
    }

    #[test]
    fn same_workspace_users_without_emails_have_distinct_labels() {
        let first = Identity {
            account_id: Some("workspace".into()),
            user_id: Some("alice".into()),
            email: None,
        }
        .profile();
        let second = Identity {
            account_id: Some("workspace".into()),
            user_id: Some("bob".into()),
            email: None,
        }
        .profile();
        let registry = Registry {
            accounts: vec![first.clone(), second.clone()],
            ..Default::default()
        };
        assert_eq!(registry.label(&first), "Codex Account (alice)");
        assert_eq!(registry.label(&second), "Codex Account (bob)");
    }

    #[test]
    fn malformed_jwts_fail_without_leaking_credential_contents() {
        for token in ["fake-secret", "a.%%%.c", "a.bm90LWpzb24.c", "a.e30.c.extra"] {
            let mut auth = json!({"tokens": {"id_token": token}});
            assert!(claims(&auth).is_none());
            let error = identity(&serde_json::to_vec(&auth).unwrap())
                .unwrap_err()
                .to_string();
            assert_eq!(error, "Credential identity unavailable");
            auth["tokens"]["account_id"] = json!("explicit-workspace");
            let bytes = serde_json::to_vec(&auth).unwrap();
            let actual = identity(&bytes).unwrap();
            assert!(actual.email.is_none());
            assert!(subscription(&bytes, &actual.profile()).is_none());
        }
        assert_eq!(
            identity(br#"{"fake-secret":"#).unwrap_err().to_string(),
            "Invalid credential JSON"
        );
    }
}
