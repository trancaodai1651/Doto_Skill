//! SkillService: quản lý không giới hạn cứng số nguồn (thư mục local + GitHub), lập chỉ mục vào SQLite,
//! watcher có debounce, kiểm tra/cập nhật GitHub có xem diff, cài skill sang provider có xác nhận.
//! Trạng thái ban đầu: 0 nguồn, 0 skill — không cài sẵn, không tự tải.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rusqlite::{params, OptionalExtension, Row};
use serde::Serialize;

use doto_core::{events::EventBus, paths::AppPaths, secrets::SharedSecrets, Db};

use crate::github::{GithubClient, GithubError};
use crate::mapping::{self, Provider};
use crate::{github_url, scanner};

pub const GITHUB_TOKEN_KEY: &str = "github.token";

#[derive(Debug, thiserror::Error)]
pub enum SkillError {
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error(transparent)]
    Github(#[from] GithubError),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillSource {
    pub id: String,
    pub kind: String,
    pub location: String,
    pub display_name: String,
    pub git_ref: Option<String>,
    pub sub_path: Option<String>,
    pub current_sha: Option<String>,
    pub pinned: bool,
    pub enabled: bool,
    pub watch: bool,
    pub private: bool,
    pub skill_count: i64,
    pub last_scan_at: Option<String>,
    pub last_error: Option<String>,
    pub created_at: String,
    #[serde(skip)]
    pub local_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Skill {
    pub id: String,
    pub source_id: String,
    pub source_name: String,
    pub canonical_id: String,
    pub name: String,
    pub description: String,
    pub rel_path: String,
    pub revision: String,
    pub has_scripts: bool,
    pub warnings: Vec<String>,
    pub duplicate_of: Vec<String>,
    pub installed_providers: Vec<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillDetail {
    #[serde(flatten)]
    pub skill: Skill,
    pub content: String,
    pub files: Vec<scanner::SkillFile>,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheck {
    pub current_sha: Option<String>,
    pub latest_sha: String,
    pub up_to_date: bool,
    pub changed_files: Vec<crate::github::ChangedFile>,
    pub compare_url: Option<String>,
}

const SRC_COLS: &str = "s.id,s.kind,s.location,s.display_name,s.git_ref,s.sub_path,s.current_sha,s.pinned,s.enabled,s.watch,s.private,
  (SELECT count(*) FROM skills k WHERE k.source_id=s.id),s.last_scan_at,s.last_error,s.created_at,s.local_path";

fn src_row(r: &Row) -> rusqlite::Result<SkillSource> {
    Ok(SkillSource {
        id: r.get(0)?,
        kind: r.get(1)?,
        location: r.get(2)?,
        display_name: r.get(3)?,
        git_ref: r.get(4)?,
        sub_path: r.get(5)?,
        current_sha: r.get(6)?,
        pinned: r.get::<_, i64>(7)? != 0,
        enabled: r.get::<_, i64>(8)? != 0,
        watch: r.get::<_, i64>(9)? != 0,
        private: r.get::<_, i64>(10)? != 0,
        skill_count: r.get(11)?,
        last_scan_at: r.get(12)?,
        last_error: r.get(13)?,
        created_at: r.get(14)?,
        local_path: r.get(15)?,
    })
}

type Debouncer = notify_debouncer_full::Debouncer<notify::RecommendedWatcher, notify_debouncer_full::RecommendedCache>;

pub struct SkillService {
    db: Db,
    paths: AppPaths,
    events: EventBus,
    secrets: SharedSecrets,
    api_base: String,
    web_base: String,
    watchers: Mutex<HashMap<String, Debouncer>>,
    scan_lock: tokio::sync::Mutex<()>,
}

impl SkillService {
    pub fn new(db: Db, paths: AppPaths, events: EventBus, secrets: SharedSecrets) -> Arc<Self> {
        Self::with_github_bases(db, paths, events, secrets, "https://api.github.com", "https://github.com")
    }

    pub fn with_github_bases(db: Db, paths: AppPaths, events: EventBus, secrets: SharedSecrets, api: &str, web: &str) -> Arc<Self> {
        Arc::new(Self {
            db,
            paths,
            events,
            secrets,
            api_base: api.into(),
            web_base: web.into(),
            watchers: Mutex::new(HashMap::new()),
            scan_lock: tokio::sync::Mutex::new(()),
        })
    }

    pub fn github_client(&self) -> GithubClient {
        let token = self.secrets.get(GITHUB_TOKEN_KEY).ok().flatten();
        GithubClient::with_bases(token, &self.api_base, &self.web_base)
    }

    pub fn github_anonymous(&self) -> GithubClient {
        GithubClient::with_bases(None, &self.api_base, &self.web_base)
    }

    // ---------------- Nguồn ----------------

    pub async fn list_sources(&self) -> anyhow::Result<Vec<SkillSource>> {
        self.db
            .call(|c| {
                let mut st = c.prepare(&format!("SELECT {SRC_COLS} FROM skill_sources s ORDER BY s.created_at"))?;
                let v = st.query_map([], src_row)?.collect::<Result<Vec<_>, _>>()?;
                Ok(v)
            })
            .await
    }

    pub async fn get_source(&self, id: &str) -> Result<SkillSource, SkillError> {
        let id2 = id.to_string();
        self.db
            .call(move |c| Ok(c.query_row(&format!("SELECT {SRC_COLS} FROM skill_sources s WHERE s.id=?1"), [id2], src_row).optional()?))
            .await?
            .ok_or_else(|| SkillError::NotFound("không tìm thấy nguồn skill".into()))
    }

    pub async fn add_folder(self: &Arc<Self>, path: &str, watch: bool) -> Result<SkillSource, SkillError> {
        let p = PathBuf::from(path.trim());
        if !p.is_absolute() {
            return Err(SkillError::Validation("cần đường dẫn tuyệt đối".into()));
        }
        let canon = doto_core::paths::dunce_canonical(&p).filter(|c| c.is_dir()).ok_or_else(|| SkillError::Validation("thư mục không tồn tại".into()))?;
        let location = canon.to_string_lossy().into_owned();
        let name = canon.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| location.clone());
        let id = doto_core::new_id();
        let (id2, loc2) = (id.clone(), location.clone());
        self.db
            .call(move |c| {
                let exists: Option<String> =
                    c.query_row("SELECT id FROM skill_sources WHERE kind='folder' AND location=?1", [&loc2], |r| r.get(0)).optional()?;
                anyhow::ensure!(exists.is_none(), "DUP");
                c.execute(
                    "INSERT INTO skill_sources(id,kind,location,display_name,local_path,watch,created_at) VALUES(?1,'folder',?2,?3,?2,?4,?5)",
                    params![id2, loc2, name, watch as i64, doto_core::now_rfc3339()],
                )?;
                Ok(())
            })
            .await
            .map_err(|e| if e.to_string() == "DUP" { SkillError::Conflict("thư mục này đã được thêm".into()) } else { SkillError::Other(e) })?;
        self.rescan(&id).await?;
        if watch {
            self.start_watch(&id, &canon);
        }
        self.get_source(&id).await
    }

    pub async fn add_github(self: &Arc<Self>, url: &str, git_ref: Option<&str>) -> Result<SkillSource, SkillError> {
        let t = github_url::parse(url, git_ref).map_err(SkillError::Validation)?;
        let client = self.github_client();
        // Kiểm tra quyền truy cập + private
        let info = match client.repo(&t.owner, &t.repo).await {
            Ok(i) => i,
            Err(GithubError::NotFound) if client_has_token(&self.secrets) => return Err(GithubError::NotFound.into()),
            Err(GithubError::NotFound) => {
                return Err(SkillError::Validation("không thấy repo — nếu là repo private, hãy đăng nhập GitHub trước (Skills › Kết nối GitHub)".into()))
            }
            Err(e) => return Err(e.into()),
        };
        let sha = client.resolve_commit(&t.owner, &t.repo, t.git_ref.as_deref()).await?;
        let location = t.canonical_repo_url();
        let id = doto_core::new_id();
        let display = match &t.sub_path {
            Some(p) => format!("{}/{}:{}", t.owner, t.repo, p),
            None => format!("{}/{}", t.owner, t.repo),
        };
        let dest = self.paths.skills_cache_dir().join(&id).join(&sha[..12]);
        let report = client.download_and_extract(&t.owner, &t.repo, &sha, t.sub_path.as_deref(), &dest).await?;
        if report.files == 0 {
            let _ = std::fs::remove_dir_all(self.paths.skills_cache_dir().join(&id));
            return Err(SkillError::Validation("không có file nào ở đường dẫn đã chọn trong repo".into()));
        }
        let (id2, loc, gref, sub, dest_s, private) =
            (id.clone(), location, t.git_ref.clone(), t.sub_path.clone(), dest.to_string_lossy().into_owned(), info.private);
        let sha2 = sha.clone();
        let res = self
            .db
            .call(move |c| {
                let exists: Option<String> = c
                    .query_row(
                        "SELECT id FROM skill_sources WHERE kind='github' AND location=?1 AND sub_path IS ?2 AND git_ref IS ?3",
                        params![loc, sub, gref],
                        |r| r.get(0),
                    )
                    .optional()?;
                anyhow::ensure!(exists.is_none(), "DUP");
                c.execute(
                    "INSERT INTO skill_sources(id,kind,location,display_name,git_ref,sub_path,current_sha,local_path,private,created_at)
                     VALUES(?1,'github',?2,?3,?4,?5,?6,?7,?8,?9)",
                    params![id2, loc, display, gref, sub, sha2, dest_s, private as i64, doto_core::now_rfc3339()],
                )?;
                Ok(())
            })
            .await;
        if let Err(e) = res {
            let _ = std::fs::remove_dir_all(self.paths.skills_cache_dir().join(&id));
            return Err(if e.to_string() == "DUP" { SkillError::Conflict("nguồn GitHub này đã được thêm".into()) } else { SkillError::Other(e) });
        }
        self.rescan(&id).await?;
        self.get_source(&id).await
    }

    pub async fn set_flags(self: &Arc<Self>, id: &str, enabled: Option<bool>, watch: Option<bool>, pinned: Option<bool>) -> Result<SkillSource, SkillError> {
        let src = self.get_source(id).await?;
        if watch == Some(true) && src.kind != "folder" {
            return Err(SkillError::Validation("chỉ thư mục local mới có watcher".into()));
        }
        let id2 = id.to_string();
        self.db
            .call(move |c| {
                c.execute(
                    "UPDATE skill_sources SET enabled=COALESCE(?2,enabled), watch=COALESCE(?3,watch), pinned=COALESCE(?4,pinned) WHERE id=?1",
                    params![id2, enabled.map(|b| b as i64), watch.map(|b| b as i64), pinned.map(|b| b as i64)],
                )?;
                Ok(())
            })
            .await?;
        let src = self.get_source(id).await?;
        if src.watch && src.enabled && src.kind == "folder" {
            self.start_watch(id, Path::new(&src.location));
        } else {
            self.watchers.lock().unwrap().remove(id);
        }
        Ok(src)
    }

    pub async fn remove(&self, id: &str) -> Result<(), SkillError> {
        let src = self.get_source(id).await?;
        self.watchers.lock().unwrap().remove(id);
        let id2 = id.to_string();
        self.db
            .call(move |c| {
                c.execute("DELETE FROM skill_sources WHERE id=?1", [id2])?;
                Ok(())
            })
            .await?;
        if src.kind == "github" {
            let _ = std::fs::remove_dir_all(self.paths.skills_cache_dir().join(id));
        }
        self.events.emit("skills", &serde_json::json!({ "sourceId": id }));
        Ok(())
    }

    /// Quét lại nguồn và đồng bộ bảng `skills` trong một transaction.
    pub async fn rescan(&self, id: &str) -> Result<SkillSource, SkillError> {
        let _g = self.scan_lock.lock().await;
        let src = self.get_source(id).await?;
        let root = PathBuf::from(src.local_path.clone().unwrap_or_else(|| src.location.clone()));
        let scan = tokio::task::spawn_blocking(move || scanner::scan(&root)).await.map_err(anyhow::Error::from)?;
        let id2 = id.to_string();
        let (skills, err) = match scan {
            Ok((s, errors)) => (s, (!errors.is_empty()).then(|| errors.join("; ").chars().take(1000).collect::<String>())),
            Err(e) => (vec![], Some(e.to_string())),
        };
        self.db
            .call(move |c| {
                let tx = c.transaction()?;
                let now = doto_core::now_rfc3339();
                let mut keep = vec![];
                for s in &skills {
                    let existing: Option<String> =
                        tx.query_row("SELECT id FROM skills WHERE source_id=?1 AND rel_path=?2", params![id2, s.rel_path], |r| r.get(0)).optional()?;
                    let sid = existing.unwrap_or_else(doto_core::new_id);
                    tx.execute(
                        "INSERT INTO skills(id,source_id,canonical_id,name,description,rel_path,revision,metadata,has_scripts,warnings,created_at,updated_at)
                         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?11)
                         ON CONFLICT(source_id,rel_path) DO UPDATE SET canonical_id=excluded.canonical_id, name=excluded.name,
                           description=excluded.description, metadata=excluded.metadata, has_scripts=excluded.has_scripts, warnings=excluded.warnings,
                           updated_at=CASE WHEN skills.revision<>excluded.revision THEN excluded.updated_at ELSE skills.updated_at END,
                           revision=excluded.revision",
                        params![
                            sid,
                            id2,
                            s.canonical_id,
                            s.name,
                            s.description,
                            s.rel_path,
                            s.revision,
                            s.metadata.to_string(),
                            s.has_scripts as i64,
                            serde_json::to_string(&s.warnings)?,
                            now
                        ],
                    )?;
                    keep.push(s.rel_path.clone());
                }
                let mut st = tx.prepare("SELECT id, rel_path FROM skills WHERE source_id=?1")?;
                let all: Vec<(String, String)> = st.query_map([&id2], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<_, _>>()?;
                drop(st);
                for (sid, rel) in all {
                    if !keep.contains(&rel) {
                        tx.execute("DELETE FROM skills WHERE id=?1", [sid])?;
                    }
                }
                tx.execute("UPDATE skill_sources SET last_scan_at=?2, last_error=?3 WHERE id=?1", params![id2, now, err])?;
                tx.commit()?;
                Ok(())
            })
            .await?;
        self.events.emit("skills", &serde_json::json!({ "sourceId": id }));
        self.get_source(id).await
    }

    fn start_watch(self: &Arc<Self>, id: &str, root: &Path) {
        use notify::RecursiveMode;
        let me = Arc::downgrade(self);
        let sid = id.to_string();
        let handle = tokio::runtime::Handle::current();
        let deb = notify_debouncer_full::new_debouncer(Duration::from_millis(1500), None, move |res: notify_debouncer_full::DebounceEventResult| {
            if res.is_ok() {
                if let Some(me) = me.upgrade() {
                    let sid = sid.clone();
                    handle.spawn(async move {
                        if let Err(e) = me.rescan(&sid).await {
                            tracing::warn!(error = %e, "watcher rescan lỗi");
                        }
                    });
                }
            }
        });
        match deb {
            Ok(mut d) => {
                if let Err(e) = d.watch(root, RecursiveMode::Recursive) {
                    tracing::warn!(error = %e, "không theo dõi được thư mục skill");
                    return;
                }
                self.watchers.lock().unwrap().insert(id.to_string(), d);
            }
            Err(e) => tracing::warn!(error = %e, "không tạo được watcher"),
        }
    }

    /// Khởi động watcher cho các nguồn đã bật watch (gọi khi app khởi động).
    pub async fn restore_watchers(self: &Arc<Self>) {
        if let Ok(sources) = self.list_sources().await {
            for s in sources.into_iter().filter(|s| s.watch && s.enabled && s.kind == "folder") {
                self.start_watch(&s.id, Path::new(&s.location));
            }
        }
    }

    // ---------------- GitHub update ----------------

    pub async fn check_update(&self, id: &str) -> Result<UpdateCheck, SkillError> {
        let src = self.get_source(id).await?;
        if src.kind != "github" {
            return Err(SkillError::Validation("chỉ nguồn GitHub mới kiểm tra cập nhật".into()));
        }
        let t = github_url::parse(&src.location, src.git_ref.as_deref()).map_err(SkillError::Validation)?;
        let client = self.github_client();
        let latest = client.resolve_commit(&t.owner, &t.repo, src.git_ref.as_deref()).await?;
        let current = src.current_sha.clone();
        if current.as_deref() == Some(latest.as_str()) {
            return Ok(UpdateCheck { current_sha: current, latest_sha: latest, up_to_date: true, changed_files: vec![], compare_url: None });
        }
        let (mut files, url) = match &current {
            Some(cur) => client.compare(&t.owner, &t.repo, cur, &latest).await?,
            None => (vec![], String::new()),
        };
        if let Some(sp) = &src.sub_path {
            let prefix = format!("{sp}/");
            files.retain(|f| f.filename.starts_with(&prefix));
        }
        Ok(UpdateCheck { current_sha: current, latest_sha: latest, up_to_date: false, changed_files: files, compare_url: (!url.is_empty()).then_some(url) })
    }

    /// Áp dụng một SHA đã xem diff. Từ chối nếu nguồn đang pin.
    pub async fn apply_update(&self, id: &str, sha: &str) -> Result<SkillSource, SkillError> {
        let src = self.get_source(id).await?;
        if src.kind != "github" {
            return Err(SkillError::Validation("không phải nguồn GitHub".into()));
        }
        if src.pinned {
            return Err(SkillError::Conflict("nguồn đang được pin commit — bỏ pin trước khi cập nhật".into()));
        }
        if sha.len() != 40 || !sha.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(SkillError::Validation("SHA không hợp lệ".into()));
        }
        let t = github_url::parse(&src.location, None).map_err(SkillError::Validation)?;
        let dest = self.paths.skills_cache_dir().join(id).join(&sha[..12]);
        let _ = std::fs::remove_dir_all(&dest);
        self.github_client().download_and_extract(&t.owner, &t.repo, sha, src.sub_path.as_deref(), &dest).await?;
        let (id2, sha2, dest_s) = (id.to_string(), sha.to_string(), dest.to_string_lossy().into_owned());
        self.db
            .call(move |c| {
                c.execute("UPDATE skill_sources SET current_sha=?2, local_path=?3 WHERE id=?1", params![id2, sha2, dest_s])?;
                Ok(())
            })
            .await?;
        if let Some(old) = src.local_path.filter(|p| Path::new(p) != dest) {
            let _ = std::fs::remove_dir_all(old);
        }
        self.rescan(id).await
    }

    // ---------------- Skills ----------------

    pub async fn list_skills(&self, q: Option<String>, source_id: Option<String>) -> anyhow::Result<Vec<Skill>> {
        self.db
            .call(move |c| {
                let mut sql = String::from(
                    "SELECT k.id,k.source_id,s.display_name,k.canonical_id,k.name,k.description,k.rel_path,k.revision,k.has_scripts,k.warnings,k.updated_at
                     FROM skills k JOIN skill_sources s ON s.id=k.source_id WHERE s.enabled=1",
                );
                let mut args: Vec<String> = vec![];
                if let Some(sid) = source_id.filter(|s| !s.is_empty()) {
                    args.push(sid);
                    sql.push_str(&format!(" AND k.source_id=?{}", args.len()));
                }
                if let Some(q) = q.filter(|q| !q.trim().is_empty()) {
                    args.push(format!("%{}%", q.trim()));
                    sql.push_str(&format!(" AND (k.name LIKE ?{0} OR k.description LIKE ?{0} OR k.canonical_id LIKE ?{0})", args.len()));
                }
                sql.push_str(" ORDER BY k.name COLLATE NOCASE LIMIT 5000");
                let mut st = c.prepare(&sql)?;
                let mut v: Vec<Skill> = st
                    .query_map(rusqlite::params_from_iter(args.iter()), |r| {
                        let w: String = r.get(9)?;
                        Ok(Skill {
                            id: r.get(0)?,
                            source_id: r.get(1)?,
                            source_name: r.get(2)?,
                            canonical_id: r.get(3)?,
                            name: r.get(4)?,
                            description: r.get(5)?,
                            rel_path: r.get(6)?,
                            revision: r.get(7)?,
                            has_scripts: r.get::<_, i64>(8)? != 0,
                            warnings: serde_json::from_str(&w).unwrap_or_default(),
                            duplicate_of: vec![],
                            installed_providers: vec![],
                            updated_at: r.get(10)?,
                        })
                    })?
                    .collect::<Result<_, _>>()?;
                // trùng lặp theo canonical_id giữa các nguồn khác nhau
                let mut dup: HashMap<String, Vec<(String, String)>> = HashMap::new();
                let mut st2 = c.prepare("SELECT k.canonical_id, k.id, k.source_id FROM skills k JOIN skill_sources s ON s.id=k.source_id WHERE s.enabled=1")?;
                for row in st2.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)))? {
                    let (cid, id, src) = row?;
                    dup.entry(cid).or_default().push((id, src));
                }
                let mut st3 = c.prepare("SELECT skill_id, provider FROM skill_installs")?;
                let mut inst: HashMap<String, Vec<String>> = HashMap::new();
                for row in st3.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
                    let (sid, p) = row?;
                    inst.entry(sid).or_default().push(p);
                }
                for s in &mut v {
                    if let Some(g) = dup.get(&s.canonical_id) {
                        s.duplicate_of = g.iter().filter(|(id, src)| id != &s.id && src != &s.source_id).map(|(id, _)| id.clone()).collect();
                    }
                    s.installed_providers = inst.remove(&s.id).unwrap_or_default();
                }
                Ok(v)
            })
            .await
    }

    async fn skill_row(&self, id: &str) -> Result<(Skill, SkillSource, serde_json::Value), SkillError> {
        let id2 = id.to_string();
        let (source_id, meta): (String, String) = self
            .db
            .call(move |c| Ok(c.query_row("SELECT source_id, metadata FROM skills WHERE id=?1", [id2], |r| Ok((r.get(0)?, r.get(1)?))).optional()?))
            .await?
            .ok_or_else(|| SkillError::NotFound("không tìm thấy skill".into()))?;
        let src = self.get_source(&source_id).await?;
        let skill = self
            .list_skills(None, Some(source_id))
            .await?
            .into_iter()
            .find(|s| s.id == id)
            .ok_or_else(|| SkillError::NotFound("skill thuộc nguồn đang tạm dừng".into()))?;
        Ok((skill, src, serde_json::from_str(&meta).unwrap_or_default()))
    }

    fn skill_dir(src: &SkillSource, rel: &str) -> Result<PathBuf, SkillError> {
        let root = PathBuf::from(src.local_path.clone().unwrap_or_else(|| src.location.clone()));
        Ok(scanner::resolve_skill_dir(&root, rel)?)
    }

    pub async fn detail(&self, id: &str) -> Result<SkillDetail, SkillError> {
        let (skill, src, metadata) = self.skill_row(id).await?;
        let dir = Self::skill_dir(&src, &skill.rel_path)?;
        let md = std::fs::read_dir(&dir)
            .map_err(anyhow::Error::from)?
            .filter_map(Result::ok)
            .find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case("skill.md"))
            .map(|e| e.path())
            .ok_or_else(|| SkillError::NotFound("SKILL.md không còn trên đĩa — hãy quét lại".into()))?;
        let mut content = std::fs::read_to_string(md).map_err(anyhow::Error::from)?;
        if content.len() > 200_000 {
            content.truncate(200_000);
            content.push_str("\n…(đã cắt bớt)");
        }
        let (files, _) = scanner::list_files(&dir);
        Ok(SkillDetail { skill, content, files, metadata })
    }

    /// Cài skill sang provider. Bắt buộc `confirm`; skill có script cần thêm `allow_scripts`.
    pub async fn install(&self, id: &str, provider: &str, confirm: bool, allow_scripts: bool) -> Result<String, SkillError> {
        let p = Provider::parse(provider).ok_or_else(|| SkillError::Validation("provider không hợp lệ".into()))?;
        if !confirm {
            return Err(SkillError::Validation("cần xác nhận trước khi cài".into()));
        }
        let (skill, src, meta) = self.skill_row(id).await?;
        if skill.has_scripts && !allow_scripts {
            return Err(SkillError::Validation("skill chứa script — hãy xem nội dung và xác nhận cho phép trước khi cài".into()));
        }
        p.validate(&meta).map_err(SkillError::Validation)?;
        let dir = Self::skill_dir(&src, &skill.rel_path)?;
        let provider_dir = p.user_skills_dir().ok_or_else(|| SkillError::Validation("không xác định được thư mục home".into()))?;
        let marker =
            mapping::Marker { skill_id: skill.id.clone(), source_id: src.id.clone(), revision: skill.revision.clone(), installed_at: doto_core::now_rfc3339() };
        let folder = skill.canonical_id.clone();
        let target = tokio::task::spawn_blocking(move || {
            std::fs::create_dir_all(&provider_dir)?;
            mapping::install_copy(&dir, &provider_dir, &folder, &marker)
        })
        .await
        .map_err(anyhow::Error::from)?
        .map_err(|e| SkillError::Conflict(e.to_string()))?;
        let (sid, pid, t, rev) = (skill.id.clone(), p.id().to_string(), target.to_string_lossy().into_owned(), skill.revision.clone());
        self.db
            .call(move |c| {
                c.execute(
                    "INSERT INTO skill_installs(skill_id,provider,target_path,revision,installed_at) VALUES(?1,?2,?3,?4,?5)
                     ON CONFLICT(skill_id,provider) DO UPDATE SET target_path=excluded.target_path, revision=excluded.revision, installed_at=excluded.installed_at",
                    params![sid, pid, t, rev, doto_core::now_rfc3339()],
                )?;
                Ok(())
            })
            .await?;
        Ok(target.to_string_lossy().into_owned())
    }

    pub async fn uninstall(&self, id: &str, provider: &str) -> Result<(), SkillError> {
        let p = Provider::parse(provider).ok_or_else(|| SkillError::Validation("provider không hợp lệ".into()))?;
        let (sid, pid) = (id.to_string(), p.id().to_string());
        let target: Option<String> = self
            .db
            .call(move |c| {
                Ok(c.query_row("SELECT target_path FROM skill_installs WHERE skill_id=?1 AND provider=?2", params![sid, pid], |r| r.get(0)).optional()?)
            })
            .await?;
        let target = target.ok_or_else(|| SkillError::NotFound("skill chưa được cài cho provider này".into()))?;
        let id2 = id.to_string();
        let t = PathBuf::from(&target);
        tokio::task::spawn_blocking(move || mapping::uninstall(&t, &id2))
            .await
            .map_err(anyhow::Error::from)?
            .map_err(|e| SkillError::Conflict(e.to_string()))?;
        let (sid, pid) = (id.to_string(), p.id().to_string());
        self.db
            .call(move |c| {
                c.execute("DELETE FROM skill_installs WHERE skill_id=?1 AND provider=?2", params![sid, pid])?;
                Ok(())
            })
            .await?;
        Ok(())
    }
}

fn client_has_token(s: &SharedSecrets) -> bool {
    s.get(GITHUB_TOKEN_KEY).ok().flatten().is_some()
}
