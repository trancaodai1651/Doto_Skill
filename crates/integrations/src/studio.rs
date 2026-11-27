//! AI Studio Runner: điều phối chạy đơn phiên & đa phiên song song trên phần cứng máy chủ
//! cho 3 công cụ AI CLI (`antigravity` / `agy`, `claude`, `codex`).
//!
//! Nguyên tắc an toàn:
//! - Không bao giờ ghép chuỗi qua shell; chỉ truyền mảng đối số cố định qua `tokio::process::Command`.
//! - Bắt buộc Owner xác nhận (`confirm = true`) ở tầng API.
//! - Mỗi phiên chạy có thư mục xuất cô lập (`<output_dir>/studio-<job8>-s<idx>`) để chạy song song không đụng độ.
//! - Toàn bộ stdout/stderr đều lọc bí mật (`redact_secrets`) trước khi lưu SQLite hoặc phát SSE.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio_util::sync::CancellationToken;

use doto_core::{events::EventBus, jobs, Db};

use crate::manager::{command, run_fixed, ToolManager};
use crate::providers;

pub const MAX_PARALLEL_SESSIONS: usize = 6;
pub const MAX_PROMPT_CHARS: usize = 32_000;
const MAX_SESSION_LOG_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StudioModelInfo {
    pub id: String,
    pub name: String,
    pub badge: Option<String>,
    pub default_effort: String,
    pub efforts: Vec<String>,
    pub slug_pattern: Option<String>,
    pub desc: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudioProviderCatalog {
    pub id: String,
    pub name: String,
    pub vendor: String,
    pub installed: bool,
    pub binary_path: Option<String>,
    pub version: Option<String>,
    pub skill_prefix: &'static str,
    pub models: Vec<StudioModelInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StudioSessionSpec {
    pub provider: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub effort: String,
    #[serde(default)]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StudioRunRequest {
    /// `single` | `multi_same_ai` | `multi_ai` | `custom_batch`
    pub mode: String,
    pub prompt: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub skill_id: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub effort: Option<String>,
    /// Số phiên song song cho chế độ `multi_same_ai` (2..=6).
    #[serde(default)]
    pub count: Option<usize>,
    /// Danh sách phiên cụ thể cho `multi_ai` hoặc `custom_batch`.
    #[serde(default)]
    pub sessions: Vec<StudioSessionSpec>,
    /// ID các media đã tải lên (`uploaded_media.id`) để đính kèm đường dẫn an toàn vào prompt.
    #[serde(default)]
    pub upload_ids: Vec<String>,
    /// Có đính kèm gợi ý đọc bộ nhớ chung (`PROJECT_MEMORY.md` / `MEMORY.md`) hay không.
    #[serde(default)]
    pub include_memory_hint: bool,
    #[serde(default)]
    pub visibility: Option<String>,
    pub confirm: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StudioSession {
    pub id: String,
    pub job_id: String,
    pub session_index: usize,
    pub label: String,
    pub provider: String,
    pub model: String,
    pub effort: String,
    pub skill_id: Option<String>,
    pub status: String,
    pub command_preview: String,
    pub output_subdir: String,
    pub result_text: String,
    pub log: Vec<String>,
    pub error: Option<String>,
    pub exit_code: Option<i32>,
    pub output_video_ids: Vec<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudioRunDetail {
    pub job: jobs::Job,
    pub sessions: Vec<StudioSession>,
}

pub fn default_claude_models() -> Vec<StudioModelInfo> {
    let efforts: Vec<String> = ["low", "medium", "high", "xhigh", "max"].into_iter().map(String::from).collect();
    vec![
        StudioModelInfo {
            id: "claude-opus-4-6".into(),
            name: "Claude Opus 4.6".into(),
            badge: Some("Khuyên dùng trọn gói".into()),
            default_effort: "high".into(),
            efforts: efforts.clone(),
            slug_pattern: None,
            desc: "Suy luận sâu và điều phối công cụ đa bước ổn định nhất trên Claude Code.".into(),
        },
        StudioModelInfo {
            id: "claude-sonnet-4-6".into(),
            name: "Claude Sonnet 4.6".into(),
            badge: Some("Nhanh · Cân bằng".into()),
            default_effort: "medium".into(),
            efforts: efforts.clone(),
            slug_pattern: None,
            desc: "Tốc độ cao, cân bằng chi phí cho kịch bản và dựng video hằng ngày.".into(),
        },
        StudioModelInfo {
            id: "claude-haiku-4-5".into(),
            name: "Claude Haiku 4.5".into(),
            badge: Some("Siêu tốc".into()),
            default_effort: "low".into(),
            efforts,
            slug_pattern: None,
            desc: "Phản hồi tức thì cho chỉnh sửa nhỏ hoặc kiểm tra nhanh.".into(),
        },
    ]
}

pub fn default_codex_models() -> Vec<StudioModelInfo> {
    // Đọc thử ~/.codex/models_cache.json nếu có trên máy chủ
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from);
    let codex_home = std::env::var_os("CODEX_HOME").map(PathBuf::from).or_else(|| home.map(|h| h.join(".codex")));
    if let Some(cache) = codex_home.map(|d| d.join("models_cache.json")).filter(|p| p.is_file()) {
        if let Ok(raw) = std::fs::read_to_string(cache) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) {
                if let Some(arr) = v.get("models").and_then(|m| m.as_array()) {
                    let parsed: Vec<StudioModelInfo> = arr
                        .iter()
                        .filter(|m| m.get("visibility").and_then(|x| x.as_str()).unwrap_or("list") == "list")
                        .filter_map(|m| {
                            let id = m.get("slug").or_else(|| m.get("id"))?.as_str()?.trim().to_string();
                            if id.is_empty() {
                                return None;
                            }
                            let name = m.get("display_name").and_then(|x| x.as_str()).unwrap_or(&id).to_string();
                            let efforts: Vec<String> = m
                                .get("supported_reasoning_levels")
                                .or_else(|| m.get("supported_reasoning_efforts"))
                                .and_then(|x| x.as_array())
                                .map(|list| {
                                    list.iter()
                                        .filter_map(|e| e.get("effort").and_then(|s| s.as_str()).or_else(|| e.as_str()).map(String::from))
                                        .collect::<Vec<_>>()
                                })
                                .filter(|v| !v.is_empty())
                                .unwrap_or_else(|| ["low", "medium", "high", "xhigh"].into_iter().map(String::from).collect());
                            let def = m.get("default_reasoning_level").and_then(|x| x.as_str()).map(String::from).unwrap_or_else(|| {
                                if efforts.iter().any(|e| e == "high") {
                                    "high".into()
                                } else {
                                    efforts[0].clone()
                                }
                            });
                            Some(StudioModelInfo {
                                id,
                                name,
                                badge: None,
                                default_effort: def,
                                efforts,
                                slug_pattern: None,
                                desc: m.get("description").and_then(|x| x.as_str()).unwrap_or("Nạp từ Codex CLI cache trên máy chủ.").to_string(),
                            })
                        })
                        .take(12)
                        .collect();
                    if !parsed.is_empty() {
                        return parsed;
                    }
                }
            }
        }
    }
    let efforts: Vec<String> = ["low", "medium", "high", "xhigh"].into_iter().map(String::from).collect();
    vec![
        StudioModelInfo {
            id: "gpt-5.4".into(),
            name: "GPT-5.4 Codex".into(),
            badge: Some("Mặc định".into()),
            default_effort: "high".into(),
            efforts: efforts.clone(),
            slug_pattern: None,
            desc: "Mô hình lập trình & tự động hoá chuỗi công cụ trên OpenAI Codex CLI.".into(),
        },
        StudioModelInfo {
            id: "gpt-5.3-codex".into(),
            name: "GPT-5.3 Codex".into(),
            badge: Some("Ổn định".into()),
            default_effort: "medium".into(),
            efforts,
            slug_pattern: None,
            desc: "Tốc độ nhanh cho chạy hàng loạt nhiều phiên song song.".into(),
        },
    ]
}

pub fn default_antigravity_models() -> Vec<StudioModelInfo> {
    vec![
        StudioModelInfo {
            id: "gemini-2.5-pro".into(),
            name: "Gemini Pro (Antigravity)".into(),
            badge: Some("Đa phương thức".into()),
            default_effort: "high".into(),
            efforts: ["low", "medium", "high"].into_iter().map(String::from).collect(),
            slug_pattern: None,
            desc: "Mô hình đa phương thức chủ lực trong Google Antigravity CLI.".into(),
        },
        StudioModelInfo {
            id: "gemini-2.5-flash".into(),
            name: "Gemini Flash (Antigravity)".into(),
            badge: Some("Tốc độ cao".into()),
            default_effort: "medium".into(),
            efforts: ["low", "medium", "high"].into_iter().map(String::from).collect(),
            slug_pattern: None,
            desc: "Tốc độ cao, phù hợp chạy nhiều phương án thử nghiệm song song.".into(),
        },
    ]
}

fn valid_token_ident(s: &str) -> bool {
    !s.is_empty() && s.len() <= 80 && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

pub fn validate_provider_model_effort(provider: &str, model: &str, effort: &str) -> Result<(), String> {
    if !matches!(provider, "antigravity" | "claude" | "codex") {
        return Err(format!("provider không hợp lệ: {provider}"));
    }
    if !model.is_empty() && !valid_token_ident(model) {
        return Err("tên model chứa ký tự không hợp lệ".into());
    }
    if !effort.is_empty() && !matches!(effort, "low" | "medium" | "high" | "xhigh" | "max" | "ultra") {
        return Err("mức effort không hợp lệ".into());
    }
    Ok(())
}

/// Sinh mảng đối số cố định (không qua shell) + chuỗi hiển thị minh bạch cho hộp thoại xác nhận.
pub fn build_invocation(
    provider: &str,
    model: &str,
    effort: &str,
    prompt: &str,
    skill_dir: Option<&Path>,
    output_dir: &Path,
) -> (Vec<String>, Option<String>, String) {
    let short_prompt = if prompt.chars().count() > 80 { format!("{}…", prompt.chars().take(80).collect::<String>()) } else { prompt.to_string() };
    // Truyền qua stdin khi prompt dài hoặc nhiều dòng để tránh giới hạn argv và lỗi thoát ký tự xuống dòng trên `.cmd` Windows (CVE-2024-24576).
    let use_stdin = prompt.len() > 3500 || prompt.contains(['\n', '\r']);
    let out_str = output_dir.to_string_lossy().into_owned();
    let skill_str = skill_dir.map(|p| p.to_string_lossy().into_owned());

    match provider {
        "claude" => {
            let p_arg = if use_stdin { "Thực hiện đầy đủ yêu cầu được truyền qua stdin.".to_string() } else { prompt.to_string() };
            let mut args = vec!["-p".to_string(), p_arg, "--output-format".to_string(), "stream-json".to_string(), "--verbose".to_string()];
            if !model.is_empty() {
                args.push("--model".into());
                args.push(model.into());
            }
            if !effort.is_empty() {
                args.push("--effort".into());
                args.push(effort.into());
            }
            args.push("--permission-mode".into());
            args.push("bypassPermissions".into());
            if let Some(sd) = &skill_str {
                args.push("--add-dir".into());
                args.push(sd.clone());
            }
            args.push("--add-dir".into());
            args.push(out_str);

            let mut disp = format!("claude -p {:?}", short_prompt);
            if !model.is_empty() {
                disp.push_str(&format!(" --model {model}"));
            }
            if !effort.is_empty() {
                disp.push_str(&format!(" --effort {effort}"));
            }
            disp.push_str(" --output-format stream-json --permission-mode bypassPermissions");
            (args, use_stdin.then(|| prompt.to_string()), disp)
        }
        "codex" => {
            let mut args =
                vec!["exec".to_string(), "--json".to_string(), "--skip-git-repo-check".to_string(), "--sandbox".to_string(), "workspace-write".to_string()];
            if let Some(sd) = &skill_str {
                args.push("--add-dir".into());
                args.push(sd.clone());
            }
            args.push("--add-dir".into());
            args.push(out_str);
            if !model.is_empty() {
                args.push("-m".into());
                args.push(model.into());
            }
            if !effort.is_empty() {
                args.push("-c".into());
                args.push(format!("model_reasoning_effort=\"{effort}\""));
            }
            args.push("--".into());
            args.push(if use_stdin { "-".into() } else { prompt.to_string() });

            let mut disp = "codex exec --json --sandbox workspace-write".to_string();
            if !model.is_empty() {
                disp.push_str(&format!(" -m {model}"));
            }
            if !effort.is_empty() {
                disp.push_str(&format!(" -c model_reasoning_effort=\"{effort}\""));
            }
            disp.push_str(&format!(" -- {:?}", short_prompt));
            (args, use_stdin.then(|| prompt.to_string()), disp)
        }
        _ => {
            let safe_inline = prompt.replace("\r\n", " | ").replace(['\r', '\n'], " | ");
            let mut args = vec!["-p".to_string(), safe_inline, "--output-format".to_string(), "stream-json".to_string()];
            if !model.is_empty() {
                args.push("--model".into());
                args.push(model.into());
            }
            if !effort.is_empty() {
                args.push("--effort".into());
                args.push(effort.into());
            }
            args.push("--dangerously-skip-permissions".into());
            if let Some(sd) = &skill_str {
                args.push("--add-dir".into());
                args.push(sd.clone());
            }
            args.push("--add-dir".into());
            args.push(out_str);

            let mut disp = format!("agy -p {:?}", short_prompt);
            if !model.is_empty() {
                disp.push_str(&format!(" --model {model}"));
            }
            if !effort.is_empty() {
                disp.push_str(&format!(" --effort {effort}"));
            }
            disp.push_str(" --output-format stream-json --dangerously-skip-permissions");
            (args, None, disp)
        }
    }
}

/// Phân tích một dòng `stream-json` từ Claude Code / Codex / Antigravity sang thông điệp dễ đọc + trích `result_text`.
pub fn parse_stream_json_line(provider: &str, line: &str) -> (Option<String>, Option<String>, Option<String>) {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return (None, None, None);
    }
    let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) else {
        return (Some(doto_core::redact_secrets(trimmed)), None, None);
    };
    match provider {
        "claude" => match v.get("type").and_then(|t| t.as_str()).unwrap_or("") {
            "system" if v.get("subtype").and_then(|s| s.as_str()) == Some("init") => {
                let sid = v.get("session_id").and_then(|s| s.as_str()).unwrap_or("");
                (Some(format!("[khởi tạo] phiên Claude {sid}")), None, None)
            }
            "assistant" => {
                let mut logs = vec![];
                let mut res_txt = None;
                if let Some(arr) = v.pointer("/message/content").and_then(|c| c.as_array()) {
                    for b in arr {
                        match b.get("type").and_then(|t| t.as_str()).unwrap_or("") {
                            "tool_use" => {
                                let name = b.get("name").and_then(|n| n.as_str()).unwrap_or("tool");
                                logs.push(format!("[công cụ] {name}"));
                            }
                            "text" => {
                                if let Some(txt) = b.get("text").and_then(|t| t.as_str()).map(str::trim).filter(|s| !s.is_empty()) {
                                    let clean = doto_core::redact_secrets(txt);
                                    res_txt = Some(clean.clone());
                                    logs.push(clean);
                                }
                            }
                            _ => {}
                        }
                    }
                }
                (if logs.is_empty() { None } else { Some(logs.join("\n")) }, res_txt, None)
            }
            "result" => {
                let res = v.get("result").and_then(|r| r.as_str()).map(doto_core::redact_secrets);
                let is_err = v.get("is_error").and_then(|b| b.as_bool()).unwrap_or(false);
                if is_err {
                    let e = res.clone().unwrap_or_else(|| "Claude báo lỗi".into());
                    (Some(format!("[lỗi] {e}")), None, Some(e))
                } else {
                    (Some("[hoàn tất] đã nhận kết quả cuối".into()), res, None)
                }
            }
            _ => (None, None, None),
        },
        "codex" => match v.get("type").and_then(|t| t.as_str()).unwrap_or("") {
            "thread.started" => {
                let tid = v.get("thread_id").and_then(|s| s.as_str()).unwrap_or("");
                (Some(format!("[khởi tạo] phiên Codex {tid}")), None, None)
            }
            "item.started" => {
                let kind = v.pointer("/item/type").and_then(|s| s.as_str()).unwrap_or("");
                if kind == "command_execution" {
                    let cmd = v.pointer("/item/command").and_then(|s| s.as_str()).unwrap_or("");
                    (Some(format!("[lệnh] {}", doto_core::redact_secrets(cmd))), None, None)
                } else {
                    (None, None, None)
                }
            }
            "item.completed" => {
                let kind = v.pointer("/item/type").and_then(|s| s.as_str()).unwrap_or("");
                if kind == "agent_message" {
                    if let Some(txt) = v.pointer("/item/text").and_then(|s| s.as_str()).map(str::trim).filter(|s| !s.is_empty()) {
                        let clean = doto_core::redact_secrets(txt);
                        return (Some(clean.clone()), Some(clean), None);
                    }
                }
                (None, None, None)
            }
            "turn.failed" | "error" => {
                let msg = v.pointer("/error/message").or_else(|| v.get("message")).and_then(|s| s.as_str()).unwrap_or("Lỗi thực thi Codex");
                let clean = doto_core::redact_secrets(msg);
                (Some(format!("[lỗi] {clean}")), None, Some(clean))
            }
            _ => (None, None, None),
        },
        _ => match v.get("event").and_then(|e| e.as_str()).unwrap_or("") {
            "init" => {
                let cid = v.get("conversation_id").and_then(|s| s.as_str()).unwrap_or("");
                (Some(format!("[khởi tạo] phiên Antigravity {cid}")), None, None)
            }
            "step_update" => {
                let stype = v.pointer("/step_update/step_type").and_then(|s| s.as_str()).unwrap_or("");
                let state = v.pointer("/step_update/state").and_then(|s| s.as_str()).unwrap_or("");
                if stype == "tool" && state == "DONE" {
                    let tname = v.pointer("/step_update/tool_name").and_then(|s| s.as_str()).unwrap_or("tool");
                    (Some(format!("[công cụ] {tname}")), None, None)
                } else if stype == "agent_response" && state == "DONE" {
                    if let Some(txt) = v.pointer("/step_update/text_delta").and_then(|s| s.as_str()).map(str::trim).filter(|s| !s.is_empty()) {
                        let clean = doto_core::redact_secrets(txt);
                        return (Some(clean.clone()), Some(clean), None);
                    }
                    (None, None, None)
                } else {
                    (None, None, None)
                }
            }
            "result" => {
                let resp = v.pointer("/result/response").and_then(|s| s.as_str()).map(doto_core::redact_secrets);
                let status = v.pointer("/result/status").and_then(|s| s.as_str()).unwrap_or("SUCCESS");
                if status != "SUCCESS" {
                    let err =
                        v.pointer("/result/error").and_then(|s| s.as_str()).map(doto_core::redact_secrets).unwrap_or_else(|| format!("Trạng thái {status}"));
                    (Some(format!("[lỗi] {err}")), resp, Some(err))
                } else {
                    (Some("[hoàn tất] phiên Antigravity thành công".into()), resp, None)
                }
            }
            _ => (None, None, None),
        },
    }
}

