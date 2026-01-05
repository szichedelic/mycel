use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use std::time::{Duration, Instant};

const LOGO_FRAMES: [&str; 5] = [
    // Frame 0 - center only
    r#"






            ●






"#,
    // Frame 1 - small network
    r#"




           ╲│╱
            ●
           ╱│╲






"#,
    // Frame 2 - growing
    r#"



         ╲  │  ╱
        ──╲─┼─╱──
       ───●─●─●───
        ──╱─┼─╲──
         ╱  │  ╲





"#,
    // Frame 3 - larger
    r#"

              ░░░▒▒▒▓▓▒▒▒░░░
           ░▒▓█▀           ▀█▓▒░
         ░▓█▀  ╲  │  ╱        ▀█▓░
        ▒█▀  ·──╲─┼─╱──·        ▀█▒
       ▓█   ───●─────●───        █▓
        ▒█▀  ·──╱─┼─╲──·        ▀█▒
         ░▓█▄  ╱  │  ╲        ▄█▓░
           ░▒▓█▄▄           ▄▄█▓▒░
              ░░░▒▒▒▓▓▒▒▒░░░



"#,
    // Frame 4 - full
    r#"
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
"#,
];

const NAME: &str = r#"
   ███╗   ███╗██╗   ██╗ ██████╗███████╗██╗
   ████╗ ████║╚██╗ ██╔╝██╔════╝██╔════╝██║
   ██╔████╔██║ ╚████╔╝ ██║     █████╗  ██║
   ██║╚██╔╝██║  ╚██╔╝  ██║     ██╔══╝  ██║
   ██║ ╚═╝ ██║   ██║   ╚██████╗███████╗███████╗
   ╚═╝     ╚═╝   ╚═╝    ╚═════╝╚══════╝╚══════╝
"#;

pub fn draw_animated(f: &mut Frame, frame_idx: usize) {
    let area = f.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(15),
            Constraint::Min(20),
            Constraint::Percentage(15),
        ])
        .split(area);

    let content_area = chunks[1];

    let gradient_colors = [
        Color::Rgb(0, 50, 80),
        Color::Rgb(0, 80, 100),
        Color::Rgb(0, 120, 120),
        Color::Rgb(0, 150, 130),
        Color::Rgb(50, 180, 140),
    ];

    let logo = LOGO_FRAMES[frame_idx.min(LOGO_FRAMES.len() - 1)];

    let logo_lines: Vec<Line> = logo
        .lines()
        .enumerate()
        .map(|(i, line)| {
            let color_idx = (i * gradient_colors.len()) / logo.lines().count().max(1);
            let color = gradient_colors[color_idx.min(gradient_colors.len() - 1)];
            Line::from(Span::styled(line, Style::default().fg(color)))
        })
        .collect();

    let logo_height = logo_lines.len() as u16;
    let name_height = NAME.lines().count() as u16;
    let total_height = logo_height + name_height + 2;

    let inner_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(logo_height),
            Constraint::Length(1),
            Constraint::Length(name_height),
        ])
        .split(centered_rect(60, total_height, content_area));

    let logo_paragraph = Paragraph::new(logo_lines).alignment(Alignment::Center);
    f.render_widget(logo_paragraph, inner_chunks[0]);

    // Only show name on last frames
    if frame_idx >= 3 {
        let name_lines: Vec<Line> = NAME
            .lines()
            .map(|line| Line::from(Span::styled(line, Style::default().fg(Color::Cyan))))
            .collect();

        let name_paragraph = Paragraph::new(name_lines).alignment(Alignment::Center);
        f.render_widget(name_paragraph, inner_chunks[2]);
    }
}

pub fn draw(f: &mut Frame) {
    draw_animated(f, LOGO_FRAMES.len() - 1);
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
