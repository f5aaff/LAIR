use crate::app::{App, CurrentScreen};
use ratatui::Terminal;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::crossterm::cursor;
use ratatui::crossterm::terminal;
use ratatui::crossterm::execute;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};
use std::io::{self, Error, Write};
use std::process::Command;

/// Launch nvim to edit a file, then return to the TUI
/// This function temporarily restores the terminal to normal mode,
/// launches nvim, then restores the TUI state
fn launch_nvim_editor(file_path: &str) -> io::Result<()> {
    let mut stdout = io::stdout();
    
    // Temporarily leave alternate screen and restore terminal
    terminal::disable_raw_mode()?;
    execute!(
        stdout,
        terminal::LeaveAlternateScreen,
        cursor::Show
    )?;
    stdout.flush()?;

    // Launch nvim
    let _status = Command::new("nvim")
        .arg(file_path)
        .status()?;

    // Re-enter alternate screen and raw mode
    terminal::enable_raw_mode()?;
    execute!(
        stdout,
        terminal::EnterAlternateScreen,
        cursor::Hide
    )?;
    stdout.flush()?;

    Ok(())
}

/// Helper function to create a centered rect using up certain percentage of the available rect `r`
pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
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

/// Main UI function that dispatches to screen-specific renderers
pub fn ui(f: &mut Frame, app: &App) {
    match app.current_screen {
        CurrentScreen::Main => render_main_screen(f, app),
        CurrentScreen::Browsing => render_browsing_screen(f, app),
        CurrentScreen::Editing => render_editing_screen(f, app),
        CurrentScreen::Exiting => render_exiting_screen(f, app),
    }
}

/// Main screen - shows welcome message and options
fn render_main_screen(f: &mut Frame, _app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(0),    // Main content
            Constraint::Length(3), // Footer/help
        ])
        .split(f.area());

    // Header
    let header = Paragraph::new("Escritoire - Note Management")
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(header, chunks[0]);

    // Main content area - centered options
    let main_area = centered_rect(60, 40, chunks[1]);
    let options = vec![
        Line::from("(N) New Note"),
        Line::from("(B) Browse Notes"),
        Line::from("(Q) Quit"),
    ];
    let content = Paragraph::new(options)
        .style(Style::default().fg(Color::White))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).title("Options"));
    f.render_widget(content, main_area);

    // Footer with help text
    let help_text = "Press 'N' for new note, 'B' to browse, 'Q' to quit";
    let footer = Paragraph::new(help_text)
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[2]);
}

/// Browsing screen - shows list of notes
fn render_browsing_screen(f: &mut Frame, _app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(0),    // Note list
            Constraint::Length(3), // Footer
        ])
        .split(f.area());

    // Header
    let header = Paragraph::new("Browse Notes")
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(header, chunks[0]);

    // Note list (placeholder - you'll populate this with actual notes)
    let notes: Vec<ListItem> = vec![
        ListItem::new("Note 1.md"),
        ListItem::new("Note 2.md"),
        ListItem::new("Note 3.md"),
    ];
    let list = List::new(notes)
        .block(Block::default().borders(Borders::ALL).title("Notes"))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );
    f.render_widget(list, chunks[1]);

    // Footer
    let help_text = "↑↓ Navigate | Enter: Open | Esc: Back | Q: Quit";
    let footer = Paragraph::new(help_text)
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[2]);
}

/// New Note screen - shows popup dialog for entering note name
fn render_editing_screen(f: &mut Frame, app: &App) {
    // Create a centered popup dialog
    let popup_area = centered_rect(60, 30, f.area());
    
    // Split the popup into sections
    let popup_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Length(5), // Input field
            Constraint::Length(3), // Help text
        ])
        .split(popup_area);

    // Title
    let title = Paragraph::new("New Note")
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(Clear, popup_area); // Clear the area first
    f.render_widget(title, popup_chunks[0]);

    // Input field - show the current input with a cursor indicator
    let input_display = if app.note_name_input.is_empty() {
        "Enter note name...".to_string()
    } else {
        format!("{}_", app.note_name_input)
    };
    let input_style = if app.note_name_input.is_empty() {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default().fg(Color::White)
    };
    let input = Paragraph::new(input_display)
        .style(input_style)
        .block(Block::default().borders(Borders::ALL).title("Note Name"));
    f.render_widget(input, popup_chunks[1]);

    // Help text
    let help_text = "Enter: Create & Edit | Esc: Cancel";
    let footer = Paragraph::new(help_text)
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, popup_chunks[2]);
}