pub type PostSessionHook = Arc<dyn Fn(String, PathBuf, String) -> futures_box::BoxFuture<'static, Vec<String>> + Send + Sync>;

mod futures_box {
    pub type BoxFuture<'a, T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + 'a>>;
}

pub struct PreparedSession {
    pub spec: StudioSessionSpec,
    pub binary: PathBuf,
    pub working_dir: PathBuf,
    pub skill_dir: Option<PathBuf>,
    pub output_subdir: PathBuf,
    pub full_prompt: String,
}

pub struct StudioRunner {
    db: Db,
    events: EventBus,
    tools: Arc<ToolManager>,
    cancels: Mutex<HashMap<String, CancellationToken>>,
    binary_overrides: Mutex<HashMap<String, PathBuf>>,
}

impl StudioRunner {
    pub fn new(db: Db, events: EventBus, tools: Arc<ToolManager>) -> Arc<Self> {
        Arc::new(Self { db, events, tools, cancels: Mutex::new(HashMap::new()), binary_overrides: Mutex::new(HashMap::new()) })
    }

    /// Chỉ dùng trong kiểm thử tự động để trỏ provider sang script giả lập an toàn.
    pub fn set_binary_override(&self, provider: &str, path: PathBuf) {
        self.binary_overrides.lock().unwrap().insert(provider.to_string(), path);
    }

