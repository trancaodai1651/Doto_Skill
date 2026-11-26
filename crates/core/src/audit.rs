use serde::Serialize;

use crate::Db;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditEvent {
    pub id: i64,
    pub ts: String,
    pub actor: String,
    pub action: String,
    pub target: Option<String>,
    pub detail: Option<serde_json::Value>,
    pub ip: Option<String>,
}

/// Ghi một sự kiện kiểm toán. `detail` phải đã được lọc bí mật trước khi truyền vào.
pub async fn record(
    db: &Db,
    actor: impl Into<String>,
    action: impl Into<String>,
    target: Option<String>,
    detail: Option<serde_json::Value>,
    ip: Option<String>,
) {
    let (actor, action) = (actor.into(), action.into());
    let detail_s = detail.map(|d| crate::redact_secrets(&d.to_string()));
    let res = db
        .call(move |c| {
            c.execute(
                "INSERT INTO audit_events(ts,actor,action,target,detail,ip) VALUES(?1,?2,?3,?4,?5,?6)",
                rusqlite::params![crate::now_rfc3339(), actor, action, target, detail_s, ip],
            )?;
            Ok(())
        })
        .await;
    if let Err(e) = res {
        tracing::error!(error = %e, "không ghi được audit event");
    }
}

pub async fn list(db: &Db, limit: u32) -> anyhow::Result<Vec<AuditEvent>> {
    let limit = limit.clamp(1, 1000);
    db.call(move |c| {
        let mut stmt = c.prepare("SELECT id,ts,actor,action,target,detail,ip FROM audit_events ORDER BY id DESC LIMIT ?1")?;
        let rows = stmt
            .query_map([limit], |r| {
                let detail: Option<String> = r.get(5)?;
                Ok(AuditEvent {
                    id: r.get(0)?,
                    ts: r.get(1)?,
                    actor: r.get(2)?,
                    action: r.get(3)?,
                    target: r.get(4)?,
                    detail: detail.and_then(|d| serde_json::from_str(&d).ok()),
                    ip: r.get(6)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn audit_records_and_redacts() {
        let db = Db::open_in_memory().unwrap();
        record(&db, "owner", "tunnel.token.set", None, Some(serde_json::json!({"t": "ghp_secretvalue"})), None).await;
        let ev = list(&db, 10).await.unwrap();
        assert_eq!(ev.len(), 1);
        assert!(!ev[0].detail.as_ref().map(|d| d.to_string()).unwrap_or_default().contains("ghp_secret"));
    }
}
