//! ToolManager: discover / version / auth status / install có xác nhận / diagnostics cho 4 integration.
//! Subprocess được cô lập: không qua shell khi kiểm tra, có timeout, log đã lọc bí mật.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, BufReader};

use doto_core::{events::EventBus, jobs, Db};

use crate::providers::{self, current_os, AuthState, CliProvider, InstallKind, Os};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallMethod {
    pub id: String,
    pub label: String,
    pub command: String,
    pub source_url: String,
    pub requires_elevation: bool,
    pub available: bool,
    pub unavailable_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolStatus {
    pub id: String,
    pub name: String,
    pub installed: bool,
    pub path: Option<String>,
    pub version: Option<String>,
    pub platform_supported: bool,
    pub auth_status: AuthState,
    pub auth_detail: Option<String>,
    pub install_methods: Vec<InstallMethod>,
    pub docs_url: String,
    pub skills_dir: Option<String>,
    pub last_checked_at: Option<String>,
}

pub struct ToolManager {
    db: Db,
    events: EventBus,
    bin_dir: PathBuf,
    home: PathBuf,
}

fn home_dir() -> PathBuf {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from).unwrap_or_default()
}

pub fn command(program: &Path) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(program);
    cmd.stdin(Stdio::null()).kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000);
    cmd
}

/// Chạy chương trình với đối số cố định (không qua shell), timeout, trả (exit_ok, stdout+stderr đã lọc).
pub async fn run_fixed(program: &Path, args: &[&str], timeout: Duration) -> Option<(bool, String, String)> {
    let mut cmd = command(program);
    cmd.args(args).env("NO_COLOR", "1").env("CI", "1");
    let out = tokio::time::timeout(timeout, cmd.output()).await.ok()?.ok()?;
    Some((
        out.status.success(),
        doto_core::redact_secrets(&String::from_utf8_lossy(&out.stdout)),
        doto_core::redact_secrets(&String::from_utf8_lossy(&out.stderr)),
    ))
}

impl ToolManager {
    pub fn new(db: Db, events: EventBus, data_dir: &Path) -> Arc<Self> {
        let bin_dir = data_dir.join("bin");
        let _ = std::fs::create_dir_all(&bin_dir);
        Arc::new(Self { db, events, bin_dir, home: home_dir() })
    }

    pub fn bin_dir(&self) -> &Path {
        &self.bin_dir
    }

    /// Tìm executable: PATH → vị trí cài thông dụng → thư mục bin riêng của app.
    pub fn locate(&self, p: &dyn CliProvider) -> Option<PathBuf> {
        let own = self.bin_dir.join(if cfg!(windows) { format!("{}.exe", p.binary()) } else { p.binary().to_string() });
        which::which(p.binary()).ok().or_else(|| p.common_locations(&self.home).into_iter().find(|c| c.is_file())).or_else(|| own.is_file().then_some(own))
    }

    fn methods(&self, p: &dyn CliProvider) -> Vec<InstallMethod> {
        p.install_methods(current_os())
            .into_iter()
            .map(|m| {
                let (command, reason) = match &m.kind {
                    InstallKind::Shell { command, requires } => {
                        let missing = requires.filter(|r| which::which(r).is_err());
                        (command.clone(), missing.map(|r| format!("cần `{r}` trên máy")))
                    }
                    InstallKind::VerifiedDownload => (format!("tải cloudflared-windows-amd64.exe từ GitHub Releases → {}", self.bin_dir.display()), None),
                };
                InstallMethod {
                    id: m.id.into(),
                    label: m.label.into(),
                    command,
                    source_url: m.source_url.into(),
                    requires_elevation: m.requires_elevation,
                    available: reason.is_none(),
                    unavailable_reason: reason,
                }
            })
            .collect()
    }

