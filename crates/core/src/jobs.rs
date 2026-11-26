//! Bảng `jobs` dùng chung cho render video, cài CLI, quét thư mục…

use rusqlite::{params, OptionalExtension, Row};
use serde::Serialize;

use crate::Db;

pub const MAX_LOG_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub id: String,
    pub kind: String,
    pub status: String,
    pub title: String,
    pub progress: f64,
    pub eta_seconds: Option<i64>,
    pub speed: Option<String>,
    pub encoder: Option<String>,
    pub error: Option<String>,
    pub attempts: i64,
    pub output_video_id: Option<String>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    #[serde(skip)]
    pub priority: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobDetail {
    #[serde(flatten)]
    pub job: Job,
    pub params: serde_json::Value,
    pub log: Vec<String>,
}

const COLS: &str = "id,kind,status,title,progress,eta_seconds,speed,encoder,error,attempts,output_video_id,created_at,started_at,finished_at,priority";

fn from_row(r: &Row) -> rusqlite::Result<Job> {
    Ok(Job {
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
}

pub async fn create(db: &Db, kind: &str, title: &str, params_json: serde_json::Value, priority: i64) -> anyhow::Result<Job> {
    let id = crate::new_id();
    let (kind, title) = (kind.to_string(), title.to_string());
    let id2 = id.clone();
    db.call(move |c| {
        c.execute(
            "INSERT INTO jobs(id,kind,status,title,priority,params,created_at) VALUES(?1,?2,'queued',?3,?4,?5,?6)",
            params![id2, kind, title, priority, params_json.to_string(), crate::now_rfc3339()],
        )?;
        Ok(())
    })
    .await?;
    get(db, &id).await?.ok_or_else(|| anyhow::anyhow!("job vừa tạo không tồn tại"))
}

pub async fn get(db: &Db, id: &str) -> anyhow::Result<Option<Job>> {
    let id = id.to_string();
    db.call(move |c| Ok(c.query_row(&format!("SELECT {COLS} FROM jobs WHERE id=?1"), [id], from_row).optional()?)).await
}

pub async fn detail(db: &Db, id: &str) -> anyhow::Result<Option<JobDetail>> {
    let id = id.to_string();
    db.call(move |c| {
        let job = c.query_row(&format!("SELECT {COLS} FROM jobs WHERE id=?1"), [&id], from_row).optional()?;
        let Some(job) = job else { return Ok(None) };
        let (p, log): (String, String) = c.query_row("SELECT params, log FROM jobs WHERE id=?1", [&id], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(Some(JobDetail { job, params: serde_json::from_str(&p).unwrap_or_default(), log: log.lines().map(String::from).collect() }))
    })
    .await
}

pub async fn params(db: &Db, id: &str) -> anyhow::Result<serde_json::Value> {
    let id = id.to_string();
    db.call(move |c| {
        let p: String = c.query_row("SELECT params FROM jobs WHERE id=?1", [id], |r| r.get(0))?;
        Ok(serde_json::from_str(&p)?)
    })
    .await
}

pub async fn list(db: &Db, limit: u32) -> anyhow::Result<Vec<Job>> {
    db.call(move |c| {
        let mut st = c.prepare(&format!("SELECT {COLS} FROM jobs ORDER BY created_at DESC LIMIT ?1"))?;
        let v = st.query_map([limit.clamp(1, 500)], from_row)?.collect::<Result<Vec<_>, _>>()?;
        Ok(v)
    })
    .await
}

pub async fn queued(db: &Db, kind: &str) -> anyhow::Result<Vec<Job>> {
    let kind = kind.to_string();
    db.call(move |c| {
        let mut st = c.prepare(&format!("SELECT {COLS} FROM jobs WHERE status='queued' AND kind=?1 ORDER BY priority DESC, created_at ASC"))?;
        let v = st.query_map([kind], from_row)?.collect::<Result<Vec<_>, _>>()?;
        Ok(v)
    })
    .await
}

pub async fn count_by_status(db: &Db, status: &str) -> anyhow::Result<i64> {
    let s = status.to_string();
    db.call(move |c| Ok(c.query_row("SELECT count(*) FROM jobs WHERE status=?1", [s], |r| r.get(0))?)).await
}

pub async fn mark_running(db: &Db, id: &str, encoder: Option<String>) -> anyhow::Result<()> {
    let id = id.to_string();
    db.call(move |c| {
        c.execute(
            "UPDATE jobs SET status='running', started_at=?2, finished_at=NULL, error=NULL, progress=0, eta_seconds=NULL, encoder=?3, attempts=attempts+1 WHERE id=?1",
            params![id, crate::now_rfc3339(), encoder],
        )?;
        Ok(())
    })
    .await
}

pub async fn update_progress(db: &Db, id: &str, progress: f64, eta: Option<i64>, speed: Option<String>) -> anyhow::Result<()> {
    let id = id.to_string();
    db.call(move |c| {
        c.execute("UPDATE jobs SET progress=?2, eta_seconds=?3, speed=?4 WHERE id=?1 AND status='running'", params![id, progress.clamp(0.0, 1.0), eta, speed])?;
        Ok(())
    })
    .await
}

pub async fn set_encoder(db: &Db, id: &str, encoder: &str) -> anyhow::Result<()> {
    let (id, e) = (id.to_string(), encoder.to_string());
    db.call(move |c| {
        c.execute("UPDATE jobs SET encoder=?2 WHERE id=?1", params![id, e])?;
        Ok(())
    })
    .await
}

pub async fn finish(db: &Db, id: &str, status: &str, error: Option<String>, output_video_id: Option<String>) -> anyhow::Result<()> {
    debug_assert!(matches!(status, "succeeded" | "failed" | "canceled"));
    let (id, status) = (id.to_string(), status.to_string());
    db.call(move |c| {
        c.execute(
            "UPDATE jobs SET status=?2, error=?3, output_video_id=?4, finished_at=?5, eta_seconds=NULL,
             progress=CASE WHEN ?2='succeeded' THEN 1 ELSE progress END WHERE id=?1",
            params![id, status, error, output_video_id, crate::now_rfc3339()],
        )?;
        Ok(())
    })
    .await
}

pub async fn requeue(db: &Db, id: &str) -> anyhow::Result<bool> {
    let id = id.to_string();
    db.call(move |c| {
        let n = c.execute(
            "UPDATE jobs SET status='queued', error=NULL, progress=0, eta_seconds=NULL, speed=NULL, started_at=NULL, finished_at=NULL
             WHERE id=?1 AND status IN ('failed','canceled')",
            [id],
        )?;
        Ok(n == 1)
    })
    .await
}

/// Ghi nối log (đã lọc bí mật), cắt bớt phần đầu nếu vượt MAX_LOG_BYTES.
pub async fn append_log(db: &Db, id: &str, line: &str) -> anyhow::Result<()> {
    let id = id.to_string();
    let line = crate::redact_secrets(line.trim_end());
    db.call(move |c| {
        let cur: String = c.query_row("SELECT log FROM jobs WHERE id=?1", [&id], |r| r.get(0)).unwrap_or_default();
        let mut s = cur;
        s.push_str(&line);
        s.push('\n');
        if s.len() > MAX_LOG_BYTES {
            let cut = s.len() - MAX_LOG_BYTES;
            let idx = s[cut..].find('\n').map(|i| cut + i + 1).unwrap_or(cut);
            s = format!("…(đã cắt bớt)\n{}", &s[idx..]);
        }
        c.execute("UPDATE jobs SET log=?2 WHERE id=?1", params![id, s])?;
        Ok(())
    })
    .await
}

/// Sau khi app khởi động lại: job đang `running` bị gián đoạn → đưa lại hàng đợi.
pub async fn recover_interrupted(db: &Db) -> anyhow::Result<usize> {
    db.call(|c| {
        let n = c.execute(
            "UPDATE jobs SET status='queued', progress=0, eta_seconds=NULL,
             log = log || '[khôi phục] ứng dụng đã khởi động lại khi job đang chạy — đưa lại hàng đợi\n'
             WHERE status='running' AND kind='render'",
            [],
        )?;
        let m = c.execute(
            "UPDATE jobs SET status='failed', error='ứng dụng đã thoát khi tác vụ đang chạy', finished_at=?1
             WHERE status IN ('running','queued') AND kind<>'render'",
            [crate::now_rfc3339()],
        )?;
        Ok(n + m)
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn job_lifecycle_and_recovery() {
        let db = Db::open_in_memory().unwrap();
        let j = create(&db, "render", "Test", serde_json::json!({"a":1}), 0).await.unwrap();
        assert_eq!(j.status, "queued");
        mark_running(&db, &j.id, Some("libx264".into())).await.unwrap();
        update_progress(&db, &j.id, 0.5, Some(10), Some("2.0x".into())).await.unwrap();
        append_log(&db, &j.id, "frame=10 token ghp_shouldhide").await.unwrap();
        let d = detail(&db, &j.id).await.unwrap().unwrap();
        assert_eq!(d.job.progress, 0.5);
        assert!(!d.log.join("\n").contains("ghp_should"));
        assert_eq!(recover_interrupted(&db).await.unwrap(), 1);
        assert_eq!(get(&db, &j.id).await.unwrap().unwrap().status, "queued");
        mark_running(&db, &j.id, None).await.unwrap();
        finish(&db, &j.id, "failed", Some("x".into()), None).await.unwrap();
        assert!(requeue(&db, &j.id).await.unwrap());
        let j2 = get(&db, &j.id).await.unwrap().unwrap();
        assert_eq!(j2.status, "queued");
        assert_eq!(j2.attempts, 2);
    }

    #[tokio::test]
    async fn log_is_bounded() {
        let db = Db::open_in_memory().unwrap();
        let j = create(&db, "render", "T", serde_json::json!({}), 0).await.unwrap();
        let line = "x".repeat(1000);
        for _ in 0..100 {
            append_log(&db, &j.id, &line).await.unwrap();
        }
        let d = detail(&db, &j.id).await.unwrap().unwrap();
        assert!(d.log.join("\n").len() <= MAX_LOG_BYTES + 100);
    }
}