    pub fn locate_provider(&self, provider: &str) -> Option<PathBuf> {
        if let Some(p) = self.binary_overrides.lock().unwrap().get(provider) {
            return Some(p.clone());
        }
        let p = providers::by_id(provider)?;
        self.tools.locate(p.as_ref())
    }

    pub async fn catalog(&self) -> Vec<StudioProviderCatalog> {
        let mut out = vec![];
        for (id, vendor, prefix, models) in [
            ("antigravity", "Google DeepMind", "/", default_antigravity_models()),
            ("claude", "Anthropic", "/", default_claude_models()),
            ("codex", "OpenAI", "$", default_codex_models()),
        ] {
            let p = providers::by_id(id).expect("provider");
            let bin = self.locate_provider(id);
            let ver = match &bin {
                Some(b) => run_fixed(b, &["--version"], std::time::Duration::from_secs(5))
                    .await
                    .and_then(|(_, o, e)| providers::parse_version(&o).or_else(|| providers::parse_version(&e))),
                None => None,
            };
            out.push(StudioProviderCatalog {
                id: id.into(),
                name: p.name().into(),
                vendor: vendor.into(),
                installed: bin.is_some(),
                binary_path: bin.map(|b| b.to_string_lossy().into_owned()),
                version: ver,
                skill_prefix: prefix,
                models,
            });
        }
        out
    }

