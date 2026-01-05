use anyhow::Result;
use std::collections::HashMap;
use std::process::Command;
use std::time::Duration;
use tokio::time::sleep;

use crate::db::Database;
use crate::notify;
use crate::session::SessionManager;

const CAPTURE_LINES: i32 = 80;
const WAITING_PROMPTS: [&str; 3] = [">", "Human:", "You:"];
const ERROR_MARKERS: [&str; 3] = ["error:", "panic", "traceback"];

#[derive(Clone, Copy, Default)]
struct SessionState {
    alive: bool,
    waiting: bool,
    errored: bool,
}

pub async fn run(interval_secs: u64) -> Result<()> {
    let db = Database::open()?;
    let session_manager = SessionManager::new();
    let mut states: HashMap<i64, SessionState> = HashMap::new();
    let mut initialized = false;

    loop {
        let mut next_states: HashMap<i64, SessionState> = HashMap::new();
        let projects = db.list_projects()?;

        for project in projects {
            let sessions = db.list_sessions(project.id).unwrap_or_default();

            for session in sessions {
                let alive = session_manager
                    .is_alive(&session.tmux_session)
                    .unwrap_or(false);
                let output = if alive {
                    capture_session_output(&session.tmux_session)
                } else {
                    None
                };
                let waiting = output
                    .as_ref()
                    .map(|text| is_waiting_prompt(text))
                    .unwrap_or(false);
                let errored = output
                    .as_ref()
                    .map(|text| contains_error(text))
                    .unwrap_or(false);

                if initialized {
                    let previous = states.get(&session.id).copied().unwrap_or_default();

                    if alive && waiting && !previous.waiting {
                        let title = "Mycel: input needed";
                        let body =
                            format!("{} ({}) is waiting for input.", session.name, project.name);
                        let _ = notify::send_notification(title, &body);
                    }

                    if alive && errored && !previous.errored {
                        let title = "Mycel: session error";
                        let body =
                            format!("{} ({}) reported an error.", session.name, project.name);
                        let _ = notify::send_notification(title, &body);
                    }

                    if !alive && previous.alive {
                        let title = "Mycel: session stopped";
                        let body = format!("{} ({}) session ended.", session.name, project.name);
                        let _ = notify::send_notification(title, &body);
                    }
                }

                next_states.insert(
                    session.id,
                    SessionState {
                        alive,
                        waiting,
                        errored,
                    },
                );
            }
        }

        states = next_states;
        initialized = true;
        sleep(Duration::from_secs(interval_secs.max(1))).await;
    }
}

fn capture_session_output(tmux_session: &str) -> Option<String> {
    let output = Command::new("tmux")
        .args([
            "capture-pane",
            "-p",
            "-t",
            tmux_session,
            "-S",
            &format!("-{CAPTURE_LINES}"),
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    Some(String::from_utf8_lossy(&output.stdout).to_string())
}

fn is_waiting_prompt(output: &str) -> bool {
    let last_line = output
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("");
    let trimmed = last_line.trim();
    WAITING_PROMPTS.contains(&trimmed)
}

fn contains_error(output: &str) -> bool {
    let lowered = output.to_lowercase();
    ERROR_MARKERS.iter().any(|marker| lowered.contains(marker))
}
