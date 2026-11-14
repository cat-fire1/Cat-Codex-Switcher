use anyhow::{Result, ensure};
use serde_json::Value;
pub fn validate_release_url(url: &str) -> Result<()> {
    let parsed = reqwest::Url::parse(url)?;
    let parts: Vec<_> = parsed.path().split('/').filter(|p| !p.is_empty()).collect();
    ensure!(
        parsed.scheme() == "https"
            && parsed.host_str() == Some("github.com")
            && parsed.username().is_empty()
            && parsed.password().is_none()
            && parsed.port().is_none()
            && parsed.query().is_none()
            && parsed.fragment().is_none()
            && parts.len() == 5
            && parts[2] == "releases"
            && parts[3] == "tag"
            && version(parts[4]).is_some(),
        "Unsupported release URL"
    );
    repository(&format!("{}/{}", parts[0], parts[1]))?;
    Ok(())
}
pub fn repository(value: &str) -> Result<Option<String>> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    let parts: Vec<_> = value.split('/').collect();
    ensure!(
        parts.len() == 2
            && parts.iter().all(|p| !p.is_empty()
                && *p != "."
                && *p != ".."
                && p.chars()
                    .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))),
        "Use owner/repository for the update source"
    );
    Ok(Some(value.to_owned()))
}
fn version(tag: &str) -> Option<[u64; 3]> {
    let parts: Vec<_> = tag.strip_prefix('v')?.split('.').collect();
    if parts.len() != 3
        || parts.iter().any(|p| {
            p.is_empty()
                || (p.len() > 1 && p.starts_with('0'))
                || !p.chars().all(|c| c.is_ascii_digit())
        })
    {
        return None;
    }
    Some([
        parts[0].parse().ok()?,
        parts[1].parse().ok()?,
        parts[2].parse().ok()?,
    ])
}
fn current_version(value: &str) -> Option<[u64; 3]> {
    let core = value.split(['-', '+']).next()?;
    version(&format!("v{core}"))
}
pub fn latest(rows: &[Value]) -> Option<([u64; 3], String)> {
    rows.iter()
        .filter(|r| {
            r.get("draft").and_then(Value::as_bool) == Some(false)
                && r.get("prerelease").and_then(Value::as_bool) == Some(false)
        })
        .filter(|r| {
            r.get("assets")
                .and_then(Value::as_array)
                .is_some_and(|assets| {
                    assets.iter().any(|a| {
                        a.get("name").and_then(Value::as_str) == Some("Cat-Codex-Switcher.exe")
                    })
                })
        })
        .filter_map(|r| {
            let tag = r.get("tag_name")?.as_str()?;
            Some((version(tag)?, tag.to_owned()))
        })
        .max_by_key(|v| v.0)
}
pub async fn check(repo: &str) -> Result<Option<String>> {
    let Some(repo) = repository(repo)? else {
        return Ok(None);
    };
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .user_agent(concat!("CatCodexSwitcher/", env!("CARGO_PKG_VERSION")))
        .build()?;
    let mut best = None;
    for page in 1..=100 {
        let rows: Vec<Value> = client
            .get(format!(
                "https://api.github.com/repos/{repo}/releases?per_page=100&page={page}"
            ))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        if let Some(candidate) = latest(&rows)
            && best
                .as_ref()
                .is_none_or(|b: &([u64; 3], String)| candidate.0 > b.0)
        {
            best = Some(candidate);
        }
        if rows.len() < 100 {
            break;
        }
    }
    let current = current_version(env!("CARGO_PKG_VERSION"))
        .ok_or_else(|| anyhow::anyhow!("Invalid package version"))?;
    Ok(best
        .filter(|b| b.0 > current)
        .map(|(_, tag)| format!("https://github.com/{repo}/releases/tag/{tag}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_urls_reject_shell_input_and_other_destinations() {
        assert!(validate_release_url("https://github.com/owner/repo/releases/tag/v1.2.3").is_ok());
        for url in [
            "https://example.com/owner/repo/releases/tag/v1.2.3",
            "http://github.com/owner/repo/releases/tag/v1.2.3",
            "file:///C:/Windows/System32/cmd.exe",
            "cmd /c calc",
            "https://github.com & calc",
            "https://user@github.com/owner/repo/releases/tag/v1.2.3",
            "https://github.com/owner/repo/releases/tag/v1.2.3?redirect=other",
            "https://github.com/owner/repo/releases/tag/v1.2.3#fragment",
            "https://github.com/owner/repo/releases/tag/latest",
        ] {
            assert!(validate_release_url(url).is_err(), "{url}");
        }
    }

    #[test]
    fn release_tags_stay_strict_but_package_versions_allow_suffixes() {
        assert_eq!(version("v1.2.3"), Some([1, 2, 3]));
        assert_eq!(version("v0.1.1"), Some([0, 1, 1]));
        for tag in [
            "1.2.3",
            "v1.2",
            "v01.2.3",
            "v1.2.3-beta.1",
            "v1.2.3+build.7",
        ] {
            assert_eq!(version(tag), None);
        }
        assert_eq!(current_version("0.2.0-beta.1"), Some([0, 2, 0]));
        assert_eq!(current_version("1.2.3+build.7"), Some([1, 2, 3]));
        assert_eq!(current_version("1.2"), None);
    }
}