    /// Chuẩn hoá yêu cầu `single` / `multi_same_ai` / `multi_ai` / `custom_batch` thành danh sách `StudioSessionSpec`.
    pub fn expand_specs(req: &StudioRunRequest) -> Result<Vec<StudioSessionSpec>, String> {
        if !req.confirm {
            return Err("cần xác nhận trước khi khởi chạy phiên AI trên máy chủ".into());
        }
        let prompt = req.prompt.trim();
        if prompt.is_empty() || prompt.chars().count() > MAX_PROMPT_CHARS {
            return Err(format!("nội dung yêu cầu phải từ 1 đến {MAX_PROMPT_CHARS} ký tự"));
        }
        let specs: Vec<StudioSessionSpec> = match req.mode.as_str() {
            "single" => {
                let p = req.provider.clone().unwrap_or_else(|| "claude".into());
                vec![StudioSessionSpec {
                    provider: p,
                    model: req.model.clone().unwrap_or_default(),
                    effort: req.effort.clone().unwrap_or_default(),
                    label: Some("Phiên #1".into()),
                }]
            }
            "multi_same_ai" => {
                let n = req.count.unwrap_or(2).clamp(2, MAX_PARALLEL_SESSIONS);
                let p = req.provider.clone().unwrap_or_else(|| "claude".into());
                (1..=n)
                    .map(|i| StudioSessionSpec {
                        provider: p.clone(),
                        model: req.model.clone().unwrap_or_default(),
                        effort: req.effort.clone().unwrap_or_default(),
                        label: Some(format!("Phương án #{i}")),
                    })
                    .collect()
            }
            "multi_ai" | "custom_batch" => {
                if req.sessions.is_empty() {
                    return Err("chọn ít nhất một phiên AI để chạy song song".into());
                }
                if req.sessions.len() > MAX_PARALLEL_SESSIONS {
                    return Err(format!("tối đa {MAX_PARALLEL_SESSIONS} phiên song song mỗi lượt"));
                }
                req.sessions
                    .iter()
                    .enumerate()
                    .map(|(i, s)| StudioSessionSpec {
                        provider: s.provider.trim().to_string(),
                        model: s.model.trim().to_string(),
                        effort: s.effort.trim().to_string(),
                        label: s.label.clone().filter(|l| !l.trim().is_empty()).or_else(|| Some(format!("{} #{}", s.provider.to_uppercase(), i + 1))),
                    })
                    .collect()
            }
            other => return Err(format!("chế độ chạy không hợp lệ: {other}")),
        };

        for s in &specs {
            validate_provider_model_effort(&s.provider, &s.model, &s.effort)?;
        }
        Ok(specs)
    }

