use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame, Terminal,
};
use std::io::{self, Write};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::bank::{self, BankedItem};
use crate::config::ProjectConfig;
use crate::confirm;
use crate::db::{Database, Project, Session};
use crate::disk;
use crate::session::SessionManager;
use crate::worktree;

mod logo;

const PREVIEW_LINES: usize = 80;

struct App {
    db: Database,
    session_manager: SessionManager,
    projects: Vec<ProjectWithSessions>,
    banked: Vec<(String, BankedItem)>,
    total_worktree_bytes: u64,
    disk_usage: Option<disk::DiskUsage>,
    selected: usize,
    should_quit: bool,
    show_logo: bool,
    search_query: String,
    search_mode: bool,
    preview_enabled: bool,
    preview_text: String,
    preview_session_id: Option<i64>,
}

struct ProjectWithSessions {
    project: Project,
    sessions: Vec<SessionWithStatus>,
    expanded: bool,
}

struct SessionWithStatus {
    session: Session,
    is_running: bool,
    worktree_bytes: Option<u64>,
}

impl App {
    fn new() -> Result<Self> {
        let db = Database::open()?;
        let session_manager = SessionManager::new();

        let mut app = Self {
            db,
            session_manager,
            projects: Vec::new(),
            banked: Vec::new(),
            total_worktree_bytes: 0,
            disk_usage: None,
            selected: 0,
            should_quit: false,
            show_logo: true,
            search_query: String::new(),
            search_mode: false,
            preview_enabled: false,
            preview_text: String::new(),
            preview_session_id: None,
        };

        app.refresh()?;
        Ok(app)
    }

    fn refresh(&mut self) -> Result<()> {
        let projects = self.db.list_projects()?;
        let mut refreshed_projects = Vec::new();
        let mut total_worktree_bytes: u64 = 0;
        let mut disk_usages = Vec::new();

        for project in projects {
            let sessions = self.db.list_sessions(project.id).unwrap_or_default();
            let mut sessions_with_status = Vec::new();

            for session in sessions {
                let is_running = self
                    .session_manager
                    .is_alive(&session.tmux_session)
                    .unwrap_or(false);
                let worktree_bytes = disk::dir_size_bytes(&session.worktree_path);

                if let Some(size) = worktree_bytes {
                    total_worktree_bytes = total_worktree_bytes.saturating_add(size);
                }

                if worktree_bytes.is_some() {
                    if let Some(usage) = disk::filesystem_usage(&session.worktree_path) {
                        disk_usages.push(usage);
                    }
                }

                sessions_with_status.push(SessionWithStatus {
                    session,
                    is_running,
                    worktree_bytes,
                });
            }

            refreshed_projects.push(ProjectWithSessions {
                project,
                sessions: sessions_with_status,
                expanded: true,
            });
        }

        if disk_usages.is_empty() {
            for project in &refreshed_projects {
                if let Some(usage) = disk::filesystem_usage(&project.project.path) {
                    disk_usages.push(usage);
                }
            }
        }

        self.projects = refreshed_projects;
        self.total_worktree_bytes = total_worktree_bytes;
        self.disk_usage = lowest_disk_usage(&disk_usages);

        self.banked.clear();
        for project in &self.projects {
            if let Ok(items) = bank::list_banked(&project.project.name) {
                for item in items {
                    self.banked.push((project.project.name.clone(), item));
                }
            }
        }

        self.clamp_selection();
        self.update_preview(true);
        Ok(())
    }

    fn update_preview(&mut self, force: bool) {
        if !self.preview_enabled {
            self.preview_text.clear();
            self.preview_session_id = None;
            return;
        }

        let selected = match self.get_selected_item() {
            Some(SelectedItem::Session(_, session)) => {
                Some((session.session.id, session.session.tmux_session.clone()))
            }
            _ => None,
        };

        match selected {
            Some((session_id, tmux_session)) => {
                if !force && self.preview_session_id == Some(session_id) {
                    return;
                }
                self.preview_session_id = Some(session_id);
                let output = capture_session_output(&tmux_session, PREVIEW_LINES);
                self.preview_text = output.unwrap_or_default();
            }
            None => {
                if self.preview_session_id.is_some() {
                    self.preview_session_id = None;
                    self.preview_text.clear();
                }
            }
        }
    }

    fn total_items(&self) -> usize {
        self.view_items().len()
    }

