use crate::{
    model::{self, Identity, Usage},
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
        let account = value
            .get("account")
            .and_then(Value::as_object)
            .context("Login identity unavailable")?;
        if let Some(id) = ["accountId", "accountID", "chatgptAccountId", "id"]
            .iter()
            .find_map(|k| account.get(*k).and_then(Value::as_str))
        {
            ensure!(
                local.account_id.as_deref() == Some(id),
                "Login identity mismatch"
            );
        }
        if let Some(email) = value.pointer("/account/email").and_then(Value::as_str) {
            ensure!(
                local
                    .email
                    .as_ref()
                    .is_none_or(|e| e.eq_ignore_ascii_case(email)),
                "Login identity mismatch"
            );
        }
        Ok(local)
    }
    pub async fn usage(&mut self) -> Result<Usage> {
        model::normalize(&self.request("account/rateLimits/read", json!({})).await?)
    }
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