    pub async fn launch(
        self: &Arc<Self>,
        title: String,
        skill_id: Option<String>,
        visibility: String,
        prepared: Vec<PreparedSession>,
        on_done: Option<PostSessionHook>,
    ) -> anyhow::Result<StudioRunDetail> {
        anyhow::ensure!(!prepared.is_empty(), "không có phiên nào để chạy");
        let params_json = serde_json::json!({
            "sessionCount": prepared.len(),
            "skillId": skill_id,
            "providers": prepared.iter().map(|p| p.spec.provider.clone()).collect::<Vec<_>>(),
        });
        let job = jobs::create(&self.db, "studio", &title, params_json, 5).await?;
        jobs::mark_running(&self.db, &job.id, Some(format!("{} phiên AI", prepared.len()))).await?;

        let mut session_rows = vec![];
        let mut run_items = vec![];
        for (idx, item) in prepared.into_iter().enumerate() {
            let sid = doto_core::new_id();
            let session_index = idx + 1;
            let (args, stdin_payload, preview) =
                build_invocation(&item.spec.provider, &item.spec.model, &item.spec.effort, &item.full_prompt, item.skill_dir.as_deref(), &item.output_subdir);
            let label = item.spec.label.clone().unwrap_or_else(|| format!("Phiên #{session_index}"));
            let out_sub_str = item.output_subdir.to_string_lossy().into_owned();
            let now = doto_core::now_rfc3339();
            let (sid2, jid2, lbl2, prov2, mod2, eff2, sk2, prev2, out2, now2) = (
                sid.clone(),
                job.id.clone(),
                label.clone(),
                item.spec.provider.clone(),
                item.spec.model.clone(),
                item.spec.effort.clone(),
                skill_id.clone(),
                preview.clone(),
                out_sub_str.clone(),
                now.clone(),
            );
            self.db
                .call(move |c| {
                    c.execute(
                        "INSERT INTO studio_sessions(id,job_id,session_index,label,provider,model,effort,skill_id,status,command_preview,output_subdir,started_at)
                         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,'running',?9,?10,?11)",
                        params![sid2, jid2, session_index as i64, lbl2, prov2, mod2, eff2, sk2, prev2, out2, now2],
                    )?;
                    Ok(())
                })
                .await?;
            let token = CancellationToken::new();
            self.cancels.lock().unwrap().insert(sid.clone(), token.clone());
            session_rows.push(StudioSession {
                id: sid.clone(),
                job_id: job.id.clone(),
                session_index,
                label,
                provider: item.spec.provider.clone(),
                model: item.spec.model.clone(),
                effort: item.spec.effort.clone(),
                skill_id: skill_id.clone(),
                status: "running".into(),
                command_preview: preview,
                output_subdir: out_sub_str,
                result_text: String::new(),
                log: vec![],
                error: None,
                exit_code: None,
                output_video_ids: vec![],
                started_at: Some(now),
                finished_at: None,
            });
            run_items.push((sid, item, args, stdin_payload, token));
        }

        let job_now = jobs::get(&self.db, &job.id).await?.unwrap_or(job);
        self.events.emit("job", &job_now);

        let runner = self.clone();
        let jid = job_now.id.clone();
        let total = run_items.len();
        tokio::spawn(async move {
            let mut handles = vec![];
            for (sid, item, args, stdin_payload, token) in run_items {
                let r2 = runner.clone();
                let jid2 = jid.clone();
                let vis2 = visibility.clone();
                let hook = on_done.clone();
                handles.push(tokio::spawn(async move { r2.execute_one(&jid2, &sid, item, args, stdin_payload, vis2, hook, token).await }));
            }
            let mut succeeded = 0usize;
            let mut canceled = 0usize;
            let mut first_err: Option<String> = None;
            let mut first_video: Option<String> = None;
            for (done_idx, h) in handles.into_iter().enumerate() {
                if let Ok((st, err, vids)) = h.await {
                    match st.as_str() {
                        "succeeded" => succeeded += 1,
                        "canceled" => canceled += 1,
                        _ => {
                            if first_err.is_none() {
                                first_err = err;
                            }
                        }
                    }
                    if first_video.is_none() {
                        first_video = vids.into_iter().next();
                    }
                }
                let _ = jobs::update_progress(&runner.db, &jid, (done_idx + 1) as f64 / total as f64, None, None).await;
                if let Ok(Some(j)) = jobs::get(&runner.db, &jid).await {
                    runner.events.emit("job", &j);
                }
            }
            let (final_status, final_err) = if succeeded == total {
                ("succeeded", None)
            } else if canceled == total {
                ("canceled", None)
            } else if succeeded > 0 {
                ("succeeded", Some(format!("hoàn tất {succeeded}/{total} phiên ({})", first_err.unwrap_or_default())))
            } else {
                ("failed", Some(first_err.unwrap_or_else(|| "tất cả các phiên AI đều thất bại".into())))
            };
            let _ = jobs::finish(&runner.db, &jid, final_status, final_err, first_video).await;
            if let Ok(Some(j)) = jobs::get(&runner.db, &jid).await {
                runner.events.emit("job", &j);
            }
        });

