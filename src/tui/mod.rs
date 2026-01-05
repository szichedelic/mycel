use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame, Terminal,
};
use std::io::{self, Write};

use crate::bank::{self, BankedItem};
use crate::config::ProjectConfig;
use crate::db::{Database, Project, Session};
use crate::session::SessionManager;
use crate::worktree;

mod logo;

struct App {
    db: Database,
    session_manager: SessionManager,
    projects: Vec<ProjectWithSessions>,
    banked: Vec<(String, BankedItem)>,
    selected: usize,
    should_quit: bool,
    show_logo: bool,
}

struct ProjectWithSessions {
    project: Project,
    sessions: Vec<SessionWithStatus>,
    expanded: bool,
}

struct SessionWithStatus {
    session: Session,
    is_running: bool,
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
            selected: 0,
            should_quit: false,
            show_logo: true,
        };

        app.refresh()?;
        Ok(app)
    }

    fn refresh(&mut self) -> Result<()> {
        let projects = self.db.list_projects()?;
        self.projects = projects
            .into_iter()
            .map(|project| {
                let sessions = self.db.list_sessions(project.id).unwrap_or_default();
                let sessions_with_status: Vec<SessionWithStatus> = sessions
                    .into_iter()
                    .map(|session| {
                        let is_running = self
                            .session_manager
                            .is_alive(&session.tmux_session)
                            .unwrap_or(false);
                        SessionWithStatus { session, is_running }
                    })
                    .collect();

                ProjectWithSessions {
                    project,
                    sessions: sessions_with_status,
                    expanded: true,
                }
            })
            .collect();

        self.banked.clear();
        for project in &self.projects {
            if let Ok(items) = bank::list_banked(&project.project.name) {
                for item in items {
                    self.banked.push((project.project.name.clone(), item));
                }
            }
        }

        Ok(())
    }

    fn total_items(&self) -> usize {
        let project_items: usize = self.projects
            .iter()
            .map(|p| 1 + if p.expanded { p.sessions.len() } else { 0 })
            .sum();

        if self.banked.is_empty() {
            project_items
        } else {
            project_items + 2 + self.banked.len()
        }
    }

    fn get_selected_item(&self) -> Option<SelectedItem> {
        let mut idx = 0;
        for project in &self.projects {
            if idx == self.selected {
                return Some(SelectedItem::Project(project));
            }
            idx += 1;

            if project.expanded {
                for session in &project.sessions {
                    if idx == self.selected {
                        return Some(SelectedItem::Session(project, session));
                    }
                    idx += 1;
                }
            }
        }

        if !self.banked.is_empty() {
            idx += 2;
            for (project_name, item) in &self.banked {
                if idx == self.selected {
                    return Some(SelectedItem::Banked(project_name, item));
                }
                idx += 1;
            }
        }

        None
    }

    fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    fn move_down(&mut self) {
        let total = self.total_items();
        if total > 0 && self.selected < total - 1 {
            self.selected += 1;
        }
    }

    fn toggle_expand(&mut self) {
        let mut idx = 0;
        for project in &mut self.projects {
            if idx == self.selected {
                project.expanded = !project.expanded;
                return;
            }
            idx += 1;
            if project.expanded {
                idx += project.sessions.len();
            }
        }
    }
}

