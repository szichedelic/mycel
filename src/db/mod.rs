use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::path::{Path, PathBuf};

pub struct Database {
    conn: Connection,
}

#[derive(Debug, Clone)]
pub struct Project {
    pub id: i64,
    pub name: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct Session {
    pub id: i64,
    pub name: String,
    pub branch_name: String,
    pub worktree_path: PathBuf,
    pub tmux_session: String,
    #[allow(dead_code)]
    pub runtime_kind: String,
    pub backend: String,
    pub note: Option<String>,
    pub created_at_unix: i64,
}

#[derive(Debug, Clone)]
pub struct SessionHistory {
    pub name: String,
    pub note: Option<String>,
    pub created_at_unix: i64,
    pub ended_at_unix: i64,
    pub commit_count: Option<i64>,
}

pub struct NewSession<'a> {
    pub project_id: i64,
    pub name: &'a str,
    pub branch_name: &'a str,
    pub worktree_path: &'a Path,
    pub tmux_session: &'a str,
    pub runtime_kind: &'a str,
    pub backend: &'a str,
    pub note: Option<&'a str>,
}

/// Provider-neutral runtime metadata for a session.
#[derive(Debug, Clone)]
pub struct SessionRuntime {
    pub id: i64,
    #[allow(dead_code)]
    pub session_id: i64,
    pub provider: String,
    pub host: String,
    pub runtime_ref: String,
    pub compose_project: Option<String>,
    pub state: String,
    pub last_seen_unix: i64,
}

/// A service belonging to a session runtime (e.g. the primary AI backend container).
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SessionService {
    pub id: i64,
    pub runtime_id: i64,
    pub service_name: String,
    pub is_primary: bool,
    pub health: String,
    pub ports: Option<String>,
    pub status: String,
}

/// A session with its optional runtime summary attached.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SessionWithRuntime {
    pub session: Session,
    pub runtime: Option<SessionRuntime>,
    pub services: Vec<SessionService>,
}

pub struct NewSessionRuntime<'a> {
    pub session_id: i64,
    pub provider: &'a str,
    pub host: &'a str,
    pub runtime_ref: &'a str,
    pub compose_project: Option<&'a str>,
    pub state: &'a str,
}

/// A registered remote host that can run sessions.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Host {
    pub id: i64,
    pub name: String,
    pub docker_host: String,
    pub max_sessions: i64,
    pub enabled: bool,
    pub current_project_id: Option<i64>,
}