        Ok(StudioRunDetail { job: job_now, sessions: session_rows })
    }

    #[allow(clippy::too_many_arguments)]
    async fn execute_one(
        &self,
        job_id: &str,
        session_id: &str,
        item: PreparedSession,
        args: Vec<String>,
        stdin_payload: Option<String>,
        visibility: String,
        hook: Option<PostSessionHook>,
        cancel: CancellationToken,
    ) -> (String, Option<String>, Vec<String>) {
        let _ = tokio::fs::create_dir_all(&item.output_subdir).await;
        let _ = tokio::fs::create_dir_all(&item.working_dir).await;
        self.append_session_log(session_id, &format!("$ {}", item.spec.provider)).await;
        let _ = jobs::append_log(&self.db, job_id, &format!("[{}] khởi chạy {}", item.spec.provider, item.output_subdir.display())).await;

        let mut cmd = wrap_cli_command(&item.binary, &args);
        cmd.current_dir(&item.working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(if stdin_payload.is_some() { Stdio::piped() } else { Stdio::null() })
            .env("NO_COLOR", "1")
            .env("DOTO_OUTPUT_DIR", &item.output_subdir);

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                let msg = format!("không khởi chạy được {}: {e}", item.binary.display());
                self.finish_session(session_id, "failed", Some(msg.clone()), None, vec![]).await;
                self.cancels.lock().unwrap().remove(session_id);
                return ("failed".into(), Some(msg), vec![]);
            }
        };
        let pid = child.id();

        if let (Some(payload), Some(mut stdin)) = (stdin_payload, child.stdin.take()) {
            tokio::spawn(async move {
                let _ = stdin.write_all(payload.as_bytes()).await;
                let _ = stdin.shutdown().await;
            });
        }

        let stdout = child.stdout.take().expect("piped");
        let stderr = child.stderr.take().expect("piped");

        let me_out = self.db.clone();
        let ev_out = self.events.clone();
        let sid_out = session_id.to_string();
        let prov = item.spec.provider.clone();
        let stream_err: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let stream_err_cl = stream_err.clone();

        let t_out = tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let (log_msg, res_txt, err_txt) = parse_stream_json_line(&prov, &line);
                if let Some(e) = err_txt {
                    *stream_err_cl.lock().unwrap() = Some(e);
                }
                if log_msg.is_some() || res_txt.is_some() {
                    update_session_stream(&me_out, &ev_out, &sid_out, log_msg.as_deref(), res_txt.as_deref()).await;
                }
            }
        });

        let me_err = self.db.clone();
        let ev_err = self.events.clone();
        let sid_err = session_id.to_string();
        let t_err = tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    let clean = format!("[stderr] {}", doto_core::redact_secrets(trimmed));
                    update_session_stream(&me_err, &ev_err, &sid_err, Some(&clean), None).await;
                }
            }
        });

        let wait_res = tokio::select! {
            _ = cancel.cancelled() => {
                kill_process_tree(&mut child, pid).await;
                Err("CANCELED")
            }
            r = tokio::time::timeout(std::time::Duration::from_secs(45 * 60), child.wait()) => {
                match r {
                    Ok(Ok(status)) => Ok(status),
                    Ok(Err(_)) => Err("WAIT_ERR"),
                    Err(_) => {
                        kill_process_tree(&mut child, pid).await;
                        Err("TIMEOUT")
                    }
                }
            }
        };

        let _ = t_out.await;
        let _ = t_err.await;
        self.cancels.lock().unwrap().remove(session_id);

        let parsed_err = stream_err.lock().unwrap().clone();
        match wait_res {
            Err("CANCELED") => {
                self.finish_session(session_id, "canceled", Some("đã dừng theo yêu cầu".into()), None, vec![]).await;
                let _ = jobs::append_log(&self.db, job_id, &format!("[{}] đã huỷ phiên", item.spec.provider)).await;
                ("canceled".into(), None, vec![])
            }
            Err("TIMEOUT") => {
                let m = "quá thời gian tối đa 45 phút".to_string();
                self.finish_session(session_id, "failed", Some(m.clone()), None, vec![]).await;
                ("failed".into(), Some(m), vec![])
            }
            Err(_) => {
                let m = "lỗi chờ tiến trình con".to_string();
                self.finish_session(session_id, "failed", Some(m.clone()), None, vec![]).await;
                ("failed".into(), Some(m), vec![])
            }
            Ok(status) => {
                let code = status.code();
                if status.success() && parsed_err.is_none() {
                    let vids = match hook {
                        Some(h) => h(job_id.to_string(), item.output_subdir.clone(), visibility).await,
                        None => vec![],
                    };
                    self.finish_session(session_id, "succeeded", None, code, vids.clone()).await;
                    let _ = jobs::append_log(&self.db, job_id, &format!("[{}] hoàn tất ({} video mới)", item.spec.provider, vids.len())).await;
                    ("succeeded".into(), None, vids)
                } else {
                    let m = parsed_err.unwrap_or_else(|| format!("tiến trình kết thúc với mã {:?}", code));
                    self.finish_session(session_id, "failed", Some(m.clone()), code, vec![]).await;
                    let _ = jobs::append_log(&self.db, job_id, &format!("[{}] lỗi: {m}", item.spec.provider)).await;
                    ("failed".into(), Some(m), vec![])
                }
            }
        }
    }

    async fn append_session_log(&self, session_id: &str, line: &str) {
        update_session_stream(&self.db, &self.events, session_id, Some(line), None).await;
    }

    async fn finish_session(&self, session_id: &str, status: &str, error: Option<String>, exit_code: Option<i32>, videos: Vec<String>) {
        let (sid, st, err, vids_json, now) = (
            session_id.to_string(),
            status.to_string(),
            error.map(|e| doto_core::redact_secrets(&e)),
            serde_json::to_string(&videos).unwrap_or_else(|_| "[]".into()),
            doto_core::now_rfc3339(),
        );
        let _ = self
            .db
            .call(move |c| {
                c.execute(
                    "UPDATE studio_sessions SET status=?2, error=?3, exit_code=?4, output_video_ids=?5, finished_at=?6 WHERE id=?1",
                    params![sid, st, err, exit_code, vids_json, now],
                )?;
                Ok(())
            })
            .await;
        if let Ok(Some(s)) = self.get_session(session_id).await {
            self.events.emit("studio", &s);
        }
    }

    pub async fn cancel_session(&self, session_id: &str) -> bool {
        if let Some(tok) = self.cancels.lock().unwrap().get(session_id).cloned() {
            tok.cancel();
            return true;
        }
        false
    }

    pub async fn cancel_job(&self, job_id: &str) -> anyhow::Result<usize> {
        let sessions = self.list_sessions_for_job(job_id).await?;
        let mut n = 0;
        for s in sessions {
            if self.cancel_session(&s.id).await {
                n += 1;
            }
        }
        Ok(n)
    }

    pub async fn get_session(&self, session_id: &str) -> anyhow::Result<Option<StudioSession>> {
        let sid = session_id.to_string();
        self.db.call(move |c| Ok(c.query_row(&format!("SELECT {SESSION_COLS} FROM studio_sessions WHERE id=?1"), [sid], session_from_row).optional()?)).await
    }

    pub async fn list_sessions_for_job(&self, job_id: &str) -> anyhow::Result<Vec<StudioSession>> {
        let jid = job_id.to_string();
        self.db
            .call(move |c| {
                let mut st = c.prepare(&format!("SELECT {SESSION_COLS} FROM studio_sessions WHERE job_id=?1 ORDER BY session_index ASC"))?;
                let v = st.query_map([jid], session_from_row)?.collect::<Result<Vec<_>, _>>()?;
                Ok(v)
            })
            .await
    }

    pub async fn list_runs(&self, limit: u32) -> anyhow::Result<Vec<StudioRunDetail>> {
        let lim = limit.clamp(1, 100);
        let studio_jobs: Vec<jobs::Job> = self
            .db
            .call(move |c| {
                let mut st = c.prepare(
                    "SELECT id,kind,status,title,progress,eta_seconds,speed,encoder,error,attempts,output_video_id,created_at,started_at,finished_at,priority
                     FROM jobs WHERE kind='studio' ORDER BY created_at DESC LIMIT ?1",
                )?;
                let rows = st
                    .query_map([lim], |r| {
                        Ok(jobs::Job {
                            id: r.get(0)?,
                            kind: r.get(1)?,
                            status: r.get(2)?,
                            title: r.get(3)?,
                            progress: r.get(4)?,
                            eta_seconds: r.get(5)?,
                            speed: r.get(6)?,
                            encoder: r.get(7)?,
                            error: r.get(8)?,
                            attempts: r.get(9)?,
                            output_video_id: r.get(10)?,
                            created_at: r.get(11)?,
                            started_at: r.get(12)?,
                            finished_at: r.get(13)?,
                            priority: r.get(14)?,
                        })
                    })?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(rows)
            })
            .await?;
        let mut out = Vec::with_capacity(studio_jobs.len());
        for j in studio_jobs {
            let sessions = self.list_sessions_for_job(&j.id).await?;
            out.push(StudioRunDetail { job: j, sessions });
        }
        Ok(out)
    }

    pub async fn get_run(&self, job_id: &str) -> anyhow::Result<Option<StudioRunDetail>> {
        let Some(j) = jobs::get(&self.db, job_id).await? else { return Ok(None) };
        if j.kind != "studio" {
            return Ok(None);
        }
        let sessions = self.list_sessions_for_job(job_id).await?;
        Ok(Some(StudioRunDetail { job: j, sessions }))
    }
}

