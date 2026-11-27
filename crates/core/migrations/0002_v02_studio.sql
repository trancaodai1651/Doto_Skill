-- Doto Skill schema v2 (v0.2.0: Mobile uploads, Multi-tunnel CRUD, AI Studio multi-session)

CREATE TABLE uploaded_media (
  id TEXT PRIMARY KEY,
  original_name TEXT NOT NULL,
  safe_name TEXT NOT NULL,
  file_path TEXT NOT NULL UNIQUE,
  mime_type TEXT NOT NULL,
  media_kind TEXT NOT NULL CHECK (media_kind IN ('video','image')),
  size_bytes INTEGER NOT NULL,
  video_id TEXT REFERENCES video_assets(id) ON DELETE SET NULL,
  created_at TEXT NOT NULL
);
CREATE INDEX idx_uploaded_media_created ON uploaded_media(created_at DESC);

CREATE TABLE managed_tunnels (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  kind TEXT NOT NULL CHECK (kind IN ('quick','token','named')),
  target_url TEXT NOT NULL,
  hostname TEXT,
  cf_tunnel_name TEXT,
  config_path TEXT,
  vault_key TEXT,
  auto_start INTEGER NOT NULL DEFAULT 0,
  notes TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE studio_sessions (
  id TEXT PRIMARY KEY,
  job_id TEXT NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
  session_index INTEGER NOT NULL,
  label TEXT NOT NULL,
  provider TEXT NOT NULL CHECK (provider IN ('antigravity','claude','codex')),
  model TEXT NOT NULL DEFAULT '',
  effort TEXT NOT NULL DEFAULT '',
  skill_id TEXT,
  status TEXT NOT NULL,
  command_preview TEXT NOT NULL,
  output_subdir TEXT NOT NULL,
  result_text TEXT NOT NULL DEFAULT '',
  log TEXT NOT NULL DEFAULT '',
  error TEXT,
  exit_code INTEGER,
  output_video_ids TEXT NOT NULL DEFAULT '[]',
  started_at TEXT,
  finished_at TEXT
);
CREATE INDEX idx_studio_sessions_job ON studio_sessions(job_id, session_index);
