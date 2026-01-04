use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

const LOGO: &str = r#"
              ░░░▒▒▒▓▓███▓▓▒▒▒░░░
           ░▒▓█▀▀             ▀▀█▓▒░
         ░▓█▀   ·    ·    ·      ▀█▓░
        ▒█▀  ·    ╲  │  ╱    ·     ▀█▒
       ▓█   ·   ·──╲─┼─╱──·   ·     █▓
      ▓█      ╲     ╲│╱     ╱        █▓
      █▓  · ───●─────●─────●─── ·    ▓█
      ▓█      ╱     ╱│╲     ╲        █▓
       ▓█   ·   ·──╱─┼─╲──·   ·     █▓
        ▒█▀  ·    ╱  │  ╲    ·     ▀█▒
         ░▓█▄   ·    ·    ·      ▄█▓░
           ░▒▓█▄▄             ▄▄█▓▒░
              ░░░▒▒▒▓▓███▓▓▒▒▒░░░
"#;

const NAME: &str = r#"
   ███╗   ███╗██╗   ██╗ ██████╗███████╗██╗
   ████╗ ████║╚██╗ ██╔╝██╔════╝██╔════╝██║
   ██╔████╔██║ ╚████╔╝ ██║     █████╗  ██║
   ██║╚██╔╝██║  ╚██╔╝  ██║     ██╔══╝  ██║
   ██║ ╚═╝ ██║   ██║   ╚██████╗███████╗███████╗
   ╚═╝     ╚═╝   ╚═╝    ╚═════╝╚══════╝╚══════╝
"#;

const TAGLINE: &str = "the network beneath your code";

pub fn draw(f: &mut Frame) {
    let area = f.area();

    // Center everything vertically
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Min(20),
            Constraint::Percentage(20),
        ])
        .split(area);

    let content_area = chunks[1];

    // Create gradient colors for the logo
    let gradient_colors = [
        Color::Rgb(0, 50, 80),    // Deep blue
        Color::Rgb(0, 80, 100),   // Teal
        Color::Rgb(0, 120, 120),  // Cyan
        Color::Rgb(0, 150, 130),  // Aqua
        Color::Rgb(50, 180, 140), // Soft green
    ];

    // Render logo with gradient
    let logo_lines: Vec<Line> = LOGO
        .lines()
        .enumerate()
        .map(|(i, line)| {
            let color_idx = (i * gradient_colors.len()) / LOGO.lines().count().max(1);
            let color = gradient_colors[color_idx.min(gradient_colors.len() - 1)];
            Line::from(Span::styled(line, Style::default().fg(color)))
        })
        .collect();

    let logo_height = logo_lines.len() as u16;
    let name_height = NAME.lines().count() as u16;
    let total_height = logo_height + name_height + 3;

    let inner_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(logo_height),
            Constraint::Length(1),
            Constraint::Length(name_height),
            Constraint::Length(2),
            Constraint::Length(1),
        ])
        .split(centered_rect(60, total_height, content_area));

    let logo_paragraph = Paragraph::new(logo_lines).alignment(Alignment::Center);
    f.render_widget(logo_paragraph, inner_chunks[0]);

    // Render name with cyan color
    let name_lines: Vec<Line> = NAME
        .lines()
        .map(|line| Line::from(Span::styled(line, Style::default().fg(Color::Cyan))))
        .collect();

    let name_paragraph = Paragraph::new(name_lines).alignment(Alignment::Center);
    f.render_widget(name_paragraph, inner_chunks[2]);

    // Render tagline
    let tagline = Paragraph::new(TAGLINE)
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    f.render_widget(tagline, inner_chunks[4]);
}

fn centered_rect(percent_x: u16, height: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length((r.height.saturating_sub(height)) / 2),
            Constraint::Length(height),
            Constraint::Min(0),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