/// Exiting screen - confirmation dialog
fn render_exiting_screen(f: &mut Frame, _app: &App) {
    // Render the previous screen in the background (optional)
    // For now, just show the exit confirmation

    let area = centered_rect(50, 25, f.area());

    let exit_text = vec![
        Line::from(""),
        Line::from("Are you sure you want to exit?"),
        Line::from(""),
        Line::from("(Y) Yes"),
        Line::from("(N) No"),
    ];

    let exit_dialog = Paragraph::new(exit_text)
        .style(Style::default().fg(Color::White))
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Exit")
                .border_style(Style::default().fg(Color::Red)),
        );

    f.render_widget(Clear, area); // Clear the area first
    f.render_widget(exit_dialog, area);
}

/// Main event loop function
pub fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> io::Result<bool> {
    loop {
        terminal
            .draw(|f| ui(f, &*app))
            .map_err(|e| Error::other(format!("{}", e)))?;

        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind == KeyEventKind::Press {
            match app.current_screen {
                CurrentScreen::Main => match key.code {
                    KeyCode::Char('q') | KeyCode::Char('Q') => {
                        app.current_screen = CurrentScreen::Exiting;
                    }
                    KeyCode::Char('n') | KeyCode::Char('N') => {
                        app.current_screen = CurrentScreen::Editing;
                        app.note_name_input.clear(); // Clear input when entering
                    }
                    KeyCode::Char('b') | KeyCode::Char('B') => {
                        app.current_screen = CurrentScreen::Browsing;
                    }
                    _ => {}
                },
                CurrentScreen::Browsing => {
                    match key.code {
                        KeyCode::Esc => {
                            app.current_screen = CurrentScreen::Main;
                        }
                        KeyCode::Char('q') | KeyCode::Char('Q') => {
                            app.current_screen = CurrentScreen::Exiting;
                        }
                        // TODO: Add navigation and selection logic
                        _ => {}
                    }
                }
                CurrentScreen::Editing => {
                    match key.code {
                        KeyCode::Enter => {
                            // Create note and launch nvim
                            if !app.note_name_input.is_empty() {
                                let note_name = app.note_name_input.trim();
                                if !note_name.is_empty() {
                                    // Ensure it has .md extension if not present
                                    let file_name = if note_name.ends_with(".md") {
                                        note_name.to_string()
                                    } else {
                                        format!("{}.md", note_name)
                                    };
                                    
                                    // Launch nvim with the new note
                                    if let Err(_e) = launch_nvim_editor(&file_name) {
                                        // Error launching nvim - continue in TUI
                                    }
                                    
                                    // Return to main screen after nvim exits
                                    app.current_screen = CurrentScreen::Main;
                                    app.note_name_input.clear();
                                    app.current_file = None;
                                }
                            }
                        }
                        KeyCode::Backspace => {
                            // Remove last character
                            app.note_name_input.pop();
                        }
                        KeyCode::Esc => {
                            // Cancel and return to main
                            app.current_screen = CurrentScreen::Main;
                            app.note_name_input.clear();
                            app.current_file = None;
                        }
                        KeyCode::Char(c) => {
                            // Add character to input (allow alphanumeric, spaces, dashes, underscores, dots)
                            if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' || c == '.' {
                                app.note_name_input.push(c);
                            }
                        }
                        _ => {}
                    }
                }
                CurrentScreen::Exiting => match key.code {
                    KeyCode::Char('y') | KeyCode::Char('Y') => {
                        return Ok(false);
                    }
                    KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                        app.current_screen = CurrentScreen::Main;
                    }
                    _ => {}
                },
            }
        }
    }
}