    fn get_selected_item(&self) -> Option<SelectedItem> {
        let items = self.view_items();
        let item = items.get(self.selected)?;
        match item {
            ViewItem::Project(project) => Some(SelectedItem::Project(project)),
            ViewItem::Session {
                project, session, ..
            } => Some(SelectedItem::Session(project, session)),
            ViewItem::Banked { project_name, item } => {
                Some(SelectedItem::Banked(project_name, item))
            }
            ViewItem::Spacer | ViewItem::BankedHeader => None,
        }
    }

    fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
        self.update_preview(false);
    }

    fn move_down(&mut self) {
        let total = self.total_items();
        if total > 0 && self.selected < total - 1 {
            self.selected += 1;
        }
        self.update_preview(false);
    }

    fn toggle_expand(&mut self) {
        let selected_id = {
            let items = self.view_items();
            match items.get(self.selected) {
                Some(ViewItem::Project(selected)) => Some(selected.project.id),
                _ => None,
            }
        };

        if let Some(selected_id) = selected_id {
            if let Some(project) = self
                .projects
                .iter_mut()
                .find(|p| p.project.id == selected_id)
            {
                project.expanded = !project.expanded;
            }
        }

        self.clamp_selection();
        self.update_preview(false);
    }

    fn clamp_selection(&mut self) {
        let total = self.total_items();
        if total == 0 {
            self.selected = 0;
        } else if self.selected >= total {
            self.selected = total - 1;
        }
    }

    fn clear_search(&mut self) {
        if !self.search_query.is_empty() || self.search_mode {
            self.search_query.clear();
            self.search_mode = false;
            self.clamp_selection();
            self.update_preview(false);
        }
    }

    fn view_items(&self) -> Vec<ViewItem<'_>> {
        let mut items = Vec::new();
        let query = self.search_query.trim();
        let matcher = SkimMatcherV2::default().ignore_case();

        for project in &self.projects {
            let mut matching_sessions = Vec::new();
            for session in &project.sessions {
                let matches =
                    query.is_empty() || matcher.fuzzy_match(&session.session.name, query).is_some();
                if matches {
                    matching_sessions.push(session);
                }
            }

            if query.is_empty() || !matching_sessions.is_empty() {
                items.push(ViewItem::Project(project));
                let show_sessions = project.expanded || !query.is_empty();
                if show_sessions {
                    for (idx, session) in matching_sessions.iter().enumerate() {
                        let is_last = idx + 1 == matching_sessions.len();
                        items.push(ViewItem::Session {
                            project,
                            session,
                            is_last,
                        });
                    }
                }
            }
        }

        let mut banked_matches = Vec::new();
        for (project_name, item) in &self.banked {
            let matches = query.is_empty() || matcher.fuzzy_match(&item.name, query).is_some();
            if matches {
                banked_matches.push((project_name.as_str(), item));
            }
        }

        if !banked_matches.is_empty() {
            items.push(ViewItem::Spacer);
            items.push(ViewItem::BankedHeader);
            for (project_name, item) in banked_matches {
                items.push(ViewItem::Banked { project_name, item });
            }
        }

        items
    }
}

enum SelectedItem<'a> {
    Project(&'a ProjectWithSessions),
    Session(&'a ProjectWithSessions, &'a SessionWithStatus),
    Banked(&'a str, &'a BankedItem),
}

enum ViewItem<'a> {
    Project(&'a ProjectWithSessions),
    Session {
        project: &'a ProjectWithSessions,
        session: &'a SessionWithStatus,
        is_last: bool,
    },
    Spacer,
    BankedHeader,
    Banked {
        project_name: &'a str,
        item: &'a BankedItem,
    },
}

