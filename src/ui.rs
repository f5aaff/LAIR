use crate::app::{App, CurrentScreen};
use crossterm::event::KeyModifiers;
use ratatui::Terminal;
use ratatui::crossterm::cursor;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};
use std::io::{self, Error, Write};
use std::path::PathBuf;
use std::fs;
use std::process::Command;

/// Launch editor to edit a file, then return to the TUI
/// This function temporarily restores the terminal to normal mode,
/// launches the editor, then restores the TUI state
fn launch_editor(file_path: &std::path::Path, editor: &str) -> io::Result<()> {
    let mut stdout = io::stdout();

    // Temporarily leave alternate screen and restore terminal
    terminal::disable_raw_mode()?;
    execute!(stdout, terminal::LeaveAlternateScreen, cursor::Show)?;
    stdout.flush()?;
    
    // Launch editor
    let _status = Command::new(editor).arg(file_path).status()?;

    // Re-enter alternate screen and raw mode
    terminal::enable_raw_mode()?;
    execute!(stdout, terminal::EnterAlternateScreen, cursor::Hide)?;
    stdout.flush()?;

    Ok(())
}

/// Create a new note file with date-based organization
/// Returns the full path to the created note file
/// If target_dir is provided, creates the note in that directory instead of date-based folder
fn create_note_file(
    notes_dir: &str,
    note_name: Option<&str>,
    file_format: &str,
    target_dir: Option<&PathBuf>,
) -> io::Result<PathBuf> {
    let now = chrono::Utc::now();
    
    // Determine the target directory
    let date_dir = if let Some(target) = target_dir {
        // Use provided target directory
        target.clone()
    } else {
        // Use date-based folder structure (YY-MM-DD)
        let base_dir = PathBuf::from(notes_dir);
        let date_folder = now.format("%y-%m-%d").to_string();
        base_dir.join(&date_folder)
    };
    
    // Ensure the date directory exists
    fs::create_dir_all(&date_dir)?;
    
    // Determine the file name
    let file_name = if let Some(name) = note_name {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            // Empty name, use timestamp
            format!("notes-{}.{}", now.format("%y-%m-%d_%H-%M-%S"), file_format)
        } else {
            // Use provided name, ensure it has the correct extension
            if trimmed.ends_with(&format!(".{}", file_format)) {
                trimmed.to_string()
            } else {
                format!("{}.{}", trimmed, file_format)
            }
        }
    } else {
        // No name provided, use timestamp
        format!("notes-{}.{}", now.format("%y-%m-%d_%H-%M-%S"), file_format)
    };
    
    let file_path = date_dir.join(&file_name);
    
    // Create empty file if it doesn't exist
    if !file_path.exists() {
        fs::File::create(&file_path)?;
    }
    
    Ok(file_path)
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
pub fn ui(f: &mut Frame, app: &mut App) {
    match app.current_screen {
        CurrentScreen::Main => render_main_screen(f, app),
        CurrentScreen::Browsing => render_browsing_screen(f, app),
        CurrentScreen::Editing => render_editing_screen(f, app),
        CurrentScreen::CreatingFolder => render_creating_folder_screen(f, app),
        CurrentScreen::Settings => render_settings_screen(f, app),
        CurrentScreen::Exiting => render_exiting_screen(f, app),
    }
}

/// Main screen - shows welcome message and options
fn render_main_screen(f: &mut Frame, _app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(0),    // Main content
            Constraint::Length(3), // Footer/help
        ])
        .split(f.area());

    // Header
    let header = Paragraph::new("LAIR - Note Management")
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
        Line::from("(S) Settings"),
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
fn render_browsing_screen(f: &mut Frame, app: &mut App) {
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

    // Note list
    let notes: Vec<ListItem> = app
        .browse_items
        .iter()
        .map(|(text, _)| ListItem::new(text.as_str()))
        .collect();
    let list = List::new(notes)
        .block(Block::default().borders(Borders::ALL).title("Notes"))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );
    f.render_stateful_widget(list, chunks[1], &mut app.browse_list_state);

    // Footer
    let help_text = "↑↓ Navigate | Space/→: Expand/Collapse | Enter: Open | N: New Note | F: New Folder | Esc: Back | Q: Quit";
    let footer = Paragraph::new(help_text)
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[2]);
}

/// New Note screen - shows popup dialog for entering note name
fn render_editing_screen(f: &mut Frame, app: &mut App) {
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

/// New Folder screen - shows popup dialog for entering folder name
fn render_creating_folder_screen(f: &mut Frame, app: &mut App) {
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
    let title = Paragraph::new("New Folder")
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
    let input_display = if app.folder_name_input.is_empty() {
        "Enter folder name (empty for timestamp)...".to_string()
    } else {
        format!("{}_", app.folder_name_input)
    };
    let input_style = if app.folder_name_input.is_empty() {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default().fg(Color::White)
    };
    let input = Paragraph::new(input_display)
        .style(input_style)
        .block(Block::default().borders(Borders::ALL).title("Folder Name"));
    f.render_widget(input, popup_chunks[1]);

    // Help text
    let help_text = "Enter: Create Folder | Esc: Cancel";
    let footer = Paragraph::new(help_text)
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, popup_chunks[2]);
}