const SESSION_COLS: &str =
    "id,job_id,session_index,label,provider,model,effort,skill_id,status,command_preview,output_subdir,result_text,log,error,exit_code,output_video_ids,started_at,finished_at";

fn session_from_row(r: &rusqlite::Row) -> rusqlite::Result<StudioSession> {
    let raw_log: String = r.get(12)?;
    let raw_vids: String = r.get(15)?;
    Ok(StudioSession {
        id: r.get(0)?,
        job_id: r.get(1)?,
        session_index: r.get::<_, i64>(2)? as usize,
        label: r.get(3)?,
        provider: r.get(4)?,
        model: r.get(5)?,
        effort: r.get(6)?,
        skill_id: r.get(7)?,
        status: r.get(8)?,
        command_preview: r.get(9)?,
        output_subdir: r.get(10)?,
        result_text: r.get(11)?,
        log: raw_log.lines().map(String::from).collect(),
        error: r.get(13)?,
        exit_code: r.get(14)?,
        output_video_ids: serde_json::from_str(&raw_vids).unwrap_or_default(),
        started_at: r.get(16)?,
        finished_at: r.get(17)?,
    })
}

async fn update_session_stream(db: &Db, events: &EventBus, session_id: &str, log_line: Option<&str>, result_text: Option<&str>) {
    let sid = session_id.to_string();
    let add_line = log_line.map(|l| doto_core::redact_secrets(l.trim_end()));
    let new_res = result_text.map(doto_core::redact_secrets);
    let sid2 = sid.clone();
    let updated = db
        .call(move |c| {
            if let Some(line) = add_line {
                let mut cur: String = c.query_row("SELECT log FROM studio_sessions WHERE id=?1", [&sid2], |r| r.get(0)).unwrap_or_default();
                cur.push_str(&line);
                cur.push('\n');
                if cur.len() > MAX_SESSION_LOG_BYTES {
                    let cut = cur.len() - MAX_SESSION_LOG_BYTES;
                    let idx = cur[cut..].find('\n').map(|i| cut + i + 1).unwrap_or(cut);
                    cur = format!("…(đã cắt bớt)\n{}", &cur[idx..]);
                }
                c.execute("UPDATE studio_sessions SET log=?2 WHERE id=?1", params![sid2, cur])?;
            }
            if let Some(res) = new_res {
                c.execute("UPDATE studio_sessions SET result_text=?2 WHERE id=?1", params![sid2, res])?;
            }
            Ok(c.query_row(&format!("SELECT {SESSION_COLS} FROM studio_sessions WHERE id=?1"), [&sid2], session_from_row).optional()?)
        })
        .await;
    if let Ok(Some(s)) = updated {
        events.emit("studio", &s);
    }
}