pub async fn run() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new()?;

    for frame in 0..5 {
        terminal.draw(|f| logo::draw_animated(f, frame))?;
        std::thread::sleep(std::time::Duration::from_millis(150));
    }
    std::thread::sleep(std::time::Duration::from_millis(500));
    app.show_logo = false;

    loop {
        terminal.draw(|f| draw_ui(f, &app))?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    if app.search_mode {
                        match key.code {
                            KeyCode::Esc => app.clear_search(),
                            KeyCode::Enter => app.search_mode = false,
                            KeyCode::Backspace => {
                                app.search_query.pop();
                                app.clamp_selection();
                                app.update_preview(false);
                            }
                            KeyCode::Char(c) => {
                                app.search_query.push(c);
                                app.clamp_selection();
                                app.update_preview(false);
                            }
                            _ => {}
                        }
                        continue;
                    }

                    match key.code {
                        KeyCode::Char('q') => app.should_quit = true,
                        KeyCode::Char('j') | KeyCode::Down => app.move_down(),
                        KeyCode::Char('k') | KeyCode::Up => app.move_up(),
                        KeyCode::Enter | KeyCode::Char(' ') => app.toggle_expand(),
                        KeyCode::Char('/') => app.search_mode = true,
                        KeyCode::Esc => app.clear_search(),
                        KeyCode::Char('a') => {
                            if let Some(SelectedItem::Session(project, session)) =
                                app.get_selected_item()
                            {
                                let project_name = project.project.name.clone();
                                let project_path = project.project.path.clone();
                                let session_name = session.session.name.clone();
                                let tmux_session = session.session.tmux_session.clone();
                                let worktree_path = session.session.worktree_path.clone();

                                // Restore terminal before attaching
                                disable_raw_mode()?;
                                execute!(
                                    terminal.backend_mut(),
                                    LeaveAlternateScreen,
                                    DisableMouseCapture
                                )?;

                                if !app.session_manager.is_alive(&tmux_session)? {
                                    println!("Session '{session_name}' is not running. Restarting...");
                                    let config = ProjectConfig::load(&project_path)?;
                                    app.session_manager.create(
                                        &project_name,
                                        &session_name,
                                        &worktree_path,
                                        &config.setup,
                                    )?;
                                }

                                app.session_manager.attach(&tmux_session)?;

                                // Restore TUI after detaching
                                enable_raw_mode()?;
                                execute!(
                                    terminal.backend_mut(),
                                    EnterAlternateScreen,
                                    EnableMouseCapture
                                )?;
                                terminal.clear()?;
                                app.refresh()?;
                            }
                        }
                        KeyCode::Char('r') => {
                            app.refresh()?;
                            terminal.clear()?;
                        }
                        KeyCode::Char('v') => {
                            app.preview_enabled = !app.preview_enabled;
                            app.update_preview(true);
                            terminal.clear()?;
                        }
                        KeyCode::Char('s') => {
                            let project = match app.get_selected_item() {
                                Some(SelectedItem::Project(p)) => Some(&p.project),
                                Some(SelectedItem::Session(p, _)) => Some(&p.project),
                                _ => None,
                            };

                            if let Some(project) = project {
                                disable_raw_mode()?;
                                execute!(
                                    terminal.backend_mut(),
                                    LeaveAlternateScreen,
                                    DisableMouseCapture
                                )?;

                                print!("Session name: ");
                                io::stdout().flush()?;
                                let mut name = String::new();
                                io::stdin().read_line(&mut name)?;
                                let name = name.trim();

                                if !name.is_empty() {
                                    print!("Session note (optional): ");
                                    io::stdout().flush()?;
                                    let mut note = String::new();
                                    io::stdin().read_line(&mut note)?;
                                    let note = note.trim();
                                    let note = if note.is_empty() {
                                        None
                                    } else {
                                        Some(note.to_string())
                                    };

                                    let config = ProjectConfig::load(&project.path)?;
                                    let sanitized = worktree::sanitize_branch_name(name);

                                    let branch_exists = std::process::Command::new("git")
                                        .args([
                                            "show-ref",
                                            "--verify",
                                            "--quiet",
                                            &format!("refs/heads/{sanitized}"),
                                        ])
                                        .current_dir(&project.path)
                                        .status()
                                        .map(|s| s.success())
                                        .unwrap_or(false);

                                    let (worktree_path, branch_name) = if branch_exists {
                                        println!("Using existing branch '{sanitized}'...");
                                        worktree::create_from_existing(
                                            &project.path,
                                            &sanitized,
                                            &config,
                                        )?
                                    } else {
                                        println!("Creating worktree '{sanitized}'...");
                                        worktree::create(&project.path, name, &config)?
                                    };

                                    println!("Starting Claude session...");
                                    if !config.setup.is_empty() {
                                        println!("Setup: {}", config.setup.join(" && "));
                                    }
                                    let tmux_session = app.session_manager.create(
                                        &project.name,
                                        &branch_name,
                                        &worktree_path,
                                        &config.setup,
                                    )?;
                                    app.db.add_session(
                                        project.id,
                                        &branch_name,
                                        &worktree_path,
                                        &tmux_session,
                                        note.as_deref(),
                                    )?;
                                    println!("Session '{branch_name}' created.");
                                    std::thread::sleep(std::time::Duration::from_millis(500));
                                }

                                enable_raw_mode()?;
                                execute!(
                                    terminal.backend_mut(),
                                    EnterAlternateScreen,
                                    EnableMouseCapture
                                )?;
                                terminal.clear()?;
                                app.refresh()?;
                            }
                        }
                        KeyCode::Char('n') => {
                            if let Some(SelectedItem::Session(_, session)) = app.get_selected_item()
                            {
                                let session_id = session.session.id;
                                let session_name = session.session.name.clone();
                                let current_note = session.session.note.clone().unwrap_or_default();

                                disable_raw_mode()?;
                                execute!(
                                    terminal.backend_mut(),
                                    LeaveAlternateScreen,
                                    DisableMouseCapture
                                )?;

                                println!("Edit note for '{session_name}' (blank to clear):");
                                if !current_note.is_empty() {
                                    println!("Current: {current_note}");
                                }
                                print!("New note: ");
                                io::stdout().flush()?;
                                let mut note = String::new();
                                io::stdin().read_line(&mut note)?;
                                let note = note.trim();
                                let note = if note.is_empty() {
                                    None
                                } else {
                                    Some(note.to_string())
                                };

                                app.db.update_session_note(session_id, note.as_deref())?;
                                println!("Note updated.");
                                std::thread::sleep(std::time::Duration::from_millis(500));

                                enable_raw_mode()?;
                                execute!(
                                    terminal.backend_mut(),
                                    EnterAlternateScreen,
                                    EnableMouseCapture
                                )?;
                                terminal.clear()?;
                                app.refresh()?;
                            }
                        }
                        KeyCode::Char('p') => {
                            if let Some(SelectedItem::Session(_, session)) = app.get_selected_item()
                            {
                                let session_name = session.session.name.clone();
                                let tmux_session = session.session.tmux_session.clone();
                                let worktree_path = session.session.worktree_path.clone();

                                disable_raw_mode()?;
                                execute!(
                                    terminal.backend_mut(),
                                    LeaveAlternateScreen,
                                    DisableMouseCapture
                                )?;

                                let prompt = format!("Stop session '{session_name}'?");
                                let confirmed = confirm::prompt_confirm(&prompt)?;

                                if confirmed {
                                    if app.session_manager.is_alive(&tmux_session)? {
                                        println!("Stopping session '{session_name}'...");
                                        app.session_manager.kill(&tmux_session)?;
                                    } else {
                                        println!("Session '{session_name}' already stopped.");
                                    }
                                    println!(
                                        "Worktree preserved at: {}",
                                        worktree_path.display()
                                    );
                                    std::thread::sleep(std::time::Duration::from_millis(800));
                                } else {
                                    println!("Cancelled.");
                                    std::thread::sleep(std::time::Duration::from_millis(500));
                                }

                                enable_raw_mode()?;
                                execute!(
                                    terminal.backend_mut(),
                                    EnterAlternateScreen,
                                    EnableMouseCapture
                                )?;
                                terminal.clear()?;
                                app.refresh()?;
                            }
                        }
                        KeyCode::Char('x') => {
                            if let Some(SelectedItem::Session(project, session)) =
                                app.get_selected_item()
                            {
                                let session_id = session.session.id;
                                let session_name = session.session.name.clone();
                                let tmux_session = session.session.tmux_session.clone();
                                let worktree_path = session.session.worktree_path.clone();
                                let project_path = project.project.path.clone();

                                disable_raw_mode()?;
                                execute!(
                                    terminal.backend_mut(),
                                    LeaveAlternateScreen,
                                    DisableMouseCapture
                                )?;

                                let prompt = format!(
                                    "Kill session '{session_name}' and remove its worktree?"
                                );
                                let confirmed = confirm::prompt_confirm(&prompt)?;

                                if confirmed {
                                    if app.session_manager.is_alive(&tmux_session)? {
                                        app.session_manager.kill(&tmux_session)?;
                                    }

                                    let _ = worktree::remove(&project_path, &worktree_path);

                                    app.db.delete_session(session_id)?;
                                } else {
                                    println!("Cancelled.");
                                    std::thread::sleep(std::time::Duration::from_millis(500));
                                }

                                enable_raw_mode()?;
                                execute!(
                                    terminal.backend_mut(),
                                    EnterAlternateScreen,
                                    EnableMouseCapture
                                )?;
                                terminal.clear()?;
                                app.refresh()?;
                            }
                        }
                        KeyCode::Char('b') => {
                            if let Some(SelectedItem::Session(project, session)) =
                                app.get_selected_item()
                            {
                                let session_id = session.session.id;
                                let session_name = session.session.name.clone();
                                let tmux_session = session.session.tmux_session.clone();
                                let worktree_path = session.session.worktree_path.clone();
                                let project_path = project.project.path.clone();
                                let project_name = project.project.name.clone();

                                disable_raw_mode()?;
                                execute!(
                                    terminal.backend_mut(),
                                    LeaveAlternateScreen,
                                    DisableMouseCapture
                                )?;

                                let status_output = std::process::Command::new("git")
                                    .args(["status", "--porcelain"])
                                    .current_dir(&worktree_path)
                                    .output();

                                let has_changes =
                                    status_output.map(|o| !o.stdout.is_empty()).unwrap_or(false);

                                if has_changes {
                                    println!("Error: Uncommitted changes in worktree.");
                                    println!("Commit your work first:");
                                    println!("  cd {}", worktree_path.display());
                                    println!("  git add -A && git commit -m \"your message\"");
                                    std::thread::sleep(std::time::Duration::from_millis(2000));
                                } else {
                                    let config = ProjectConfig::load(&project_path)?;
                                    let bundle_path =
                                        bank::bundle_path(&project_name, &session_name)?;

                                    if bundle_path.exists() {
                                        println!(
                                            "Error: Bundle already exists: {}",
                                            bundle_path.display()
                                        );
                                        std::thread::sleep(std::time::Duration::from_millis(1500));
                                    } else {
                                        let prompt = format!(
                                            "Banking '{session_name}' will stop the session, remove its worktree, and delete the local branch. Continue?"
                                        );
                                        let confirmed = confirm::prompt_confirm(&prompt)?;
                                        if !confirmed {
                                            println!("Cancelled.");
                                            std::thread::sleep(std::time::Duration::from_millis(
                                                500,
                                            ));
                                        } else {
                                            println!("Banking '{session_name}'...");
                                            if let Err(e) = bank::create_bundle(
                                                &project_path,
                                                &session_name,
                                                &config.base_branch,
                                                &bundle_path,
                                            ) {
                                                println!("Error creating bundle: {e}");
                                                std::thread::sleep(
                                                    std::time::Duration::from_millis(1500),
                                                );
                                            } else {
                                                if app.session_manager.is_alive(&tmux_session)? {
                                                    println!("Stopping session...");
                                                    app.session_manager.kill(&tmux_session)?;
                                                }

                                                println!("Removing worktree...");
                                                let _ =
                                                    worktree::remove(&project_path, &worktree_path);

                                                app.db.delete_session(session_id)?;

                                                let _ = std::process::Command::new("git")
                                                    .args(["branch", "-D", &session_name])
                                                    .current_dir(&project_path)
                                                    .status();

                                                println!("Banked '{session_name}'");
                                                std::thread::sleep(
                                                    std::time::Duration::from_millis(500),
                                                );
                                            }
                                        }
                                    }
                                }

                                enable_raw_mode()?;
                                execute!(
                                    terminal.backend_mut(),
                                    EnterAlternateScreen,
                                    EnableMouseCapture
                                )?;
                                terminal.clear()?;
                                app.refresh()?;
                            }
                        }
                        KeyCode::Char('u') => {
                            if let Some(SelectedItem::Banked(project_name, item)) =
                                app.get_selected_item()
                            {
                                let project_name = project_name.to_string();
                                let item_name = item.name.clone();

                                if let Some(proj) =
                                    app.projects.iter().find(|p| p.project.name == project_name)
                                {
                                    let git_root = proj.project.path.clone();
                                    let project_id = proj.project.id;
                                    let bundle_path = item.path.clone();

                                    disable_raw_mode()?;
                                    execute!(
                                        terminal.backend_mut(),
                                        LeaveAlternateScreen,
                                        DisableMouseCapture
                                    )?;

                                    let prompt = format!(
                                        "Unbanking '{item_name}' will restore the branch and delete the bundle. Continue?"
                                    );
                                    let confirmed = confirm::prompt_confirm(&prompt)?;
                                    if !confirmed {
                                        println!("Cancelled.");
                                        std::thread::sleep(std::time::Duration::from_millis(500));
                                    } else {
                                        println!("Restoring '{item_name}'...");
                                        if let Err(e) = bank::restore_bundle(
                                            &git_root,
                                            &bundle_path,
                                            &item_name,
                                        ) {
                                            println!("Error restoring bundle: {e}");
                                            std::thread::sleep(std::time::Duration::from_millis(
                                                1000,
                                            ));
                                        } else {
                                            bank::delete_bundle(&bundle_path)?;

                                            let config = ProjectConfig::load(&git_root)?;
                                            println!("Creating worktree...");
                                            let (worktree_path, branch_name) =
                                                worktree::create_from_existing(
                                                    &git_root, &item_name, &config,
                                                )?;

                                            println!("Starting Claude session...");
                                            let tmux_session = app.session_manager.create(
                                                &project_name,
                                                &branch_name,
                                                &worktree_path,
                                                &config.setup,
                                            )?;
                                            app.db.add_session(
                                                project_id,
                                                &branch_name,
                                                &worktree_path,
                                                &tmux_session,
                                                None,
                                            )?;

                                            println!("Session '{item_name}' restored.");
                                            std::thread::sleep(std::time::Duration::from_millis(
                                                500,
                                            ));
                                        }
                                    }

                                    enable_raw_mode()?;
                                    execute!(
                                        terminal.backend_mut(),
                                        EnterAlternateScreen,
                                        EnableMouseCapture
                                    )?;
                                    terminal.clear()?;
                                    app.refresh()?;
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;

    Ok(())
}

fn draw_ui(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(f.area());

    let now_unix = current_unix_timestamp();
    let session_count: usize = app.projects.iter().map(|p| p.sessions.len()).sum();
    let running_count: usize = app
        .projects
        .iter()
        .flat_map(|p| &p.sessions)
        .filter(|s| s.is_running)
        .count();
    let total_worktree = format_bytes(app.total_worktree_bytes);
    let mut disk_spans = vec![
        Span::styled("  Worktrees: ", Style::default().fg(Color::DarkGray)),
        Span::styled(total_worktree, Style::default().fg(Color::White)),
    ];

    if let Some(usage) = app.disk_usage {
        if let Some(percent) = disk_free_percent(usage) {
            let free_str = format_bytes(usage.available_bytes);
            let low_disk = is_low_disk(usage);
            let free_style = if low_disk {
                Style::default()
                    .fg(Color::Red)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Green)
            };

            disk_spans.push(Span::styled("  │  Free: ", Style::default().fg(Color::DarkGray)));
            disk_spans.push(Span::styled(
                format!("{free_str} ({percent}%)"),
                free_style,
            ));
            if low_disk {
                disk_spans.push(Span::styled(
                    "  LOW",
                    Style::default()
                        .fg(Color::Red)
                        .add_modifier(Modifier::BOLD),
                ));
            }
        }
    }

    let header_lines = vec![
        Line::from(vec![
            Span::styled("      ", Style::default()),
            Span::styled("░▒▓", Style::default().fg(Color::Rgb(0, 80, 100))),
            Span::styled("█", Style::default().fg(Color::Rgb(0, 120, 120))),
            Span::styled("●", Style::default().fg(Color::Rgb(50, 180, 140))),
            Span::styled("█", Style::default().fg(Color::Rgb(0, 120, 120))),
            Span::styled("▓▒░", Style::default().fg(Color::Rgb(0, 80, 100))),
        ]),
        Line::from(vec![
            Span::styled("    ", Style::default()),
            Span::styled("░▒", Style::default().fg(Color::Rgb(0, 60, 80))),
            Span::styled("───", Style::default().fg(Color::Rgb(0, 100, 110))),
            Span::styled("●", Style::default().fg(Color::Rgb(50, 180, 140))),
            Span::styled("───", Style::default().fg(Color::Rgb(0, 100, 110))),
            Span::styled("▒░", Style::default().fg(Color::Rgb(0, 60, 80))),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "  M Y C E L",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("  │  ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{session_count} sessions"),
                Style::default().fg(Color::White),
            ),
            Span::styled("  ", Style::default()),
            Span::styled(
                format!("{running_count} running"),
                Style::default().fg(Color::Green),
            ),
            if !app.banked.is_empty() {
                Span::styled(
                    format!("  │  {} banked", app.banked.len()),
                    Style::default().fg(Color::Magenta),
                )
            } else {
                Span::styled("", Style::default())
            },
        ]),
        Line::from(disk_spans),
    ];

    let header = Paragraph::new(header_lines).block(Block::default().borders(Borders::BOTTOM));
    f.render_widget(header, chunks[0]);

    let main_area = chunks[1];
    let (list_area, preview_area) = if app.preview_enabled {
        let split = if main_area.width >= 110 {
            Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(62), Constraint::Percentage(38)])
                .split(main_area)
        } else {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
                .split(main_area)
        };
        (split[0], Some(split[1]))
    } else {
        (main_area, None)
    };

    let mut items: Vec<ListItem> = Vec::new();
    let query = app.search_query.trim();
    let view_items = app.view_items();

    for (idx, item) in view_items.iter().enumerate() {
        match item {
            ViewItem::Project(project) => {
                let expand_icon = if project.expanded || !query.is_empty() {
                    "▼"
                } else {
                    "▶"
                };
                let project_line = format!(
                    "{} {} ({})",
                    expand_icon,
                    project.project.name,
                    project.project.path.display()
                );

                let style = if idx == app.selected {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };

                items.push(ListItem::new(Line::from(Span::styled(project_line, style))));
            }
            ViewItem::Session {
                session, is_last, ..
            } => {
                let prefix = if *is_last { "  └─" } else { "  ├─" };
                let status = if session.is_running {
                    Span::styled("●", Style::default().fg(Color::Green))
                } else {
                    Span::styled("○", Style::default().fg(Color::DarkGray))
                };
                let age = format_relative_age(session.session.created_at_unix, now_unix);

                let session_style = if idx == app.selected {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };

                let mut spans = vec![
                    Span::styled(format!("{prefix} "), Style::default().fg(Color::DarkGray)),
                    Span::styled(&session.session.name, session_style),
                    Span::raw("  "),
                    status,
                    Span::styled(
                        if session.is_running {
                            " running"
                        } else {
                            " stopped"
                        },
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(format!("  {age}"), Style::default().fg(Color::DarkGray)),
                ];

                let size_span = match session.worktree_bytes {
                    Some(size) => Span::styled(
                        format!("  {}", format_bytes(size)),
                        worktree_size_style(size),
                    ),
                    None => Span::styled("  n/a", Style::default().fg(Color::DarkGray)),
                };
                spans.push(size_span);

                if let Some(note) = &session.session.note {
                    let note = note.trim();
                    if !note.is_empty() {
                        let note = format_note_excerpt(note, 40);
                        spans.push(Span::styled(
                            format!("  - {note}"),
                            Style::default().fg(Color::DarkGray),
                        ));
                    }
                }

                let line = Line::from(spans);

                items.push(ListItem::new(line));
            }
            ViewItem::Spacer => items.push(ListItem::new(Line::from(""))),
            ViewItem::BankedHeader => {
                items.push(ListItem::new(Line::from(Span::styled(
                    "📦 BANKED",
                    Style::default()
                        .fg(Color::Magenta)
                        .add_modifier(Modifier::BOLD),
                ))));
            }
            ViewItem::Banked { project_name, item } => {
                let style = if idx == app.selected {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };

                let line = Line::from(vec![
                    Span::styled("  ", Style::default()),
                    Span::styled(&item.name, style),
                    Span::styled(
                        format!("  ({project_name}) "),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(item.size_human(), Style::default().fg(Color::DarkGray)),
                ]);

                items.push(ListItem::new(line));
            }
        }
    }

    let list_title = if query.is_empty() {
        "Sessions".to_string()
    } else {
        format!("Sessions (filter: {query})")
    };

    if items.is_empty() {
        let empty_msg = Paragraph::new(if query.is_empty() {
            "No projects registered. Run 'mycel init' in a git repository."
        } else {
            "No matching sessions."
        })
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().borders(Borders::ALL).title(list_title));
        f.render_widget(empty_msg, list_area);
    } else {
        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(list_title))
            .highlight_style(Style::default().add_modifier(Modifier::BOLD));
        f.render_widget(list, list_area);
    }

    if let Some(preview_area) = preview_area {
        let (preview_title, preview_body, preview_style) = match app.get_selected_item() {
            Some(SelectedItem::Session(_, session)) => {
                let status = if session.is_running { "running" } else { "stopped" };
                let title = format!("Preview: {} ({status})", session.session.name);
                let body = if !app.preview_text.trim().is_empty() {
                    app.preview_text.clone()
                } else if session.is_running {
                    "No recent output.".to_string()
                } else {
                    "Session stopped.".to_string()
                };
                (title, body, Style::default().fg(Color::White))
            }
            _ => (
                "Preview".to_string(),
                "Select a session to preview output.".to_string(),
                Style::default().fg(Color::DarkGray),
            ),
        };

        let max_lines = preview_area.height.saturating_sub(2) as usize;
        let preview_body = tail_lines(&preview_body, max_lines);
        let preview = Paragraph::new(preview_body)
            .style(preview_style)
            .block(Block::default().borders(Borders::ALL).title(preview_title));
        f.render_widget(preview, preview_area);
    }

    let mut footer_text = " [a]ttach  [s]pawn  [n]ote  [p]ause  [v]iew  [b]ank  [u]nbank  [x] kill  [r]efresh  [/] search  [q]uit"
        .to_string();
    if app.search_mode || !app.search_query.is_empty() {
        footer_text.push_str("  [esc] clear");
        if app.search_mode {
            footer_text.push_str("  [enter] done");
        }
    }

    let footer = Paragraph::new(footer_text)
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().borders(Borders::TOP));
    f.render_widget(footer, chunks[2]);
}