    #[allow(unused_assignments)]
    pub async fn status(&self, id: &str) -> Option<ToolStatus> {
        let p = providers::by_id(id)?;
        let path = self.locate(p.as_ref());
        let mut version = None;
        let (mut auth, mut detail) = (AuthState::Unknown, None);
        if let Some(bin) = &path {
            if let Some((_, out, err)) = run_fixed(bin, &["--version"], Duration::from_secs(15)).await {
                version = providers::parse_version(&out).or_else(|| providers::parse_version(&err));
            }
            match p.auth_args() {
                Some(args) => {
                    if let Some((ok, out, err)) = run_fixed(bin, args, Duration::from_secs(20)).await {
                        let out = if out.trim().is_empty() { err } else { out };
                        (auth, detail) = p.interpret_auth(ok, &out);
                    } else {
                        detail = Some("kiểm tra đăng nhập quá thời gian".into());
                    }
                }
                None => (auth, detail) = p.interpret_auth(true, ""),
            }
        } else {
            detail = Some("chưa cài".into());
        }
        let st = ToolStatus {
            id: p.id().into(),
            name: p.name().into(),
            installed: path.is_some(),
            path: path.map(|p| p.to_string_lossy().into_owned()),
            version,
            platform_supported: p.platform_supported(current_os()),
            auth_status: auth,
            auth_detail: detail,
            install_methods: self.methods(p.as_ref()),
            docs_url: p.docs_url().into(),
            skills_dir: p.skills_dir(&self.home).map(|d| d.to_string_lossy().into_owned()),
            last_checked_at: Some(doto_core::now_rfc3339()),
        };
        let (sid, json) = (st.id.clone(), serde_json::to_string(&st).unwrap_or_default());
        let (path_s, ver) = (st.path.clone(), st.version.clone());
        let _ = self
            .db
            .call(move |c| {
                c.execute(
                    "INSERT INTO cli_tools(id,path,version,status,last_checked_at) VALUES(?1,?2,?3,?4,?5)
                     ON CONFLICT(id) DO UPDATE SET path=excluded.path, version=excluded.version, status=excluded.status, last_checked_at=excluded.last_checked_at",
                    rusqlite::params![sid, path_s, ver, json, doto_core::now_rfc3339()],
                )?;
                Ok(())
            })
            .await;
        Some(st)
    }

    /// Trạng thái đã lưu (nhanh, không chạy subprocess); None nếu chưa từng kiểm tra.
    pub async fn cached(&self) -> Vec<ToolStatus> {
        let rows: Vec<String> = self
            .db
            .call(|c| {
                let mut st = c.prepare("SELECT status FROM cli_tools")?;
                let v = st.query_map([], |r| r.get(0))?.collect::<Result<Vec<String>, _>>()?;
                Ok(v)
            })
            .await
            .unwrap_or_default();
        rows.iter().filter_map(|r| serde_json::from_str::<serde_json::Value>(r).ok()).filter_map(|v| serde_json::from_value(v).ok()).collect()
    }

    pub async fn status_all(&self) -> Vec<ToolStatus> {
        let ids: Vec<&'static str> = providers::all().iter().map(|p| p.id()).collect();
        let futs = ids.into_iter().map(|id| self.status(id));
        futures_join(futs).await.into_iter().flatten().collect()
    }

    /// Chẩn đoán: chạy allowlist cố định của provider, kèm thông tin PATH/kiến trúc.
    pub async fn diagnostics(&self, id: &str) -> Option<Vec<String>> {
        let p = providers::by_id(id)?;
        let mut lines = vec![
            format!("OS: {} / kiến trúc: {}", std::env::consts::OS, std::env::consts::ARCH),
            format!("Tìm `{}` trên PATH: {}", p.binary(), which::which(p.binary()).map(|x| x.display().to_string()).unwrap_or_else(|_| "không thấy".into())),
        ];
        for c in p.common_locations(&self.home) {
            lines.push(format!("  vị trí thông dụng {} — {}", c.display(), if c.is_file() { "có" } else { "không" }));
        }
        if let Some(bin) = self.locate(p.as_ref()) {
            for args in p.diagnostic_commands() {
                match run_fixed(&bin, args, Duration::from_secs(20)).await {
                    Some((ok, out, err)) => {
                        lines.push(format!("$ {} {} → {}", p.binary(), args.join(" "), if ok { "OK" } else { "lỗi" }));
                        lines.extend(out.lines().chain(err.lines()).take(40).map(|l| format!("  {l}")));
                    }
                    None => lines.push(format!("$ {} {} → quá thời gian", p.binary(), args.join(" "))),
                }
            }
        }
        Some(lines)
    }