fn wrap_cli_command(binary: &Path, args: &[String]) -> tokio::process::Command {
    let mut cmd = command(binary);
    cmd.args(args);
    cmd
}

async fn kill_process_tree(child: &mut tokio::process::Child, pid: Option<u32>) {
    #[cfg(windows)]
    if let Some(p) = pid {
        let _ = command(Path::new("taskkill")).args(["/PID", &p.to_string(), "/T", "/F"]).output().await;
    }
    #[cfg(not(windows))]
    let _ = pid;
    let _ = child.kill().await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_all_four_modes_and_validates() {
        let mut req = StudioRunRequest {
            mode: "single".into(),
            prompt: "Dựng video bản tin 60s".into(),
            title: None,
            skill_id: None,
            provider: Some("claude".into()),
            model: Some("claude-sonnet-4-6".into()),
            effort: Some("high".into()),
            count: None,
            sessions: vec![],
            upload_ids: vec![],
            include_memory_hint: false,
            visibility: None,
            confirm: false,
        };
        assert!(StudioRunner::expand_specs(&req).is_err(), "thiếu confirm phải từ chối");
        req.confirm = true;
        assert_eq!(StudioRunner::expand_specs(&req).unwrap().len(), 1);

        req.mode = "multi_same_ai".into();
        req.count = Some(3);
        assert_eq!(StudioRunner::expand_specs(&req).unwrap().len(), 3);

        req.mode = "multi_ai".into();
        req.sessions = vec![
            StudioSessionSpec { provider: "antigravity".into(), model: "gemini-2.5-pro".into(), effort: "high".into(), label: None },
            StudioSessionSpec { provider: "claude".into(), model: "claude-opus-4-6".into(), effort: "high".into(), label: None },
            StudioSessionSpec { provider: "codex".into(), model: "gpt-5.4".into(), effort: "medium".into(), label: None },
        ];
        assert_eq!(StudioRunner::expand_specs(&req).unwrap().len(), 3);

        req.sessions[0].model = "bad;rm -rf".into();
        assert!(StudioRunner::expand_specs(&req).is_err());
    }

    #[test]
    fn parses_stream_json_and_redacts_tokens() {
        let claude_line = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Xin chào ghp_secretToken1234567890"}]}}"#;
        let (log, res, err) = parse_stream_json_line("claude", claude_line);
        assert!(err.is_none());
        assert!(!log.unwrap().contains("ghp_secret"));
        assert!(res.unwrap().contains("[REDACTED]"));

        let codex_line = r#"{"type":"item.completed","item":{"type":"agent_message","text":"Đã tạo xong video"}}"#;
        let (_, res2, _) = parse_stream_json_line("codex", codex_line);
        assert_eq!(res2.as_deref(), Some("Đã tạo xong video"));

        let agy_line = r#"{"event":"result","result":{"status":"SUCCESS","response":"Hoàn thành bản nháp"}}"#;
        let (_, res3, _) = parse_stream_json_line("antigravity", agy_line);
        assert_eq!(res3.as_deref(), Some("Hoàn thành bản nháp"));
    }

    #[tokio::test]
    async fn executes_parallel_sessions_with_isolated_outputs() {
        let tmp = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let events = EventBus::default();
        let tools = ToolManager::new(db.clone(), events.clone(), tmp.path());
        let runner = StudioRunner::new(db.clone(), events, tools);

        // Tạo script giả lập an toàn không qua shell thực sự
        #[cfg(windows)]
        let fake_bin = {
            let p = tmp.path().join("fake-ai.cmd");
            std::fs::write(&p, "@echo off\r\necho {\"type\":\"result\",\"result\":\"OK from fake AI\"}\r\n").unwrap();
            p
        };
        #[cfg(unix)]
        let fake_bin = {
            use std::os::unix::fs::PermissionsExt;
            let p = tmp.path().join("fake-ai.sh");
            std::fs::write(&p, "#!/bin/sh\necho '{\"type\":\"result\",\"result\":\"OK from fake AI\"}'\n").unwrap();
            let mut perms = std::fs::metadata(&p).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&p, perms).unwrap();
            p
        };
        runner.set_binary_override("claude", fake_bin.clone());
        runner.set_binary_override("codex", fake_bin);

        let prepared = vec![
            PreparedSession {
                spec: StudioSessionSpec { provider: "claude".into(), model: "".into(), effort: "high".into(), label: Some("S1".into()) },
                binary: runner.locate_provider("claude").unwrap(),
                working_dir: tmp.path().join("w1"),
                skill_dir: None,
                output_subdir: tmp.path().join("out-s1"),
                full_prompt: "Test 1".into(),
            },
            PreparedSession {
                spec: StudioSessionSpec { provider: "codex".into(), model: "".into(), effort: "medium".into(), label: Some("S2".into()) },
                binary: runner.locate_provider("codex").unwrap(),
                working_dir: tmp.path().join("w2"),
                skill_dir: None,
                output_subdir: tmp.path().join("out-s2"),
                full_prompt: "Test 2".into(),
            },
        ];

        let run = runner.launch("Chạy song song 2 AI".into(), None, "private".into(), prepared, None).await.unwrap();
        assert_eq!(run.sessions.len(), 2);

        for _ in 0..40 {
            let cur = runner.get_run(&run.job.id).await.unwrap().unwrap();
            if cur.job.status == "succeeded" {
                assert_eq!(cur.sessions[0].status, "succeeded");
                assert_eq!(cur.sessions[1].status, "succeeded");
                assert!(tmp.path().join("out-s1").is_dir());
                assert!(tmp.path().join("out-s2").is_dir());
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        panic!("timeout chờ studio run hoàn tất");
    }
}
