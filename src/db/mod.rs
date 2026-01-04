use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::path::PathBuf;

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
    pub project_id: i64,
    pub name: String,
    pub worktree_path: PathBuf,
    pub tmux_session: String,
}

impl Database {
    pub fn open() -> Result<Self> {
        let db_path = dirs::data_dir()
            .context("Could not find data directory")?
            .join("mycel")
            .join("mycel.db");

        // Ensure directory exists
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
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (project_id) REFERENCES projects(id),
                UNIQUE(project_id, name)
            );
            ",
        )?;

        Ok(())
    }

    pub fn add_project(&self, name: &str, path: &PathBuf) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO projects (name, path) VALUES (?1, ?2)",
            params![name, path.to_string_lossy()],
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_project_by_path(&self, path: &PathBuf) -> Result<Option<Project>> {
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

    pub fn add_session(
        &self,
        project_id: i64,
        name: &str,
        worktree_path: &PathBuf,
        tmux_session: &str,
    ) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO sessions (project_id, name, worktree_path, tmux_session) VALUES (?1, ?2, ?3, ?4)",
            params![project_id, name, worktree_path.to_string_lossy(), tmux_session],
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_session_by_name(&self, project_id: i64, name: &str) -> Result<Option<Session>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, project_id, name, worktree_path, tmux_session FROM sessions WHERE project_id = ?1 AND name = ?2",
        )?;

        let result = stmt.query_row(params![project_id, name], |row| {
            Ok(Session {
                id: row.get(0)?,
                project_id: row.get(1)?,
                name: row.get(2)?,
                worktree_path: PathBuf::from(row.get::<_, String>(3)?),
                tmux_session: row.get(4)?,
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
            "SELECT id, project_id, name, worktree_path, tmux_session FROM sessions WHERE project_id = ?1",
        )?;

        let sessions = stmt
            .query_map(params![project_id], |row| {
                Ok(Session {
                    id: row.get(0)?,
                    project_id: row.get(1)?,
                    name: row.get(2)?,
                    worktree_path: PathBuf::from(row.get::<_, String>(3)?),
                    tmux_session: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(sessions)
    }

    pub fn list_all_sessions(&self) -> Result<Vec<(Project, Session)>> {
        let mut stmt = self.conn.prepare(
            "SELECT p.id, p.name, p.path, s.id, s.project_id, s.name, s.worktree_path, s.tmux_session
             FROM sessions s
             JOIN projects p ON s.project_id = p.id
             ORDER BY p.name, s.name",
        )?;

        let results = stmt
            .query_map([], |row| {
                let project = Project {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    path: PathBuf::from(row.get::<_, String>(2)?),
                };
                let session = Session {
                    id: row.get(3)?,
                    project_id: row.get(4)?,
                    name: row.get(5)?,
                    worktree_path: PathBuf::from(row.get::<_, String>(6)?),
                    tmux_session: row.get(7)?,
                };
                Ok((project, session))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(results)
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
}
