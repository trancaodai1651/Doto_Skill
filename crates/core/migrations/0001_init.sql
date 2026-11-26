-- Doto Skill schema v1
CREATE TABLE app_settings (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE host_hardware (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  snapshot TEXT NOT NULL,
  probed_at TEXT NOT NULL
);

CREATE TABLE cli_tools (
  id TEXT PRIMARY KEY,
  path TEXT,
  version TEXT,
  status TEXT NOT NULL DEFAULT '{}',
  last_checked_at TEXT
);

CREATE TABLE skill_sources (
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL CHECK (kind IN ('folder','github')),
  location TEXT NOT NULL,
  display_name TEXT NOT NULL,
  git_ref TEXT,
  sub_path TEXT,
  current_sha TEXT,
  local_path TEXT,
  pinned INTEGER NOT NULL DEFAULT 0,
  enabled INTEGER NOT NULL DEFAULT 1,
  watch INTEGER NOT NULL DEFAULT 0,
  private INTEGER NOT NULL DEFAULT 0,
  last_scan_at TEXT,
  last_error TEXT,
  created_at TEXT NOT NULL,
  UNIQUE(kind, location, sub_path, git_ref)
);

CREATE TABLE skills (
  id TEXT PRIMARY KEY,
  source_id TEXT NOT NULL REFERENCES skill_sources(id) ON DELETE CASCADE,
  canonical_id TEXT NOT NULL,
  name TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  rel_path TEXT NOT NULL,
  revision TEXT NOT NULL,
  metadata TEXT NOT NULL DEFAULT '{}',
  has_scripts INTEGER NOT NULL DEFAULT 0,
  warnings TEXT NOT NULL DEFAULT '[]',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE(source_id, rel_path)
);
CREATE INDEX idx_skills_canonical ON skills(canonical_id);

CREATE TABLE skill_installs (
  skill_id TEXT NOT NULL REFERENCES skills(id) ON DELETE CASCADE,
  provider TEXT NOT NULL,
  target_path TEXT NOT NULL,
  revision TEXT NOT NULL,
  installed_at TEXT NOT NULL,
  PRIMARY KEY (skill_id, provider)
);

CREATE TABLE github_connections (
  id TEXT PRIMARY KEY,
  login TEXT,
  scopes TEXT NOT NULL DEFAULT '[]',
  method TEXT NOT NULL,
  vault_key TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE TABLE users (
  id TEXT PRIMARY KEY,
  username TEXT NOT NULL UNIQUE COLLATE NOCASE,
  password_hash TEXT NOT NULL,
  role TEXT NOT NULL CHECK (role IN ('owner','viewer')),
  disabled INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL
);

CREATE TABLE sessions (
  id TEXT PRIMARY KEY,
  token_hash TEXT NOT NULL UNIQUE,
  csrf_token TEXT NOT NULL,
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  device_name TEXT NOT NULL,
  user_agent TEXT,
  ip TEXT,
  host_control INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  last_seen_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  revoked INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE pairing_codes (
  code_hash TEXT PRIMARY KEY,
  role TEXT NOT NULL,
  created_by TEXT NOT NULL,
  created_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  used_at TEXT,
  used_by_user TEXT
);

CREATE TABLE video_assets (
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL CHECK (kind IN ('source','export')),
  title TEXT NOT NULL,
  file_path TEXT NOT NULL UNIQUE,
  file_name TEXT NOT NULL,
  size_bytes INTEGER NOT NULL,
  mtime TEXT,
  duration_ms INTEGER,
  width INTEGER,
  height INTEGER,
  video_codec TEXT,
  audio_codec TEXT,
  container TEXT,
  thumbnail_path TEXT,
  visibility TEXT NOT NULL DEFAULT 'private' CHECK (visibility IN ('private','viewers')),
  status TEXT NOT NULL DEFAULT 'ready',
  job_id TEXT,
  created_at TEXT NOT NULL
);

CREATE TABLE jobs (
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL,
  status TEXT NOT NULL,
  title TEXT NOT NULL,
  priority INTEGER NOT NULL DEFAULT 0,
  params TEXT NOT NULL DEFAULT '{}',
  progress REAL NOT NULL DEFAULT 0,
  eta_seconds INTEGER,
  speed TEXT,
  encoder TEXT,
  error TEXT,
  log TEXT NOT NULL DEFAULT '',
  attempts INTEGER NOT NULL DEFAULT 0,
  output_video_id TEXT,
  created_at TEXT NOT NULL,
  started_at TEXT,
  finished_at TEXT
);
CREATE INDEX idx_jobs_status ON jobs(status);

CREATE TABLE exports (
  id TEXT PRIMARY KEY,
  job_id TEXT NOT NULL,
  video_id TEXT NOT NULL REFERENCES video_assets(id) ON DELETE CASCADE,
  preset TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL
);

CREATE TABLE share_links (
  id TEXT PRIMARY KEY,
  token_hash TEXT NOT NULL UNIQUE,
  video_id TEXT NOT NULL REFERENCES video_assets(id) ON DELETE CASCADE,
  created_by TEXT NOT NULL,
  created_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  revoked INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE audit_events (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  ts TEXT NOT NULL,
  actor TEXT NOT NULL,
  action TEXT NOT NULL,
  target TEXT,
  detail TEXT,
  ip TEXT
);

CREATE TABLE handoffs (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  agent TEXT NOT NULL,
  repo_sha TEXT,
  summary TEXT NOT NULL,
  created_at TEXT NOT NULL
);
