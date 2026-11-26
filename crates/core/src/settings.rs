use rusqlite::OptionalExtension;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::Db;

pub const KEY_APP: &str = "app";
pub const KEY_ONBOARDING: &str = "onboarding";
pub const KEY_RESOURCES: &str = "resources";
pub const KEY_SERVER: &str = "server";
pub const KEY_TUNNEL: &str = "tunnel";
pub const KEY_ENCODERS: &str = "encoders";

/// Cấu hình chung của ứng dụng (bảng `app_settings`, key `app`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettings {
    pub language: String,
    pub theme: String,
    /// Thư mục nguồn video người dùng đã cấp quyền (chỉ đọc).
    pub video_source_dirs: Vec<String>,
    /// Thư mục xuất video; None = `<data>/exports`.
    pub output_dir: Option<String>,
    pub ffmpeg_path: Option<String>,
    pub ffprobe_path: Option<String>,
    pub launch_at_login: bool,
    pub run_in_background: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            language: "vi".into(),
            theme: "system".into(),
            video_source_dirs: vec![],
            output_dir: None,
            ffmpeg_path: None,
            ffprobe_path: None,
            launch_at_login: false,
            run_in_background: false,
        }
    }
}

impl AppSettings {
    pub fn validate(&self) -> Result<(), String> {
        if !matches!(self.language.as_str(), "vi" | "en") {
            return Err("ngôn ngữ phải là vi hoặc en".into());
        }
        if !matches!(self.theme.as_str(), "system" | "light" | "dark") {
            return Err("giao diện phải là system, light hoặc dark".into());
        }
        for d in &self.video_source_dirs {
            let p = std::path::Path::new(d);
            if !p.is_absolute() {
                return Err(format!("thư mục nguồn phải là đường dẫn tuyệt đối: {d}"));
            }
        }
        if let Some(o) = &self.output_dir {
            if !std::path::Path::new(o).is_absolute() {
                return Err("thư mục output phải là đường dẫn tuyệt đối".into());
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct OnboardingState {
    pub completed: bool,
    pub completed_at: Option<String>,
    pub accepted_terms_at: Option<String>,
}

/// Đọc giá trị JSON theo key; trả Default nếu chưa có hoặc dữ liệu hỏng (ghi log cảnh báo).
pub async fn get<T: DeserializeOwned + Default + Send + 'static>(db: &Db, key: &'static str) -> anyhow::Result<T> {
    db.call(move |c| get_sync(c, key)).await
}

pub fn get_sync<T: DeserializeOwned + Default>(c: &rusqlite::Connection, key: &str) -> anyhow::Result<T> {
    let raw: Option<String> = c.query_row("SELECT value FROM app_settings WHERE key=?1", [key], |r| r.get(0)).optional()?;
    Ok(match raw {
        Some(s) => serde_json::from_str(&s).unwrap_or_else(|e| {
            tracing::warn!(key, error = %e, "setting hỏng, dùng mặc định");
            T::default()
        }),
        None => T::default(),
    })
}

pub async fn set<T: Serialize>(db: &Db, key: &'static str, value: &T) -> anyhow::Result<()> {
    let json = serde_json::to_string(value)?;
    db.call(move |c| {
        c.execute(
            "INSERT INTO app_settings(key,value,updated_at) VALUES(?1,?2,?3)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value, updated_at=excluded.updated_at",
            rusqlite::params![key, json, crate::now_rfc3339()],
        )?;
        Ok(())
    })
    .await
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn settings_roundtrip_and_defaults() {
        let db = Db::open_in_memory().unwrap();
        let s: AppSettings = get(&db, KEY_APP).await.unwrap();
        assert_eq!(s, AppSettings::default());
        let mut s2 = s.clone();
        s2.language = "en".into();
        s2.video_source_dirs = vec![std::env::temp_dir().to_string_lossy().into_owned()];
        set(&db, KEY_APP, &s2).await.unwrap();
        let s3: AppSettings = get(&db, KEY_APP).await.unwrap();
        assert_eq!(s3, s2);
    }

    #[tokio::test]
    async fn corrupt_value_falls_back_to_default() {
        let db = Db::open_in_memory().unwrap();
        db.call(|c| {
            c.execute("INSERT INTO app_settings(key,value,updated_at) VALUES('app','{not json','t')", [])?;
            Ok(())
        })
        .await
        .unwrap();
        let s: AppSettings = get(&db, KEY_APP).await.unwrap();
        assert_eq!(s.language, "vi");
    }

    #[test]
    fn validation_rejects_relative_paths_and_bad_enums() {
        let mut s = AppSettings::default();
        s.video_source_dirs = vec!["relative/dir".into()];
        assert!(s.validate().is_err());
        let mut s = AppSettings::default();
        s.theme = "neon".into();
        assert!(s.validate().is_err());
        assert!(AppSettings::default().validate().is_ok());
    }
}
