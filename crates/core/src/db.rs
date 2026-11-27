use std::path::Path;
use std::sync::{Arc, Mutex};

use rusqlite::Connection;

/// Danh sách migration theo thứ tự. Phiên bản = chỉ số + 1, lưu ở `PRAGMA user_version`.
/// Không bao giờ sửa migration đã phát hành — chỉ thêm migration mới.
const MIGRATIONS: &[&str] = &[include_str!("../migrations/0001_init.sql"), include_str!("../migrations/0002_v02_studio.sql")];

pub const SCHEMA_VERSION: i64 = MIGRATIONS.len() as i64;

/// Kết nối SQLite dùng chung (WAL). Mọi truy vấn chạy trong `spawn_blocking` qua [`Db::call`].
#[derive(Clone)]
pub struct Db {
    conn: Arc<Mutex<Connection>>,
}

impl Db {
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        let conn = Connection::open(path)?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> anyhow::Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(mut conn: Connection) -> anyhow::Result<Self> {
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        migrate(&mut conn)?;
        Ok(Self { conn: Arc::new(Mutex::new(conn)) })
    }

    /// Chạy closure với kết nối trên thread blocking.
    pub async fn call<T, F>(&self, f: F) -> anyhow::Result<T>
    where
        F: FnOnce(&mut Connection) -> anyhow::Result<T> + Send + 'static,
        T: Send + 'static,
    {
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let mut guard = conn.lock().map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
            f(&mut guard)
        })
        .await?
    }

    /// Phiên bản đồng bộ — chỉ dùng trong test/khởi động.
    pub fn call_sync<T>(&self, f: impl FnOnce(&mut Connection) -> anyhow::Result<T>) -> anyhow::Result<T> {
        let mut guard = self.conn.lock().map_err(|_| anyhow::anyhow!("db mutex poisoned"))?;
        f(&mut guard)
    }

    /// Sao lưu trực tuyến (online backup API) ra file đích.
    pub async fn backup_to(&self, dest: std::path::PathBuf) -> anyhow::Result<()> {
        self.call(move |conn| {
            let mut dst = Connection::open(&dest)?;
            let backup = rusqlite::backup::Backup::new(conn, &mut dst)?;
            backup.run_to_completion(256, std::time::Duration::from_millis(5), None)?;
            Ok(())
        })
        .await
    }

    /// Khôi phục từ file backup (ghi đè DB hiện tại trong một thao tác).
    pub async fn restore_from(&self, src: std::path::PathBuf) -> anyhow::Result<()> {
        self.call(move |conn| {
            let src_conn = Connection::open(&src)?;
            let v: i64 = src_conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
            anyhow::ensure!(v <= SCHEMA_VERSION, "bản sao lưu có schema mới hơn ứng dụng ({v} > {SCHEMA_VERSION})");
            let backup = rusqlite::backup::Backup::new(&src_conn, conn)?;
            backup.run_to_completion(256, std::time::Duration::from_millis(5), None)?;
            drop(backup);
            migrate(conn)?;
            Ok(())
        })
        .await
    }
}

fn migrate(conn: &mut Connection) -> anyhow::Result<()> {
    let current: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    anyhow::ensure!(current <= SCHEMA_VERSION, "cơ sở dữ liệu có schema {current} mới hơn ứng dụng ({SCHEMA_VERSION}); hãy cập nhật ứng dụng");
    for (idx, sql) in MIGRATIONS.iter().enumerate().skip(current as usize) {
        let version = idx as i64 + 1;
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", version)?;
        tx.commit()?;
        tracing::info!(version, "đã áp dụng migration");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_apply_and_are_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.sqlite");
        {
            let db = Db::open(&path).unwrap();
            let v: i64 = db.call_sync(|c| Ok(c.pragma_query_value(None, "user_version", |r| r.get(0))?)).unwrap();
            assert_eq!(v, SCHEMA_VERSION);
        }
        // mở lại không chạy lại migration
        let db = Db::open(&path).unwrap();
        let n: i64 = db.call_sync(|c| Ok(c.query_row("SELECT count(*) FROM sqlite_master WHERE type='table' AND name='skills'", [], |r| r.get(0))?)).unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn foreign_keys_cascade() {
        let db = Db::open_in_memory().unwrap();
        db.call_sync(|c| {
            c.execute("INSERT INTO skill_sources(id,kind,location,display_name,created_at) VALUES('s','folder','/x','x','t')", [])?;
            c.execute(
                "INSERT INTO skills(id,source_id,canonical_id,name,rel_path,revision,created_at,updated_at) VALUES('k','s','a','a','a','r','t','t')",
                [],
            )?;
            c.execute("DELETE FROM skill_sources WHERE id='s'", [])?;
            let n: i64 = c.query_row("SELECT count(*) FROM skills", [], |r| r.get(0))?;
            assert_eq!(n, 0);
            Ok(())
        })
        .unwrap();
    }

    #[tokio::test]
    async fn backup_and_restore_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("a.sqlite")).unwrap();
        db.call(|c| {
            c.execute("INSERT INTO app_settings(key,value,updated_at) VALUES('k','\"v1\"','t')", [])?;
            Ok(())
        })
        .await
        .unwrap();
        let bak = dir.path().join("b.sqlite");
        db.backup_to(bak.clone()).await.unwrap();
        db.call(|c| {
            c.execute("UPDATE app_settings SET value='\"v2\"' WHERE key='k'", [])?;
            Ok(())
        })
        .await
        .unwrap();
        db.restore_from(bak).await.unwrap();
        let v: String = db.call(|c| Ok(c.query_row("SELECT value FROM app_settings WHERE key='k'", [], |r| r.get(0))?)).await.unwrap();
        assert_eq!(v, "\"v1\"");
    }
}
