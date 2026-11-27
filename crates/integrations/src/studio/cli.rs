//! Định nghĩa mô hình dữ liệu, danh mục model mặc định, hàm kiểm tra tham số,
//! sinh đối số cố định (`build_invocation` — không qua shell, tự động chuyển `stdin` khi nhiều dòng)
//! và trình phân tích `stream-json` cho 3 AI CLI (`antigravity`, `claude`, `codex`).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use doto_core::jobs;

pub const MAX_PARALLEL_SESSIONS: usize = 6;
pub const MAX_PROMPT_CHARS: usize = 32_000;

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
    let home = doto_core::paths::user_home_dir();
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