    /// Cài đặt (đã có xác nhận ở tầng API). Trả job; tiến trình chạy nền, log vào job.
    pub async fn install(self: &Arc<Self>, id: &str, method_id: &str) -> anyhow::Result<jobs::Job> {
        let p = providers::by_id(id).ok_or_else(|| anyhow::anyhow!("công cụ không hợp lệ"))?;
        let spec = p
            .install_methods(current_os())
            .into_iter()
            .find(|m| m.id == method_id)
            .ok_or_else(|| anyhow::anyhow!("phương thức cài không hợp lệ cho OS này"))?;
        if let InstallKind::Shell { requires: Some(r), .. } = &spec.kind {
            anyhow::ensure!(which::which(r).is_ok(), "cần `{r}` trên máy trước khi dùng phương thức này");
        }
        let job = jobs::create(&self.db, "install", &format!("Cài {} ({})", p.name(), spec.label), serde_json::json!({ "tool": id, "method": method_id }), 10)
            .await?;
        jobs::mark_running(&self.db, &job.id, None).await?;
        self.events.emit("job", &job);
        let me = self.clone();
        let jid = job.id.clone();
        let tool_id = id.to_string();
        tokio::spawn(async move {
            let res = match spec.kind {
                InstallKind::Shell { command, .. } => me.run_shell(&jid, &command).await,
                InstallKind::VerifiedDownload => crate::cloudflared_install::install_windows(&me.db, &jid, &me.bin_dir).await,
            };
            let st = me.status(&tool_id).await;
            let (status, err) = match (&res, st.as_ref().map(|s| s.installed)) {
                (Ok(()), Some(true)) => ("succeeded", None),
                (Ok(()), _) => {
                    ("failed", Some("lệnh cài kết thúc nhưng chưa tìm thấy executable — có thể cần mở lại terminal/ứng dụng để cập nhật PATH".to_string()))
                }
                (Err(e), _) => ("failed", Some(e.to_string())),
            };
            let _ = jobs::finish(&me.db, &jid, status, err, None).await;
            if let Ok(Some(j)) = jobs::get(&me.db, &jid).await {
                me.events.emit("job", &j);
            }
            if let Some(st) = st {
                me.events.emit("tool", &st);
            }
        });
        Ok(job)
    }

    async fn run_shell(&self, job_id: &str, command_line: &str) -> anyhow::Result<()> {
        let (prog, args): (PathBuf, Vec<String>) = match current_os() {
            Os::Windows => (
                "powershell.exe".into(),
                vec!["-NoProfile".into(), "-NonInteractive".into(), "-ExecutionPolicy".into(), "Bypass".into(), "-Command".into(), command_line.into()],
            ),
            _ => ("/bin/bash".into(), vec!["-lc".into(), command_line.into()]),
        };
        jobs::append_log(&self.db, job_id, &format!("$ {command_line}")).await?;
        let mut cmd = command(&prog);
        cmd.args(&args).stdout(Stdio::piped()).stderr(Stdio::piped()).env("NO_COLOR", "1");
        let mut child = cmd.spawn()?;
        let out = child.stdout.take().expect("piped");
        let err = child.stderr.take().expect("piped");
        let (db1, db2, j1, j2) = (self.db.clone(), self.db.clone(), job_id.to_string(), job_id.to_string());
        let t1 = tokio::spawn(async move {
            let mut l = BufReader::new(out).lines();
            while let Ok(Some(line)) = l.next_line().await {
                let _ = jobs::append_log(&db1, &j1, &line).await;
            }
        });
        let t2 = tokio::spawn(async move {
            let mut l = BufReader::new(err).lines();
            while let Ok(Some(line)) = l.next_line().await {
                let _ = jobs::append_log(&db2, &j2, &line).await;
            }
        });
        let status =
            tokio::time::timeout(Duration::from_secs(15 * 60), child.wait()).await.map_err(|_| anyhow::anyhow!("quá 15 phút — đã dừng tiến trình cài"))??;
        let _ = t1.await;
        let _ = t2.await;
        anyhow::ensure!(status.success(), "lệnh cài thoát mã {:?}", status.code());
        Ok(())
    }
}

async fn futures_join<F: std::future::Future>(futs: impl IntoIterator<Item = F>) -> Vec<F::Output> {
    let mut out = vec![];
    let handles: Vec<_> = futs.into_iter().collect();
    for f in handles {
        out.push(f.await);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn status_reports_every_tool_without_installing() {
        let tmp = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let m = ToolManager::new(db.clone(), EventBus::default(), tmp.path());
        let all = m.status_all().await;
        assert_eq!(all.len(), 4);
        for s in &all {
            assert!(!s.install_methods.is_empty() || !s.platform_supported, "{}", s.id);
            if !s.installed {
                assert!(s.path.is_none() && s.version.is_none());
            }
        }
        // không có job nào được tạo (không tự cài)
        assert_eq!(jobs::list(&db, 10).await.unwrap().len(), 0);
        assert_eq!(m.cached().await.len(), 4);
    }

    #[tokio::test]
    async fn rejects_unknown_tool_or_method() {
        let tmp = tempfile::tempdir().unwrap();
        let m = ToolManager::new(Db::open_in_memory().unwrap(), EventBus::default(), tmp.path());
        assert!(m.install("rm", "x").await.is_err());
        assert!(m.install("claude", "curl-evil").await.is_err());
    }
}
