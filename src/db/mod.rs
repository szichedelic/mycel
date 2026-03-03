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
    pub backend: &'a str,
    pub note: Option<&'a str>,
}

impl Database {
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
            "INSERT INTO sessions (project_id, name, branch_name, worktree_path, tmux_session, backend, note) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                session.project_id,
                session.name,
                session.branch_name,
                session.worktree_path.to_string_lossy(),
                session.tmux_session,
                session.backend,
                session.note
            ],
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_session_by_name(&self, project_id: i64, name: &str) -> Result<Option<Session>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, COALESCE(branch_name, name), worktree_path, tmux_session, backend, note, CAST(strftime('%s', created_at) AS INTEGER)
             FROM sessions WHERE project_id = ?1 AND name = ?2",
        )?;

        let result = stmt.query_row(params![project_id, name], |row| {
            Ok(Session {
                id: row.get(0)?,
                name: row.get(1)?,
                branch_name: row.get(2)?,
                worktree_path: PathBuf::from(row.get::<_, String>(3)?),
                tmux_session: row.get(4)?,
                backend: row.get(5)?,
                note: row.get(6)?,
                created_at_unix: row.get(7)?,
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
            "SELECT id, name, COALESCE(branch_name, name), worktree_path, tmux_session, backend, note, CAST(strftime('%s', created_at) AS INTEGER)
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
                    backend: row.get(5)?,
                    note: row.get(6)?,
                    created_at_unix: row.get(7)?,
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
}