fn capture_session_output(tmux_session: &str, lines: usize) -> Option<String> {
    let output = Command::new("tmux")
        .args([
            "capture-pane",
            "-p",
            "-t",
            tmux_session,
            "-S",
            &format!("-{lines}"),
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    Some(
        String::from_utf8_lossy(&output.stdout)
            .trim_end_matches('\n')
            .to_string(),
    )
}

fn tail_lines(text: &str, max_lines: usize) -> String {
    if max_lines == 0 {
        return String::new();
    }

    let lines: Vec<&str> = text.lines().collect();
    if lines.len() <= max_lines {
        return text.to_string();
    }

    lines[lines.len() - max_lines..].join("\n")
}

fn current_unix_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn format_relative_age(created_at_unix: i64, now_unix: i64) -> String {
    let age_secs = if now_unix > created_at_unix {
        (now_unix - created_at_unix) as u64
    } else {
        0
    };

    if age_secs < 60 {
        "just now".to_string()
    } else if age_secs < 3600 {
        format!("{}m ago", age_secs / 60)
    } else if age_secs < 86_400 {
        format!("{}h ago", age_secs / 3600)
    } else if age_secs < 604_800 {
        format!("{}d ago", age_secs / 86_400)
    } else {
        format!("{}w ago", age_secs / 604_800)
    }
}

fn format_note_excerpt(note: &str, max_len: usize) -> String {
    if max_len <= 3 {
        return note.chars().take(max_len).collect();
    }

    let mut chars = note.chars();
    let mut excerpt: String = chars.by_ref().take(max_len).collect();
    if chars.next().is_some() {
        excerpt = excerpt.chars().take(max_len - 3).collect();
        excerpt.push_str("...");
    }

    excerpt
}

const LARGE_WORKTREE_BYTES: u64 = 1024 * 1024 * 1024;
const HUGE_WORKTREE_BYTES: u64 = 5 * 1024 * 1024 * 1024;
const LOW_DISK_FREE_BYTES: u64 = 5 * 1024 * 1024 * 1024;
const LOW_DISK_FREE_RATIO: f64 = 0.10;

fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes < 1024_u64.pow(4) {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    } else {
        format!("{:.1} TB", bytes as f64 / (1024.0 * 1024.0 * 1024.0 * 1024.0))
    }
}

fn worktree_size_style(bytes: u64) -> Style {
    if bytes >= HUGE_WORKTREE_BYTES {
        Style::default()
            .fg(Color::Red)
            .add_modifier(Modifier::BOLD)
    } else if bytes >= LARGE_WORKTREE_BYTES {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    }
}

fn disk_free_percent(usage: disk::DiskUsage) -> Option<u64> {
    if usage.total_bytes == 0 {
        return None;
    }
    Some(
        (usage.available_bytes.saturating_mul(100) / usage.total_bytes)
            .min(100),
    )
}

fn is_low_disk(usage: disk::DiskUsage) -> bool {
    if usage.total_bytes == 0 {
        return false;
    }
    let free_ratio = usage.available_bytes as f64 / usage.total_bytes as f64;
    usage.available_bytes < LOW_DISK_FREE_BYTES || free_ratio < LOW_DISK_FREE_RATIO
}

fn lowest_disk_usage(usages: &[disk::DiskUsage]) -> Option<disk::DiskUsage> {
    usages
        .iter()
        .copied()
        .filter(|usage| usage.total_bytes > 0)
        .min_by(|a, b| {
            let left = (a.available_bytes as u128).saturating_mul(b.total_bytes as u128);
            let right = (b.available_bytes as u128).saturating_mul(a.total_bytes as u128);
            left.cmp(&right)
        })
}