fn render_settings_screen(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(0),    // Settings fields
            Constraint::Length(3), // Footer
        ])
        .split(f.area());

    // Header
    let header = Paragraph::new("Settings")
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(header, chunks[0]);

    // Settings fields area
    let settings_area = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // Notes Directory
            Constraint::Length(5), // Editor
            Constraint::Length(5), // File Format
        ])
        .split(chunks[1]);

    // Helper function to render a settings field
    let render_field = |f: &mut Frame, area: Rect, label: &str, value: &str, is_active: bool| {
        let field_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(20), // Label
                Constraint::Min(0),     // Value
            ])
            .split(area);

        // Label
        let label_style = if is_active {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        let label_text = Paragraph::new(label)
            .style(label_style)
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(label_text, field_chunks[0]);

        // Value input field
        let value_display = if value.is_empty() {
            format!("{}_", "Enter value...")
        } else {
            format!("{}_", value)
        };
        let value_style = if is_active {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        let value_text = Paragraph::new(value_display)
            .style(value_style)
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(value_text, field_chunks[1]);
    };

    // Notes Directory field
    let is_active = app.active_settings_field == Some(crate::app::SettingsField::NotesDirectory);
    render_field(
        f,
        settings_area[0],
        "Notes Directory:",
        &app.settings_field_inputs[0],
        is_active,
    );

    // Editor field
    let is_active = app.active_settings_field == Some(crate::app::SettingsField::Editor);
    render_field(
        f,
        settings_area[1],
        "Editor:",
        &app.settings_field_inputs[1],
        is_active,
    );

    // File Format field
    let is_active = app.active_settings_field == Some(crate::app::SettingsField::FileFormat);
    render_field(
        f,
        settings_area[2],
        "File Format:",
        &app.settings_field_inputs[2],
        is_active,
    );

    // Footer
    let help_text = if app.active_settings_field.is_some() {
        "Type to edit | Enter: Save | Esc: Cancel/Back"
    } else {
        "↑↓ Navigate | Enter: Edit | S: Save | Esc: Back"
    };
    let footer = Paragraph::new(help_text)
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[2]);
}