enum SelectedItem<'a> {
    Project(&'a ProjectWithSessions),
    Session(&'a ProjectWithSessions, &'a SessionWithStatus),
    Banked(&'a str, &'a BankedItem),
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
                    match key.code {
                        KeyCode::Char('q') => app.should_quit = true,
                        KeyCode::Char('j') | KeyCode::Down => app.move_down(),
                        KeyCode::Char('k') | KeyCode::Up => app.move_up(),
                        KeyCode::Enter | KeyCode::Char(' ') => app.toggle_expand(),
                        KeyCode::Char('a') => {
                            if let Some(SelectedItem::Session(_, session)) = app.get_selected_item()
                            {
                                // Restore terminal before attaching
                                disable_raw_mode()?;
                                execute!(
                                    terminal.backend_mut(),
                                    LeaveAlternateScreen,
                                    DisableMouseCapture
                                )?;

                                app.session_manager.attach(&session.session.tmux_session)?;

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
                                    let config = ProjectConfig::load(&project.path)?;
                                    let sanitized = worktree::sanitize_branch_name(name);

                                    let branch_exists = std::process::Command::new("git")
                                        .args(["show-ref", "--verify", "--quiet", &format!("refs/heads/{}", sanitized)])
                                        .current_dir(&project.path)
                                        .status()
                                        .map(|s| s.success())
                                        .unwrap_or(false);

                                    let (worktree_path, branch_name) = if branch_exists {
                                        println!("Using existing branch '{}'...", sanitized);
                                        worktree::create_from_existing(&project.path, &sanitized, &config)?
                                    } else {
                                        println!("Creating worktree '{}'...", sanitized);
                                        worktree::create(&project.path, name, &config)?
                                    };

                                    println!("Starting Claude session...");
                                    if !config.setup.is_empty() {
                                        println!("Setup: {}", config.setup.join(" && "));
                                    }
                                    let tmux_session = app.session_manager.create(&project.name, &branch_name, &worktree_path, &config.setup)?;
                                    app.db.add_session(project.id, &branch_name, &worktree_path, &tmux_session)?;
                                    println!("Session '{}' created.", branch_name);
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
                            if let Some(SelectedItem::Session(project, session)) = app.get_selected_item() {
                                let session_id = session.session.id;
                                let tmux_session = session.session.tmux_session.clone();
                                let worktree_path = session.session.worktree_path.clone();
                                let project_path = project.project.path.clone();

                                if app.session_manager.is_alive(&tmux_session)? {
                                    app.session_manager.kill(&tmux_session)?;
                                }

                                let _ = worktree::remove(&project_path, &worktree_path);

                                app.db.delete_session(session_id)?;
                                app.refresh()?;
                            }
                        }
                        KeyCode::Char('b') => {
                            if let Some(SelectedItem::Session(project, session)) = app.get_selected_item() {
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

                                let has_changes = status_output
                                    .map(|o| !o.stdout.is_empty())
                                    .unwrap_or(false);

                                if has_changes {
                                    println!("Error: Uncommitted changes in worktree.");
                                    println!("Commit your work first:");
                                    println!("  cd {}", worktree_path.display());
                                    println!("  git add -A && git commit -m \"your message\"");
                                    std::thread::sleep(std::time::Duration::from_millis(2000));
                                } else {
                                    let config = ProjectConfig::load(&project_path)?;
                                    let bundle_path = bank::bundle_path(&project_name, &session_name)?;

                                    if bundle_path.exists() {
                                        println!("Error: Bundle already exists: {}", bundle_path.display());
                                        std::thread::sleep(std::time::Duration::from_millis(1500));
                                    } else {
                                        println!("Banking '{}'...", session_name);
                                        if let Err(e) = bank::create_bundle(&project_path, &session_name, &config.base_branch, &bundle_path) {
                                            println!("Error creating bundle: {}", e);
                                            std::thread::sleep(std::time::Duration::from_millis(1500));
                                        } else {
                                            if app.session_manager.is_alive(&tmux_session)? {
                                                println!("Stopping session...");
                                                app.session_manager.kill(&tmux_session)?;
                                            }

                                            println!("Removing worktree...");
                                            let _ = worktree::remove(&project_path, &worktree_path);

                                            app.db.delete_session(session_id)?;

                                            let _ = std::process::Command::new("git")
                                                .args(["branch", "-D", &session_name])
                                                .current_dir(&project_path)
                                                .status();

                                            println!("Banked '{}'", session_name);
                                            std::thread::sleep(std::time::Duration::from_millis(500));
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

    let session_count: usize = app.projects.iter().map(|p| p.sessions.len()).sum();
    let running_count: usize = app.projects.iter()
        .flat_map(|p| &p.sessions)
        .filter(|s| s.is_running)
        .count();

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
            Span::styled("  M Y C E L", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled("  │  ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{} sessions", session_count), Style::default().fg(Color::White)),
            Span::styled("  ", Style::default()),
            Span::styled(format!("{} running", running_count), Style::default().fg(Color::Green)),
            if !app.banked.is_empty() {
                Span::styled(format!("  │  {} banked", app.banked.len()), Style::default().fg(Color::Magenta))
            } else {
                Span::styled("", Style::default())
            },
        ]),
    ];

    let header = Paragraph::new(header_lines)
        .block(Block::default().borders(Borders::BOTTOM));
    f.render_widget(header, chunks[0]);

    let mut items: Vec<ListItem> = Vec::new();
    let mut idx = 0;

    for project in &app.projects {
        let expand_icon = if project.expanded { "▼" } else { "▶" };
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
        idx += 1;

        if project.expanded {
            for (i, session) in project.sessions.iter().enumerate() {
                let is_last = i == project.sessions.len() - 1;
                let prefix = if is_last { "  └─" } else { "  ├─" };
                let status = if session.is_running {
                    Span::styled("●", Style::default().fg(Color::Green))
                } else {
                    Span::styled("○", Style::default().fg(Color::DarkGray))
                };

                let session_style = if idx == app.selected {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };

                let line = Line::from(vec![
                    Span::styled(format!("{} ", prefix), Style::default().fg(Color::DarkGray)),
                    Span::styled(&session.session.name, session_style),
                    Span::raw("  "),
                    status,
                    Span::styled(
                        if session.is_running { " running" } else { " stopped" },
                        Style::default().fg(Color::DarkGray),
                    ),
                ]);

                items.push(ListItem::new(line));
                idx += 1;
            }
        }
    }

    if !app.banked.is_empty() {
        items.push(ListItem::new(Line::from("")));
        items.push(ListItem::new(Line::from(Span::styled(
            "📦 BANKED",
            Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
        ))));
        idx += 2;

        for (project_name, item) in &app.banked {
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
                Span::styled(format!("  ({}) ", project_name), Style::default().fg(Color::DarkGray)),
                Span::styled(item.size_human(), Style::default().fg(Color::DarkGray)),
            ]);

            items.push(ListItem::new(line));
            idx += 1;
        }
    }

    if items.is_empty() {
        let empty_msg = Paragraph::new("No projects registered. Run 'mycel init' in a git repository.")
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::ALL).title("Sessions"));
        f.render_widget(empty_msg, chunks[1]);
    } else {
        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Sessions"))
            .highlight_style(Style::default().add_modifier(Modifier::BOLD));
        f.render_widget(list, chunks[1]);
    }

    let footer = Paragraph::new(" [a]ttach  [s]pawn  [b]ank  [u]nbank  [x] kill  [r]efresh  [q]uit")
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().borders(Borders::TOP));
    f.render_widget(footer, chunks[2]);
}
