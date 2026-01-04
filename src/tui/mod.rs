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
use std::io;

use crate::db::{Database, Project, Session};
use crate::session::SessionManager;

mod logo;

struct App {
    db: Database,
    session_manager: SessionManager,
    projects: Vec<ProjectWithSessions>,
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

        Ok(())
    }

    fn total_items(&self) -> usize {
        self.projects
            .iter()
            .map(|p| 1 + if p.expanded { p.sessions.len() } else { 0 })
            .sum()
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
}

pub async fn run() -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new()?;

    // Show logo briefly
    terminal.draw(|f| logo::draw(f))?;
    std::thread::sleep(std::time::Duration::from_millis(1500));
    app.show_logo = false;

    // Main loop
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
                                app.refresh()?;
                            }
                        }
                        KeyCode::Char('r') => app.refresh()?,
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
            Constraint::Length(3), // Header
            Constraint::Min(0),    // Main content
            Constraint::Length(3), // Footer
        ])
        .split(f.area());

    // Header
    let session_count: usize = app.projects.iter().map(|p| p.sessions.len()).sum();
    let header = Paragraph::new(format!("  mycel                              {} sessions", session_count))
        .style(Style::default().fg(Color::Cyan))
        .block(Block::default().borders(Borders::BOTTOM));
    f.render_widget(header, chunks[0]);

    // Main content - project/session list
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

    // Footer
    let footer = Paragraph::new(" [a]ttach  [k]ill  [s]pawn  [r]efresh  [q]uit")
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().borders(Borders::TOP));
    f.render_widget(footer, chunks[2]);
}