/// Exiting screen - confirmation dialog
fn render_exiting_screen(f: &mut Frame, _app: &mut App) {
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
            .draw(|f| ui(f, app))
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
                        app.load_browse_items();
                        app.current_screen = CurrentScreen::Browsing;
                    }
                    KeyCode::Char('s') | KeyCode::Char('S') => {
                        app.current_screen = CurrentScreen::Settings;
                        app.reset_settings_inputs(); // Reset to current saved values
                        app.active_settings_field = None;
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
                        KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('K') => {
                            app.browse_up();
                        }
                        KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('J') => {
                            app.browse_down();
                        }
                        KeyCode::Enter => {
                            // Open the selected file
                            if let Some(file_path) = app.get_selected_file_path() {
                                if let Err(_e) = launch_editor(file_path, &app.settings.editor) {
                                    // Error launching editor - continue in TUI
                                }
                                app.current_file = Some(file_path.to_string_lossy().to_string());
                            }
                        }
                        KeyCode::Char(' ') | KeyCode::Right => {
                            // Toggle expand/collapse of selected folder
                            app.toggle_folder_expansion();
                        }
                        KeyCode::Char('n') | KeyCode::Char('N') => {
                            // Create new note in selected directory
                            app.target_directory = Some(app.get_selected_directory());
                            app.note_name_input.clear();
                            app.current_screen = CurrentScreen::Editing;
                        }
                        KeyCode::Char('f') | KeyCode::Char('F') => {
                            // Create new folder - go to folder creation screen
                            app.target_directory = Some(app.get_selected_directory());
                            app.folder_name_input.clear();
                            app.current_screen = CurrentScreen::CreatingFolder;
                        }
                        _ => {}
                    }
                }
                CurrentScreen::Editing => {
                    match key.code {
                        KeyCode::Enter => {
                            // Create note and launch editor
                            let note_name = if app.note_name_input.trim().is_empty() {
                                None
                            } else {
                                Some(app.note_name_input.as_str())
                            };
                            
                            match create_note_file(
                                &app.settings.notes_directory,
                                note_name,
                                &app.settings.default_file_format,
                                app.target_directory.as_ref(),
                            ) {
                                Ok(file_path) => {
                                    // Launch editor with the new note
                                    if let Err(_e) = launch_editor(&file_path, &app.settings.editor) {
                                        // Error launching editor - continue in TUI
                                    }

                                    // Return to appropriate screen after editor exits
                                    if app.target_directory.is_some() {
                                        // Came from browse screen, return there
                                        app.current_screen = CurrentScreen::Browsing;
                                        app.load_browse_items(); // Reload to show new note
                                    } else {
                                        // Came from main screen
                                        app.current_screen = CurrentScreen::Main;
                                    }
                                    app.note_name_input.clear();
                                    app.target_directory = None;
                                    app.current_file = Some(file_path.to_string_lossy().to_string());
                                }
                                Err(e) => {
                                    eprintln!("Error creating note file: {}", e);
                                    // Stay in editing screen on error
                                }
                            }
                        }
                        KeyCode::Backspace => {
                            // Remove last character
                            app.note_name_input.pop();
                        }
                        KeyCode::Esc => {
                            // Cancel and return to previous screen
                            if app.target_directory.is_some() {
                                app.current_screen = CurrentScreen::Browsing;
                            } else {
                                app.current_screen = CurrentScreen::Main;
                            }
                            app.note_name_input.clear();
                            app.target_directory = None;
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
                CurrentScreen::CreatingFolder => {
                    match key.code {
                        KeyCode::Enter => {
                            // Create folder
                            if let Err(e) = app.create_new_folder() {
                                eprintln!("Error creating folder: {}", e);
                            } else {
                                // Return to browse screen
                                app.current_screen = CurrentScreen::Browsing;
                            }
                        }
                        KeyCode::Backspace => {
                            // Remove last character
                            app.folder_name_input.pop();
                        }
                        KeyCode::Esc => {
                            // Cancel and return to browse screen
                            app.current_screen = CurrentScreen::Browsing;
                            app.folder_name_input.clear();
                            app.target_directory = None;
                        }
                        KeyCode::Char(c) => {
                            // Add character to input (allow alphanumeric, spaces, dashes, underscores, dots)
                            if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' || c == '.' {
                                app.folder_name_input.push(c);
                            }
                        }
                        _ => {}
                    }
                }
                CurrentScreen::Settings => {
                    match key.code {
                        KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('K') => {
                            // Navigate up through fields
                            app.active_settings_field = match app.active_settings_field {
                                None => Some(crate::app::SettingsField::NotesDirectory),
                                Some(crate::app::SettingsField::NotesDirectory) => {
                                    Some(crate::app::SettingsField::NotesDirectory)
                                }
                                Some(crate::app::SettingsField::Editor) => {
                                    Some(crate::app::SettingsField::NotesDirectory)
                                }
                                Some(crate::app::SettingsField::FileFormat) => {
                                    Some(crate::app::SettingsField::Editor)
                                }
                            };
                        }
                        KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('J') => {
                            // Navigate down through fields
                            app.active_settings_field = match app.active_settings_field {
                                None => Some(crate::app::SettingsField::NotesDirectory),
                                Some(crate::app::SettingsField::NotesDirectory) => {
                                    Some(crate::app::SettingsField::Editor)
                                }
                                Some(crate::app::SettingsField::Editor) => {
                                    Some(crate::app::SettingsField::FileFormat)
                                }
                                Some(crate::app::SettingsField::FileFormat) => {
                                    Some(crate::app::SettingsField::FileFormat)
                                }
                            };
                        }
                        KeyCode::Enter => {
                            // Start editing if no field is active, or save if editing
                            if app.active_settings_field.is_none() {
                                app.active_settings_field =
                                    Some(crate::app::SettingsField::NotesDirectory);
                            } else {
                                // Save settings and exit edit mode
                                if let Err(e) = app.save_settings() {
                                    eprintln!("Error saving settings: {}", e);
                                }
                                app.active_settings_field = None;
                            }
                        }
                        KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            // Save settings
                            if let Err(e) = app.save_settings() {
                                eprintln!("Error saving settings: {}", e);
                            }
                            app.active_settings_field = None;
                        }
                        KeyCode::Esc => {
                            if app.active_settings_field.is_some() {
                                // Cancel editing - reset to saved values
                                app.reset_settings_inputs();
                                app.active_settings_field = None;
                            } else {
                                // Exit settings screen
                                app.current_screen = CurrentScreen::Main;
                            }
                        }
                        KeyCode::Backspace => {
                            // Handle backspace when editing
                            if let Some(field) = app.active_settings_field {
                                let idx = match field {
                                    crate::app::SettingsField::NotesDirectory => 0,
                                    crate::app::SettingsField::Editor => 1,
                                    crate::app::SettingsField::FileFormat => 2,
                                };
                                app.settings_field_inputs[idx].pop();
                            }
                        }
                        KeyCode::Char(c) => {
                            // Add character when editing
                            if let Some(field) = app.active_settings_field {
                                let idx = match field {
                                    crate::app::SettingsField::NotesDirectory => 0,
                                    crate::app::SettingsField::Editor => 1,
                                    crate::app::SettingsField::FileFormat => 2,
                                };
                                // Allow most characters for paths and editor names
                                // For file format, only allow alphanumeric
                                match field {
                                    crate::app::SettingsField::FileFormat => {
                                        if c.is_alphanumeric() {
                                            app.settings_field_inputs[idx].push(c);
                                        }
                                    }
                                    _ => {
                                        // Allow most characters for paths and editor
                                        if !c.is_control() {
                                            app.settings_field_inputs[idx].push(c);
                                        }
                                    }
                                }
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
