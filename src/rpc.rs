use crate::{
    model::{self, Identity, Profile, Usage},
    platform,
};
use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines},
    process::{Child, ChildStdin, ChildStdout},
};

pub fn locate() -> Result<PathBuf> {
    if let Some(p) = std::env::var_os("CODEX_CLI_PATH").filter(|p| !p.is_empty()) {
        let p = PathBuf::from(p);
        if p.is_absolute() {
            ensure!(
                p.is_file() && p.extension().is_some_and(|e| e.eq_ignore_ascii_case("exe")),
                "CODEX_CLI_PATH must point to an executable"
            );
            return Ok(p);
        }
        ensure!(
            p.components().count() == 1,
            "CODEX_CLI_PATH must be absolute or an executable name"
        );
        return search_path(&p).context("Configured Codex runtime not found");
    }
    if let Some(p) = search_path(Path::new("codex")) {
        return Ok(p);
    }
    let bin = platform::local_data().join("OpenAI/Codex/bin");
    let mut versions: Vec<_> = std::fs::read_dir(bin)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .collect();
    versions.sort_by_key(|p| std::cmp::Reverse(p.metadata().and_then(|m| m.modified()).ok()));
    for v in versions {
        let p = v.path().join("codex.exe");
        if p.is_file() {
            return Ok(p);
        }
    }
    bail!("Codex runtime not found. Install Codex or set CODEX_CLI_PATH.")
}
fn search_path(name: &Path) -> Option<PathBuf> {
    let name = if name.extension().is_some() {
        name.to_path_buf()
    } else {
        name.with_extension("exe")
    };
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|p| p.join(&name))
        .find(|p| p.is_file())
}
pub struct Session {
    child: Child,
    input: ChildStdin,
    lines: Lines<BufReader<ChildStdout>>,
    pending: VecDeque<Value>,
    cancel: Arc<AtomicBool>,
    next_id: u32,
}
impl Session {
    pub async fn start(exe: &Path, home: &Path, cancel: Arc<AtomicBool>) -> Result<Self> {
        let mut command = tokio::process::Command::new(exe);
        command
            .args(["app-server", "--stdio"])
            .env("CODEX_HOME", home)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true)
            .creation_flags(0x08000000);
        Self::start_command(command, cancel).await
    }
    async fn start_command(
        mut command: tokio::process::Command,
        cancel: Arc<AtomicBool>,
    ) -> Result<Self> {
        let mut child = command
            .spawn()
            .context("Could not start Codex app-server")?;
        let input = child.stdin.take().context("Missing RPC stdin")?;
        let lines = BufReader::new(child.stdout.take().context("Missing RPC stdout")?).lines();
        let mut session = Self {
            child,
            input,
            lines,
            pending: VecDeque::new(),
            cancel,
            next_id: 0,
        };
        session.request("initialize", json!({"clientInfo":{"name":"cat_codex_switcher","title":"Cat Codex Switcher","version":env!("CARGO_PKG_VERSION")}})).await?;
        session
            .send(json!({"method":"initialized","params":{}}))
            .await?;
        Ok(session)
    }
    async fn send(&mut self, v: Value) -> Result<()> {
        let mut bytes = serde_json::to_vec(&v)?;
        bytes.push(b'\n');
        self.input.write_all(&bytes).await?;
        self.input.flush().await?;
        Ok(())
    }
    pub async fn request(&mut self, method: &str, params: Value) -> Result<Value> {
        let id = self.next_id;
        self.next_id += 1;
        self.send(json!({"id":id,"method":method,"params":params}))
            .await?;
        self.receive(Some(id), None, Duration::from_secs(20)).await
    }
    async fn receive(
        &mut self,
        id: Option<u32>,
        method: Option<&str>,
        duration: Duration,
    ) -> Result<Value> {
        let matches = |v: &Value| {
            if let Some(id) = id {
                v.get("id").and_then(Value::as_u64) == Some(id as u64)
            } else {
                v.get("method").and_then(Value::as_str) == method
            }
        };
        if let Some(pos) = self.pending.iter().position(matches) {
            return decode(
                self.pending.remove(pos).expect("queued response exists"),
                id.is_some(),
            );
        }
        let deadline = tokio::time::Instant::now() + duration;
        loop {
            ensure!(!self.cancel.load(Ordering::SeqCst), "Operation cancelled");
            if tokio::time::Instant::now() >= deadline {
                bail!("Codex app-server response timed out");
            }
            if let Ok(line) =
                tokio::time::timeout(Duration::from_millis(50), self.lines.next_line()).await
            {
                let line = line?.context("Codex app-server closed the connection")?;
                ensure!(line.len() <= 4 * 1024 * 1024, "RPC message too large");
                let value: Value = serde_json::from_str(&line)
                    .map_err(|_| anyhow::anyhow!("Malformed RPC response"))?;
                if matches(&value) {
                    return decode(value, id.is_some());
                }
                // A login notification may arrive before the login/start response.
                if value.get("method").and_then(Value::as_str) == Some("account/login/completed") {
                    ensure!(self.pending.len() < 64, "Too many queued RPC messages");
                    self.pending.push_back(value);
                }
            }
        }
    }
    pub async fn stop(&mut self) {
        let _ = self.child.kill().await;
        let _ = self.child.wait().await;
    }
    pub async fn login(&mut self, home: &Path) -> Result<Identity> {
        self.login_with(home, platform::browser).await
    }
    pub async fn login_with(
        &mut self,
        home: &Path,
        open_browser: impl FnOnce(&str) -> Result<()>,
    ) -> Result<Identity> {
        let start = self
            .request(
                "account/login/start",
                json!({"type":"chatgpt","useHostedLoginSuccessPage":true,"appBrand":"codex"}),
            )
            .await?;
        open_browser(
            start
                .get("authUrl")
                .and_then(Value::as_str)
                .context("Login URL missing")?,
        )?;
        let done = self
            .receive(
                None,
                Some("account/login/completed"),
                Duration::from_secs(600),
            )
            .await?;
        ensure!(
            done.get("success").and_then(Value::as_bool) == Some(true),
            "Browser login did not complete"
        );
        let value = self
            .request("account/read", json!({"refreshToken":false}))
            .await?;
        let local = model::identity(&platform::read(&home.join("auth.json"))?)?;
        verify_account(&value, &local)?;
        Ok(local)
    }
    async fn verify_login(&mut self, home: &Path, profile: &Profile) -> Result<()> {
        let value = self
            .request("account/read", json!({"refreshToken":true}))
            .await?;
        let local = model::identity(&platform::read(&home.join("auth.json"))?)?;
        ensure!(
            local.matches(profile),
            "Target identity verification failed"
        );
        verify_account(&value, &local)?;
        // A live authenticated read must succeed even when this plan has no weekly window.
        self.request("account/rateLimits/read", json!({})).await?;
        Ok(())
    }
    pub async fn usage(&mut self, profile: &Profile) -> Result<Usage> {
        let value = self.request("account/rateLimits/read", json!({})).await?;
        if let Some(account) = value.get("accountId").and_then(Value::as_str)
            && let Some(expected) = &profile.account_id
        {
            ensure!(account == expected, "Usage account identity mismatch");
        }
        model::normalize(&value)
    }
}
pub async fn verify_login(
    exe: &Path,
    home: &Path,
    profile: &Profile,
    cancel: Arc<AtomicBool>,
) -> Result<()> {
    let mut session = Session::start(exe, home, cancel).await?;
    let result = session.verify_login(home, profile).await;
    session.stop().await;
    result
}
fn verify_account(value: &Value, local: &Identity) -> Result<()> {
    let account = value
        .get("account")
        .and_then(Value::as_object)
        .context("Login identity unavailable")?;
    ensure!(
        account.get("type").and_then(Value::as_str) == Some("chatgpt"),
        "Expected a ChatGPT login"
    );
    if let Some(id) = ["accountId", "accountID", "chatgptAccountId", "id"]
        .iter()
        .find_map(|k| account.get(*k).and_then(Value::as_str))
    {
        ensure!(
            local.account_id.as_deref() == Some(id),
            "Login identity mismatch"
        );
    }
    if let Some(email) = account.get("email").and_then(Value::as_str) {
        ensure!(
            local
                .email
                .as_ref()
                .is_none_or(|e| e.eq_ignore_ascii_case(email)),
            "Login identity mismatch"
        );
    }
    Ok(())
}
fn decode(v: Value, response: bool) -> Result<Value> {
    // Remote text may contain credentials. Show a bounded category and numeric code only.
    if let Some(e) = v.get("error") {
        bail!(
            "Codex RPC error (code {})",
            e.get("code").and_then(Value::as_i64).unwrap_or(-1)
        );
    }
    v.get(if response { "result" } else { "params" })
        .cloned()
        .context("Malformed RPC envelope")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_login_accepts_completion_before_start_response() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let root = std::env::temp_dir().join(format!("switcher-login-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let auth = json!({"tokens": {"account_id":"saved-id", "id_token":
            "a.eyJodHRwczovL2FwaS5vcGVuYWkuY29tL2F1dGgiOnsiY2hhdGdwdF91c2VyX2lkIjoidGVzdC11c2VyIn19.b"}});
        std::fs::write(root.join("auth.json"), auth.to_string()).unwrap();
        let script = r#"
            $reads = 0
            while ($null -ne ($line = [Console]::ReadLine())) {
                $request = ConvertFrom-Json -InputObject $line
                if ($null -eq $request.id) { continue }
                $result = '{}'
                switch ($request.method) {
                    'initialize' {}
                    'account/login/start' {
                        [Console]::WriteLine('{"method":"account/login/completed","params":{"success":true}}')
                        $result = '{"authUrl":"https://example.test/login"}'
                    }
                    'account/read' { $result = '{"account":{"type":"chatgpt","id":"saved-id"}}' }
                    'account/rateLimits/read' {
                        $reads++
                        $account = if ($reads -eq 1) { 'other' } else { 'saved-id' }
                        $result = '{"accountId":"' + $account + '","rateLimits":{"primary":{"windowDurationMins":10080,"usedPercent":40,"resetsAt":null}}}'
                    }
                    default { exit 2 }
                }
                [Console]::WriteLine(('{"id":' + $request.id + ',"result":' + $result + '}'))
            }
        "#;
        let exe = PathBuf::from(std::env::var_os("SystemRoot").unwrap())
            .join("System32/WindowsPowerShell/v1.0/powershell.exe");
        let mut command = tokio::process::Command::new(exe);
        command
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true)
            .creation_flags(0x08000000);
        let mut opened = String::new();
        let identity = rt.block_on(async {
            let mut session = Session::start_command(command, Arc::new(AtomicBool::new(false)))
                .await
                .unwrap();
            let result = tokio::time::timeout(
                Duration::from_secs(5),
                session.login_with(&root, |url| {
                    opened = url.to_owned();
                    Ok(())
                }),
            )
            .await
            .expect("login completion was not delivered");
            let identity = result.unwrap();
            let profile = identity.profile();
            assert!(
                session
                    .usage(&profile)
                    .await
                    .unwrap_err()
                    .to_string()
                    .contains("identity mismatch")
            );
            assert_eq!(session.usage(&profile).await.unwrap().remaining_percent, 60);
            session.stop().await;
            identity
        });
        assert_eq!(opened, "https://example.test/login");
        assert_eq!(identity.account_id.as_deref(), Some("saved-id"));
        assert_eq!(identity.user_id.as_deref(), Some("test-user"));
        std::fs::remove_file(root.join("auth.json")).unwrap();
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    fn verification_refreshes_auth_and_requires_a_live_authenticated_read() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let root = std::env::temp_dir().join(format!("switcher-rpc-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let token = "a.eyJodHRwczovL2FwaS5vcGVuYWkuY29tL2F1dGgiOnsiY2hhdGdwdF91c2VyX2lkIjoidGVzdC11c2VyIn19.b";
        let auth = json!({"tokens":{"account_id":"saved-id","refresh_token":"old-fake-token", "id_token":token}}).to_string();
        let profile = model::identity(auth.as_bytes()).unwrap().profile();
        let log = root.join("requests.txt");
        let script = r#"
            $ErrorActionPreference = 'Stop'
            while ($null -ne ($line = [Console]::ReadLine())) {
                $request = ConvertFrom-Json -InputObject $line
                if ($null -eq $request.id) { continue }
                $result = '{}'
                $failed = $false
                switch ($request.method) {
                    'initialize' {}
                    'account/read' {
                        if ($request.params.refreshToken -ne $true) { exit 2 }
                        [IO.File]::AppendAllText($env:SWITCHER_TEST_LOG, "refresh`n")
                        $failed = $env:SWITCHER_TEST_FAILURE -eq 'refresh'
                        if (-not $failed) {
                            [IO.File]::WriteAllText($env:SWITCHER_TEST_AUTH_PATH, $env:SWITCHER_TEST_AUTH)
                        }
                        $result = $env:SWITCHER_TEST_ACCOUNT
                    }
                    'account/rateLimits/read' {
                        [IO.File]::AppendAllText($env:SWITCHER_TEST_LOG, "limits`n")
                        $failed = $env:SWITCHER_TEST_FAILURE -eq 'limits'
                    }
                    default { exit 3 }
                }
                if ($failed) {
                    [Console]::WriteLine(('{"id":' + $request.id + ',"error":{"code":401,"message":"fake secret"}}'))
                } else {
                    [Console]::WriteLine(('{"id":' + $request.id + ',"result":' + $result + '}'))
                }
            }
        "#;
        for (account, failure, rotated_id, expected) in [
            (
                json!({"type":"chatgpt","id":"saved-id"}),
                "",
                "saved-id",
                true,
            ),
            (Value::Null, "", "saved-id", false),
            (json!({"type":"apiKey"}), "", "saved-id", false),
            (
                json!({"type":"chatgpt","id":"other"}),
                "",
                "saved-id",
                false,
            ),
            (json!({"type":"chatgpt"}), "", "other", false),
            (json!({"type":"chatgpt"}), "refresh", "saved-id", false),
            (json!({"type":"chatgpt"}), "limits", "saved-id", false),
        ] {
            std::fs::write(root.join("auth.json"), &auth).unwrap();
            std::fs::write(&log, b"").unwrap();
            let rotated = json!({"tokens":{"account_id":rotated_id,"refresh_token":"new-fake-token", "id_token":token}});
            let exe = PathBuf::from(std::env::var_os("SystemRoot").unwrap())
                .join("System32/WindowsPowerShell/v1.0/powershell.exe");
            let mut command = tokio::process::Command::new(exe);
            command
                .args(["-NoProfile", "-NonInteractive", "-Command", script])
                .env("SWITCHER_TEST_LOG", &log)
                .env("SWITCHER_TEST_AUTH_PATH", root.join("auth.json"))
                .env("SWITCHER_TEST_AUTH", rotated.to_string())
                .env(
                    "SWITCHER_TEST_ACCOUNT",
                    json!({"account":account}).to_string(),
                )
                .env("SWITCHER_TEST_FAILURE", failure)
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .kill_on_drop(true)
                .creation_flags(0x08000000);
            let result = rt.block_on(async {
                let mut session = Session::start_command(command, Arc::new(AtomicBool::new(false)))
                    .await
                    .unwrap();
                let result = session.verify_login(&root, &profile).await;
                session.stop().await;
                result
            });
            assert_eq!(result.is_ok(), expected, "{account}, {failure}: {result:?}");
            let requests = std::fs::read_to_string(&log).unwrap();
            assert!(requests.starts_with("refresh\n"));
            if expected || failure == "limits" {
                assert_eq!(requests, "refresh\nlimits\n");
                assert_eq!(
                    std::fs::read(root.join("auth.json")).unwrap(),
                    rotated.to_string().as_bytes()
                );
            }
            if let Err(error) = result {
                assert!(!format!("{error:#}").contains("fake secret"));
            }
        }
        std::fs::remove_file(root.join("auth.json")).unwrap();
        std::fs::remove_file(log).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
}