impl Database {
    /// Create an in-memory database for testing.
    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Self { conn };
        db.init_schema()?;
        Ok(db)
    }

    pub fn open() -> Result<Self> {
        let db_path = dirs::data_dir()
            .context("Could not find data directory")?
            .join("mycel")
            .join("mycel.db");

        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(&db_path)?;

        let db = Self { conn };
        db.init_schema()?;

        Ok(db)
    }

    fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS projects (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                path TEXT NOT NULL UNIQUE
            );

            CREATE TABLE IF NOT EXISTS sessions (
                id INTEGER PRIMARY KEY,
                project_id INTEGER NOT NULL,
                name TEXT NOT NULL,
                worktree_path TEXT NOT NULL,
                tmux_session TEXT NOT NULL,
                backend TEXT NOT NULL DEFAULT 'claude',
                note TEXT,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (project_id) REFERENCES projects(id),
                UNIQUE(project_id, name)
            );

            CREATE TABLE IF NOT EXISTS session_history (
                id INTEGER PRIMARY KEY,
                project_id INTEGER NOT NULL,
                name TEXT NOT NULL,
                worktree_path TEXT NOT NULL,
                note TEXT,
                created_at TEXT NOT NULL,
                ended_at TEXT NOT NULL,
                commit_count INTEGER,
                FOREIGN KEY (project_id) REFERENCES projects(id)
            );
            ",
        )?;

        self.ensure_sessions_created_at()?;
        self.ensure_sessions_note()?;
        self.ensure_sessions_backend()?;
        self.ensure_sessions_branch_name()?;
        self.ensure_sessions_runtime_kind()?;
        self.ensure_session_runtimes_table()?;
        self.ensure_session_services_table()?;
        self.ensure_hosts_table()?;
        self.ensure_hosts_current_project()?;
        self.backfill_tmux_runtimes()?;
        Ok(())
    }

    fn ensure_sessions_created_at(&self) -> Result<()> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('sessions') WHERE name = 'created_at'",
            [],
            |row| row.get(0),
        )?;

        if count == 0 {
            self.conn.execute(
                "ALTER TABLE sessions ADD COLUMN created_at TEXT DEFAULT CURRENT_TIMESTAMP",
                [],
            )?;
            self.conn.execute(
                "UPDATE sessions SET created_at = CURRENT_TIMESTAMP WHERE created_at IS NULL",
                [],
            )?;
        }

        Ok(())
    }

    fn ensure_sessions_note(&self) -> Result<()> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('sessions') WHERE name = 'note'",
            [],
            |row| row.get(0),
        )?;

        if count == 0 {
            self.conn
                .execute("ALTER TABLE sessions ADD COLUMN note TEXT", [])?;
        }

        Ok(())
    }

    fn ensure_sessions_backend(&self) -> Result<()> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('sessions') WHERE name = 'backend'",
            [],
            |row| row.get(0),
        )?;

        if count == 0 {
            self.conn.execute(
                "ALTER TABLE sessions ADD COLUMN backend TEXT NOT NULL DEFAULT 'claude'",
                [],
            )?;
            self.conn.execute(
                "UPDATE sessions SET backend = 'claude' WHERE backend IS NULL OR backend = ''",
                [],
            )?;
        }

        Ok(())
    }

    fn ensure_sessions_branch_name(&self) -> Result<()> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('sessions') WHERE name = 'branch_name'",
            [],
            |row| row.get(0),
        )?;

        if count == 0 {
            self.conn
                .execute("ALTER TABLE sessions ADD COLUMN branch_name TEXT", [])?;
            self.conn.execute(
                "UPDATE sessions SET branch_name = name WHERE branch_name IS NULL",
                [],
            )?;
        }

        Ok(())
    }

    fn ensure_sessions_runtime_kind(&self) -> Result<()> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('sessions') WHERE name = 'runtime_kind'",
            [],
            |row| row.get(0),
        )?;

        if count == 0 {
            self.conn.execute(
                "ALTER TABLE sessions ADD COLUMN runtime_kind TEXT NOT NULL DEFAULT 'tmux'",
                [],
            )?;
            self.conn.execute(
                "UPDATE sessions SET runtime_kind = 'tmux' WHERE runtime_kind IS NULL OR runtime_kind = ''",
                [],
            )?;
        }

        Ok(())
    }

    pub fn add_project(&self, name: &str, path: &Path) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO projects (name, path) VALUES (?1, ?2)",
            params![name, path.to_string_lossy()],
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_project_by_path(&self, path: &Path) -> Result<Option<Project>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, path FROM projects WHERE path = ?1")?;

        let result = stmt.query_row(params![path.to_string_lossy()], |row| {
            Ok(Project {
                id: row.get(0)?,
                name: row.get(1)?,
                path: PathBuf::from(row.get::<_, String>(2)?),
            })
        });

        match result {
            Ok(project) => Ok(Some(project)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn list_projects(&self) -> Result<Vec<Project>> {
        let mut stmt = self.conn.prepare("SELECT id, name, path FROM projects")?;

        let projects = stmt
            .query_map([], |row| {
                Ok(Project {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    path: PathBuf::from(row.get::<_, String>(2)?),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(projects)
    }

    pub fn add_session(&self, session: &NewSession) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO sessions (project_id, name, branch_name, worktree_path, tmux_session, runtime_kind, backend, note) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                session.project_id,
                session.name,
                session.branch_name,
                session.worktree_path.to_string_lossy(),
                session.tmux_session,
                session.runtime_kind,
                session.backend,
                session.note
            ],
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_session_by_name(&self, project_id: i64, name: &str) -> Result<Option<Session>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, COALESCE(branch_name, name), worktree_path, tmux_session, COALESCE(runtime_kind, 'tmux'), backend, note, CAST(strftime('%s', created_at) AS INTEGER)
             FROM sessions WHERE project_id = ?1 AND name = ?2",
        )?;

        let result = stmt.query_row(params![project_id, name], |row| {
            Ok(Session {
                id: row.get(0)?,
                name: row.get(1)?,
                branch_name: row.get(2)?,
                worktree_path: PathBuf::from(row.get::<_, String>(3)?),
                tmux_session: row.get(4)?,
                runtime_kind: row.get(5)?,
                backend: row.get(6)?,
                note: row.get(7)?,
                created_at_unix: row.get(8)?,
            })
        });

        match result {
            Ok(session) => Ok(Some(session)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn list_sessions(&self, project_id: i64) -> Result<Vec<Session>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, COALESCE(branch_name, name), worktree_path, tmux_session, COALESCE(runtime_kind, 'tmux'), backend, note, CAST(strftime('%s', created_at) AS INTEGER)
             FROM sessions WHERE project_id = ?1",
        )?;

        let sessions = stmt
            .query_map(params![project_id], |row| {
                Ok(Session {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    branch_name: row.get(2)?,
                    worktree_path: PathBuf::from(row.get::<_, String>(3)?),
                    tmux_session: row.get(4)?,
                    runtime_kind: row.get(5)?,
                    backend: row.get(6)?,
                    note: row.get(7)?,
                    created_at_unix: row.get(8)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(sessions)
    }

    pub fn archive_session(
        &self,
        project_id: i64,
        session: &Session,
        commit_count: Option<i64>,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO session_history (project_id, name, worktree_path, note, created_at, ended_at, commit_count)
             VALUES (?1, ?2, ?3, ?4, datetime(?5, 'unixepoch'), CURRENT_TIMESTAMP, ?6)",
            params![
                project_id,
                session.name,
                session.worktree_path.to_string_lossy(),
                session.note.as_deref(),
                session.created_at_unix,
                commit_count
            ],
        )?;

        Ok(())
    }

    pub fn list_session_history(&self, project_id: i64) -> Result<Vec<SessionHistory>> {
        let mut stmt = self.conn.prepare(
            "SELECT name, note,
                    CAST(strftime('%s', created_at) AS INTEGER),
                    CAST(strftime('%s', ended_at) AS INTEGER),
                    commit_count
             FROM session_history
             WHERE project_id = ?1
             ORDER BY ended_at DESC",
        )?;

        let sessions = stmt
            .query_map(params![project_id], |row| {
                Ok(SessionHistory {
                    name: row.get(0)?,
                    note: row.get(1)?,
                    created_at_unix: row.get(2)?,
                    ended_at_unix: row.get(3)?,
                    commit_count: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(sessions)
    }

    pub fn count_sessions_for_project(&self, project_id: i64) -> Result<usize> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM sessions WHERE project_id = ?1",
            params![project_id],
            |row| row.get(0),
        )?;

        Ok(count as usize)
    }

    pub fn delete_session(&self, session_id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM sessions WHERE id = ?1", params![session_id])?;

        Ok(())
    }

    pub fn update_session_note(&self, session_id: i64, note: Option<&str>) -> Result<()> {
        self.conn.execute(
            "UPDATE sessions SET note = ?1 WHERE id = ?2",
            params![note, session_id],
        )?;

        Ok(())
    }

    pub fn update_session_name(&self, session_id: i64, name: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE sessions SET name = ?1 WHERE id = ?2",
            params![name, session_id],
        )?;

        Ok(())
    }

    pub fn update_session_tmux(&self, session_id: i64, tmux_session: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE sessions SET tmux_session = ?1 WHERE id = ?2",
            params![tmux_session, session_id],
        )?;

        Ok(())
    }

    /// Update the runtime kind and runtime_id for a session during handoff.
    pub fn update_session_runtime_kind(
        &self,
        session_id: i64,
        runtime_kind: &str,
        runtime_id: &str,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE sessions SET runtime_kind = ?1, tmux_session = ?2 WHERE id = ?3",
            params![runtime_kind, runtime_id, session_id],
        )?;
        Ok(())
    }

    /// Replace the session_runtimes row for a handoff.
    /// Deletes the old runtime (cascading to services) and inserts a new one.
    pub fn replace_session_runtime(&self, rt: &NewSessionRuntime) -> Result<i64> {
        self.conn.execute(
            "DELETE FROM session_runtimes WHERE session_id = ?1",
            params![rt.session_id],
        )?;
        self.add_session_runtime(rt)
    }

    // -- session_runtimes / session_services migrations --

    fn ensure_session_runtimes_table(&self) -> Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS session_runtimes (
                id INTEGER PRIMARY KEY,
                session_id INTEGER NOT NULL UNIQUE,
                provider TEXT NOT NULL,
                host TEXT NOT NULL DEFAULT 'local',
                runtime_ref TEXT NOT NULL,
                compose_project TEXT,
                state TEXT NOT NULL DEFAULT 'running',
                last_seen TEXT DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
            );",
        )?;
        Ok(())
    }

    fn ensure_session_services_table(&self) -> Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS session_services (
                id INTEGER PRIMARY KEY,
                runtime_id INTEGER NOT NULL,
                service_name TEXT NOT NULL,
                is_primary INTEGER NOT NULL DEFAULT 0,
                health TEXT NOT NULL DEFAULT 'unknown',
                ports TEXT,
                status TEXT NOT NULL DEFAULT 'unknown',
                FOREIGN KEY (runtime_id) REFERENCES session_runtimes(id) ON DELETE CASCADE
            );",
        )?;
        Ok(())
    }

    fn backfill_tmux_runtimes(&self) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO session_runtimes (session_id, provider, host, runtime_ref, state)
             SELECT id, 'tmux', 'local', tmux_session, 'running'
             FROM sessions
             WHERE id NOT IN (SELECT session_id FROM session_runtimes)",
            [],
        )?;
        Ok(())
    }

    // -- session_runtimes CRUD --

    pub fn add_session_runtime(&self, rt: &NewSessionRuntime) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO session_runtimes (session_id, provider, host, runtime_ref, compose_project, state)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                rt.session_id,
                rt.provider,
                rt.host,
                rt.runtime_ref,
                rt.compose_project,
                rt.state,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_session_runtime(&self, session_id: i64) -> Result<Option<SessionRuntime>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, session_id, provider, host, runtime_ref, compose_project, state,
                    CAST(strftime('%s', last_seen) AS INTEGER)
             FROM session_runtimes WHERE session_id = ?1",
        )?;

        let result = stmt.query_row(params![session_id], |row| {
            Ok(SessionRuntime {
                id: row.get(0)?,
                session_id: row.get(1)?,
                provider: row.get(2)?,
                host: row.get(3)?,
                runtime_ref: row.get(4)?,
                compose_project: row.get(5)?,
                state: row.get(6)?,
                last_seen_unix: row.get(7)?,
            })
        });

        match result {
            Ok(rt) => Ok(Some(rt)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    #[allow(dead_code)]
    pub fn update_runtime_state(&self, runtime_id: i64, state: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE session_runtimes SET state = ?1, last_seen = CURRENT_TIMESTAMP WHERE id = ?2",
            params![state, runtime_id],
        )?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn touch_runtime(&self, runtime_id: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE session_runtimes SET last_seen = CURRENT_TIMESTAMP WHERE id = ?1",
            params![runtime_id],
        )?;
        Ok(())
    }

    // -- session_services CRUD --

    #[allow(dead_code)]
    pub fn add_session_service(
        &self,
        runtime_id: i64,
        service_name: &str,
        is_primary: bool,
    ) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO session_services (runtime_id, service_name, is_primary)
             VALUES (?1, ?2, ?3)",
            params![runtime_id, service_name, is_primary as i32],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    #[allow(dead_code)]
    pub fn list_services_for_runtime(&self, runtime_id: i64) -> Result<Vec<SessionService>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, runtime_id, service_name, is_primary, health, ports, status
             FROM session_services WHERE runtime_id = ?1",
        )?;

        let services = stmt
            .query_map(params![runtime_id], |row| {
                Ok(SessionService {
                    id: row.get(0)?,
                    runtime_id: row.get(1)?,
                    service_name: row.get(2)?,
                    is_primary: row.get::<_, i32>(3)? != 0,
                    health: row.get(4)?,
                    ports: row.get(5)?,
                    status: row.get(6)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(services)
    }

    #[allow(dead_code)]
    pub fn update_service_health(&self, service_id: i64, health: &str, status: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE session_services SET health = ?1, status = ?2 WHERE id = ?3",
            params![health, status, service_id],
        )?;
        Ok(())
    }

    // -- Joined query: sessions with runtime summaries --

    #[allow(dead_code)]
    pub fn list_sessions_with_runtimes(&self, project_id: i64) -> Result<Vec<SessionWithRuntime>> {
        let sessions = self.list_sessions(project_id)?;
        let mut result = Vec::with_capacity(sessions.len());

        for session in sessions {
            let runtime = self.get_session_runtime(session.id)?;
            let services = match &runtime {
                Some(rt) => self.list_services_for_runtime(rt.id)?,
                None => Vec::new(),
            };
            result.push(SessionWithRuntime {
                session,
                runtime,
                services,
            });
        }

        Ok(result)
    }

    // -- hosts table --

    fn ensure_hosts_table(&self) -> Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS hosts (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                docker_host TEXT NOT NULL,
                max_sessions INTEGER NOT NULL DEFAULT 4,
                enabled INTEGER NOT NULL DEFAULT 1
            );",
        )?;
        Ok(())
    }

    fn ensure_hosts_current_project(&self) -> Result<()> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('hosts') WHERE name = 'current_project_id'",
            [],
            |row| row.get(0),
        )?;

        if count == 0 {
            self.conn.execute(
                "ALTER TABLE hosts ADD COLUMN current_project_id INTEGER REFERENCES projects(id)",
                [],
            )?;
        }

        Ok(())
    }

    pub fn add_host(&self, name: &str, docker_host: &str, max_sessions: i64) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO hosts (name, docker_host, max_sessions) VALUES (?1, ?2, ?3)",
            params![name, docker_host, max_sessions],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn remove_host(&self, name: &str) -> Result<bool> {
        let affected = self
            .conn
            .execute("DELETE FROM hosts WHERE name = ?1", params![name])?;
        Ok(affected > 0)
    }

    pub fn list_hosts(&self) -> Result<Vec<Host>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, docker_host, max_sessions, enabled, current_project_id FROM hosts ORDER BY name",
        )?;

        let hosts = stmt
            .query_map([], |row| {
                Ok(Host {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    docker_host: row.get(2)?,
                    max_sessions: row.get(3)?,
                    enabled: row.get::<_, i32>(4)? != 0,
                    current_project_id: row.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(hosts)
    }

    #[allow(dead_code)]
    pub fn get_host_by_name(&self, name: &str) -> Result<Option<Host>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, docker_host, max_sessions, enabled, current_project_id FROM hosts WHERE name = ?1",
        )?;

        let result = stmt.query_row(params![name], |row| {
            Ok(Host {
                id: row.get(0)?,
                name: row.get(1)?,
                docker_host: row.get(2)?,
                max_sessions: row.get(3)?,
                enabled: row.get::<_, i32>(4)? != 0,
                current_project_id: row.get(5)?,
            })
        });

        match result {
            Ok(host) => Ok(Some(host)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn set_host_enabled(&self, name: &str, enabled: bool) -> Result<bool> {
        let affected = self.conn.execute(
            "UPDATE hosts SET enabled = ?1 WHERE name = ?2",
            params![enabled as i32, name],
        )?;
        Ok(affected > 0)
    }

    pub fn set_host_project(&self, host_name: &str, project_id: Option<i64>) -> Result<()> {
        self.conn.execute(
            "UPDATE hosts SET current_project_id = ?1 WHERE name = ?2",
            params![project_id, host_name],
        )?;
        Ok(())
    }

    pub fn get_host_project_name(&self, host_id: i64) -> Result<Option<String>> {
        let result: rusqlite::Result<Option<String>> = self.conn.query_row(
            "SELECT p.name FROM hosts h
             LEFT JOIN projects p ON h.current_project_id = p.id
             WHERE h.id = ?1",
            params![host_id],
            |row| row.get(0),
        );

        match result {
            Ok(name) => Ok(name),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn find_sessions_on_host(
        &self,
        docker_host: &str,
    ) -> Result<Vec<(Session, SessionRuntime)>> {
        let mut stmt = self.conn.prepare(
            "SELECT s.id, s.name, COALESCE(s.branch_name, s.name), s.worktree_path, s.tmux_session,
                    COALESCE(s.runtime_kind, 'tmux'), s.backend, s.note,
                    CAST(strftime('%s', s.created_at) AS INTEGER),
                    r.id, r.session_id, r.provider, r.host, r.runtime_ref, r.compose_project,
                    r.state, CAST(strftime('%s', r.last_seen) AS INTEGER)
             FROM sessions s
             JOIN session_runtimes r ON r.session_id = s.id
             WHERE r.host = ?1 AND r.state = 'running'",
        )?;

        let rows = stmt
            .query_map(params![docker_host], |row| {
                let session = Session {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    branch_name: row.get(2)?,
                    worktree_path: PathBuf::from(row.get::<_, String>(3)?),
                    tmux_session: row.get(4)?,
                    runtime_kind: row.get(5)?,
                    backend: row.get(6)?,
                    note: row.get(7)?,
                    created_at_unix: row.get(8)?,
                };
                let runtime = SessionRuntime {
                    id: row.get(9)?,
                    session_id: row.get(10)?,
                    provider: row.get(11)?,
                    host: row.get(12)?,
                    runtime_ref: row.get(13)?,
                    compose_project: row.get(14)?,
                    state: row.get(15)?,
                    last_seen_unix: row.get(16)?,
                };
                Ok((session, runtime))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(rows)
    }

    /// Count active sessions on a given host.
    pub fn count_sessions_on_host(&self, docker_host: &str) -> Result<i64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM session_runtimes WHERE host = ?1 AND state = 'running'",
            params![docker_host],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// Find sessions whose runtimes haven't been seen since `cutoff_unix`.
    pub fn find_idle_runtimes(&self, cutoff_unix: i64) -> Result<Vec<(Session, SessionRuntime)>> {
        let mut stmt = self.conn.prepare(
            "SELECT s.id, s.name, COALESCE(s.branch_name, s.name), s.worktree_path, s.tmux_session,
                    COALESCE(s.runtime_kind, 'tmux'), s.backend, s.note,
                    CAST(strftime('%s', s.created_at) AS INTEGER),
                    r.id, r.session_id, r.provider, r.host, r.runtime_ref, r.compose_project,
                    r.state, CAST(strftime('%s', r.last_seen) AS INTEGER)
             FROM sessions s
             JOIN session_runtimes r ON r.session_id = s.id
             WHERE r.state = 'running'
               AND CAST(strftime('%s', r.last_seen) AS INTEGER) < ?1",
        )?;

        let rows = stmt
            .query_map(params![cutoff_unix], |row| {
                let session = Session {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    branch_name: row.get(2)?,
                    worktree_path: PathBuf::from(row.get::<_, String>(3)?),
                    tmux_session: row.get(4)?,
                    runtime_kind: row.get(5)?,
                    backend: row.get(6)?,
                    note: row.get(7)?,
                    created_at_unix: row.get(8)?,
                };
                let runtime = SessionRuntime {
                    id: row.get(9)?,
                    session_id: row.get(10)?,
                    provider: row.get(11)?,
                    host: row.get(12)?,
                    runtime_ref: row.get(13)?,
                    compose_project: row.get(14)?,
                    state: row.get(15)?,
                    last_seen_unix: row.get(16)?,
                };
                Ok((session, runtime))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn in_memory_db() -> Database {
        let conn = Connection::open_in_memory().unwrap();
        let db = Database { conn };
        db.init_schema().unwrap();
        db
    }

    fn seed_project(db: &Database) -> i64 {
        db.add_project("test-project", Path::new("/tmp/test"))
            .unwrap()
    }

    fn seed_session(db: &Database, project_id: i64) -> i64 {
        let sid = db
            .add_session(&NewSession {
                project_id,
                name: "feat-x",
                branch_name: "feat-x",
                worktree_path: Path::new("/tmp/wt"),
                tmux_session: "mycel_test_feat-x",
                runtime_kind: "tmux",
                backend: "claude",
                note: None,
            })
            .unwrap();
        // Simulate what backfill does for sessions created after migration
        db.backfill_tmux_runtimes().unwrap();
        sid
    }

    #[test]
    fn fresh_schema_creates_runtime_tables() {
        let db = in_memory_db();
        let count: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='session_runtimes'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);

        let count: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='session_services'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn backfill_creates_runtime_for_existing_sessions() {
        let db = in_memory_db();
        let pid = seed_project(&db);
        let sid = seed_session(&db, pid);

        let rt = db.get_session_runtime(sid).unwrap();
        assert!(rt.is_some());
        let rt = rt.unwrap();
        assert_eq!(rt.provider, "tmux");
        assert_eq!(rt.host, "local");
        assert_eq!(rt.runtime_ref, "mycel_test_feat-x");
        assert_eq!(rt.state, "running");
    }

    #[test]
    fn backfill_is_idempotent() {
        let db = in_memory_db();
        let pid = seed_project(&db);
        seed_session(&db, pid);

        // Run backfill again — should not fail or duplicate
        db.backfill_tmux_runtimes().unwrap();
        let count: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM session_runtimes", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn add_and_list_services() {
        let db = in_memory_db();
        let pid = seed_project(&db);
        let sid = seed_session(&db, pid);

        let rt = db.get_session_runtime(sid).unwrap().unwrap();
        db.add_session_service(rt.id, "claude-agent", true).unwrap();
        db.add_session_service(rt.id, "web-server", false).unwrap();

        let services = db.list_services_for_runtime(rt.id).unwrap();
        assert_eq!(services.len(), 2);
        assert!(services[0].is_primary);
        assert!(!services[1].is_primary);
    }

    #[test]
    fn list_sessions_with_runtimes_joins_correctly() {
        let db = in_memory_db();
        let pid = seed_project(&db);
        let sid = seed_session(&db, pid);

        let rt = db.get_session_runtime(sid).unwrap().unwrap();
        db.add_session_service(rt.id, "agent", true).unwrap();

        let sessions = db.list_sessions_with_runtimes(pid).unwrap();
        assert_eq!(sessions.len(), 1);
        assert!(sessions[0].runtime.is_some());
        assert_eq!(sessions[0].services.len(), 1);
    }

    #[test]
    fn delete_session_cascades_to_runtime_and_services() {
        let db = in_memory_db();
        let pid = seed_project(&db);
        let sid = seed_session(&db, pid);

        let rt = db.get_session_runtime(sid).unwrap().unwrap();
        db.add_session_service(rt.id, "agent", true).unwrap();

        db.delete_session(sid).unwrap();

        let rt_count: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM session_runtimes", [], |row| {
                row.get(0)
            })
            .unwrap();
        let svc_count: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM session_services", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(rt_count, 0);
        assert_eq!(svc_count, 0);
    }

    #[test]
    fn update_runtime_state_and_touch() {
        let db = in_memory_db();
        let pid = seed_project(&db);
        let sid = seed_session(&db, pid);

        let rt = db.get_session_runtime(sid).unwrap().unwrap();
        db.update_runtime_state(rt.id, "stopped").unwrap();

        let rt2 = db.get_session_runtime(sid).unwrap().unwrap();
        assert_eq!(rt2.state, "stopped");

        db.touch_runtime(rt.id).unwrap();
        let rt3 = db.get_session_runtime(sid).unwrap().unwrap();
        assert!(rt3.last_seen_unix >= rt2.last_seen_unix);
    }

    #[test]
    fn schema_idempotent_on_reopen() {
        let db = in_memory_db();
        // Running init_schema again should not fail
        db.init_schema().unwrap();
    }

    #[test]
    fn update_session_runtime_kind_changes_kind_and_id() {
        let db = in_memory_db();
        let pid = seed_project(&db);
        let sid = seed_session(&db, pid);

        db.update_session_runtime_kind(sid, "compose", "mycel-test-feat-x")
            .unwrap();

        let session = db.get_session_by_name(pid, "feat-x").unwrap().unwrap();
        assert_eq!(session.runtime_kind, "compose");
        assert_eq!(session.tmux_session, "mycel-test-feat-x");
    }

    #[test]
    fn replace_session_runtime_swaps_row() {
        let db = in_memory_db();
        let pid = seed_project(&db);
        let sid = seed_session(&db, pid);

        // Original is tmux/local from backfill
        let rt = db.get_session_runtime(sid).unwrap().unwrap();
        assert_eq!(rt.provider, "tmux");

        // Replace with compose
        db.replace_session_runtime(&NewSessionRuntime {
            session_id: sid,
            provider: "compose",
            host: "local",
            runtime_ref: "mycel-test-compose",
            compose_project: Some("mycel-test-compose"),
            state: "running",
        })
        .unwrap();

        let rt2 = db.get_session_runtime(sid).unwrap().unwrap();
        assert_eq!(rt2.provider, "compose");
        assert_eq!(rt2.runtime_ref, "mycel-test-compose");

        // Only one runtime row should exist
        let count: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM session_runtimes WHERE session_id = ?1",
                params![sid],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn host_crud_lifecycle() {
        let db = in_memory_db();

        // Add hosts
        db.add_host("devbox", "ssh://user@devbox", 4).unwrap();
        db.add_host("ci-runner", "ssh://ci@runner", 2).unwrap();

        // List
        let hosts = db.list_hosts().unwrap();
        assert_eq!(hosts.len(), 2);
        assert!(hosts[0].enabled);

        // Disable
        db.set_host_enabled("devbox", false).unwrap();
        let host = db.get_host_by_name("devbox").unwrap().unwrap();
        assert!(!host.enabled);

        // Remove
        assert!(db.remove_host("ci-runner").unwrap());
        assert!(!db.remove_host("nonexistent").unwrap());
        assert_eq!(db.list_hosts().unwrap().len(), 1);
    }

    #[test]
    fn count_sessions_on_host_counts_running_only() {
        let db = in_memory_db();
        let pid = seed_project(&db);
        let sid = seed_session(&db, pid);

        // Backfill creates a runtime on "local"
        assert_eq!(db.count_sessions_on_host("local").unwrap(), 1);
        assert_eq!(db.count_sessions_on_host("ssh://other").unwrap(), 0);

        // Mark as stopped — should not count
        let rt = db.get_session_runtime(sid).unwrap().unwrap();
        db.update_runtime_state(rt.id, "stopped").unwrap();
        assert_eq!(db.count_sessions_on_host("local").unwrap(), 0);
    }

    #[test]
    fn set_and_get_host_project() {
        let db = in_memory_db();
        let pid = seed_project(&db);
        db.add_host("devbox", "ssh://user@devbox", 4).unwrap();

        // Initially no project
        let host = db.get_host_by_name("devbox").unwrap().unwrap();
        assert!(host.current_project_id.is_none());
        assert!(db.get_host_project_name(host.id).unwrap().is_none());

        // Set project
        db.set_host_project("devbox", Some(pid)).unwrap();
        let host = db.get_host_by_name("devbox").unwrap().unwrap();
        assert_eq!(host.current_project_id, Some(pid));
        assert_eq!(
            db.get_host_project_name(host.id).unwrap().as_deref(),
            Some("test-project")
        );

        // Clear project
        db.set_host_project("devbox", None).unwrap();
        let host = db.get_host_by_name("devbox").unwrap().unwrap();
        assert!(host.current_project_id.is_none());
    }

    #[test]
    fn find_sessions_on_host_returns_running() {
        let db = in_memory_db();
        let pid = seed_project(&db);
        let sid = db
            .add_session(&NewSession {
                project_id: pid,
                name: "remote-s1",
                branch_name: "remote-s1",
                worktree_path: Path::new("/tmp/wt"),
                tmux_session: "mycel-test-remote-s1",
                runtime_kind: "remote",
                backend: "claude",
                note: None,
            })
            .unwrap();
        db.replace_session_runtime(&NewSessionRuntime {
            session_id: sid,
            provider: "remote",
            host: "ssh://devbox",
            runtime_ref: "mycel-test-remote-s1",
            compose_project: Some("mycel-test-remote-s1"),
            state: "running",
        })
        .unwrap();

        let sessions = db.find_sessions_on_host("ssh://devbox").unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].0.name, "remote-s1");

        // No sessions on other hosts
        assert!(db.find_sessions_on_host("ssh://other").unwrap().is_empty());

        // Mark as stopped — should not appear
        let rt = db.get_session_runtime(sid).unwrap().unwrap();
        db.update_runtime_state(rt.id, "stopped").unwrap();
        assert!(db.find_sessions_on_host("ssh://devbox").unwrap().is_empty());
    }

    #[test]
    fn hosts_include_current_project_id_in_list() {
        let db = in_memory_db();
        let pid = seed_project(&db);
        db.add_host("devbox", "ssh://user@devbox", 4).unwrap();
        db.set_host_project("devbox", Some(pid)).unwrap();

        let hosts = db.list_hosts().unwrap();
        assert_eq!(hosts[0].current_project_id, Some(pid));
    }
}
