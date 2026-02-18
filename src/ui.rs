use crate::app::{App, CurrentScreen, SearchMode};
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
use std::fs;
use std::io::{self, Error, Write};
use std::path::PathBuf;
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

    // Clear any residual output from the editor
    execute!(stdout, terminal::Clear(terminal::ClearType::All))?;
    stdout.flush()?;

    Ok(())
}

/// Create a new note file
/// Returns the full path to the created note file
/// If note_name contains a path (e.g., "x/y/note.md"), creates directories as needed
/// Default is to create the note in the root notes directory
fn create_note_file(
    notes_dir: &str,
    note_name: Option<&str>,
    file_format: &str,
    target_dir: Option<&PathBuf>,
) -> io::Result<PathBuf> {
    let now = chrono::Utc::now();
    let base_dir = PathBuf::from(notes_dir);

    // Determine the target directory - default to root if no target_dir provided
    let target_directory = if let Some(target) = target_dir {
        target.clone()
    } else {
        // Default to root directory
        base_dir.clone()
    };

    // Determine the file name and path
    let (file_name, dir_path) = if let Some(name) = note_name {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            // Empty name, use timestamp in target directory
            let file_name = format!("notes-{}.{}", now.format("%y-%m-%d_%H-%M-%S"), file_format);
            (file_name, target_directory)
        } else {
            // Check if the name contains a path separator
            if trimmed.contains('/') {
                // Split path and filename
                let path = PathBuf::from(trimmed);
                let file_name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| {
                        format!("notes-{}.{}", now.format("%y-%m-%d_%H-%M-%S"), file_format)
                    });

                // Ensure file has correct extension
                let file_name = if file_name.ends_with(&format!(".{}", file_format)) {
                    file_name
                } else {
                    format!("{}.{}", file_name, file_format)
                };

                // Get directory part (parent of the file in the path)
                let dir_part = path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
                let dir_path = base_dir.join(&dir_part);

                (file_name, dir_path)
            } else {
                // Simple filename, use target directory
                let file_name = if trimmed.ends_with(&format!(".{}", file_format)) {
                    trimmed.to_string()
                } else {
                    format!("{}.{}", trimmed, file_format)
                };
                (file_name, target_directory)
            }
        }
    } else {
        // No name provided, use timestamp in target directory
        let file_name = format!("notes-{}.{}", now.format("%y-%m-%d_%H-%M-%S"), file_format);
        (file_name, target_directory)
    };

    // Ensure the directory exists
    fs::create_dir_all(&dir_path)?;

    let file_path = dir_path.join(&file_name);

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

/// Create a search title widget
fn create_search_title(title: &str) -> Paragraph<'static> {
    Paragraph::new(title.to_string())
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL))
}

/// Create a search input widget
fn create_search_input(input: &str, placeholder: &str) -> Paragraph<'static> {
    let display = if input.is_empty() {
        placeholder.to_string()
    } else {
        format!("{}_", input)
    };
    let style = if input.is_empty() {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default().fg(Color::White)
    };
    Paragraph::new(display)
        .style(style)
        .block(Block::default().borders(Borders::ALL).title("Query"))
        .wrap(ratatui::widgets::Wrap { trim: false })
}

/// Generate preview text for live grep matches
fn generate_grep_preview(
    selected_idx: Option<usize>,
    filtered_paths: &[Option<PathBuf>],
    grep_matches: &[crate::search::MatchInfo],
) -> String {
    let idx = match selected_idx {
        Some(i) => i,
        None => return "Select a file to see matches".to_string(),
    };

    let selected_path = match filtered_paths.get(idx).and_then(|p| p.as_ref()) {
        Some(p) => p,
        None => return "Select a file to see matches".to_string(),
    };

    let file_matches: Vec<&crate::search::MatchInfo> = grep_matches
        .iter()
        .filter(|m| &m.file_path == selected_path)
        .collect();

    if file_matches.is_empty() {
        return "No matches found".to_string();
    }

    file_matches
        .iter()
        .take(10)
        .map(|m| {
            let line = if m.line_content.len() > 80 {
                format!("{}...", &m.line_content[..77])
            } else {
                m.line_content.clone()
            };
            format!("Line {}: {}", m.line_number, line)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Start a search with the given mode
fn start_search(app: &mut App, mode: SearchMode) {
    app.is_searching = true;
    app.search_input.clear();
    app.apply_search_filter(mode);
}

/// Handle search input events
fn handle_search_input(key: KeyCode, app: &mut App) {
    match key {
        KeyCode::Esc => {
            // Cancel search
            app.is_searching = false;
            app.search_input.clear();
            app.filtered_browse_items.clear();
            app.filtered_browse_paths.clear();
            app.grep_matches.clear();
            app.load_browse_items();
        }
        KeyCode::Enter => {
            // In live grep, Enter opens the selected file; in fuzzy search, it exits search mode
            if app.search_mode == SearchMode::LiveGrep {
                if let Some(file_path) = app.get_selected_file_path() {
                    let _ = launch_editor(file_path, &app.settings.editor);
                    app.current_file = Some(file_path.to_string_lossy().to_string());
                    app.load_browse_items();
                }
            } else {
                app.is_searching = false;
            }
        }
        KeyCode::Backspace => {
            app.search_input.pop();
            app.apply_search_filter(app.search_mode);
        }
        KeyCode::Up => {
            app.browse_up();
        }
        KeyCode::Down => {
            app.browse_down();
        }
        KeyCode::Char(c) => {
            app.search_input.push(c);
            app.apply_search_filter(app.search_mode);
        }
        _ => {}
    }
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
        CurrentScreen::GroupingNotes => render_grouping_screen(f, app),
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

/// Render search overlay based on search mode
fn render_search_overlay(f: &mut Frame, app: &mut App) {
    match app.search_mode {
        SearchMode::LiveGrep => {
            let search_area = centered_rect(90, 80, f.area());
            let search_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3), // Title
                    Constraint::Length(3), // Input
                    Constraint::Min(0),    // Content area
                ])
                .split(search_area);

            let content_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(50), // File list
                    Constraint::Percentage(50), // Preview
                ])
                .split(search_chunks[2]);

            f.render_widget(Clear, search_area);
            f.render_widget(
                create_search_title("Content Search (Live Grep)"),
                search_chunks[0],
            );
            f.render_widget(
                create_search_input(&app.search_input, "Enter search query..."),
                search_chunks[1],
            );

            // File list with matches
            let file_items: Vec<ListItem> = app
                .filtered_browse_items
                .iter()
                .map(|(text, _)| ListItem::new(text.as_str()))
                .collect();
            let file_list = List::new(file_items)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Files with Matches"),
                )
                .highlight_style(
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                );
            f.render_stateful_widget(file_list, content_chunks[0], &mut app.browse_list_state);

            // Preview window
            let preview_text = generate_grep_preview(
                app.browse_list_state.selected(),
                &app.filtered_browse_paths,
                &app.grep_matches,
            );
            let preview = Paragraph::new(preview_text)
                .style(Style::default().fg(Color::White))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Match Preview"),
                )
                .wrap(ratatui::widgets::Wrap { trim: false });
            f.render_widget(preview, content_chunks[1]);
        }
        SearchMode::FuzzySearch => {
            let search_area = centered_rect(75, 15, f.area());
            let search_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3), // Title
                    Constraint::Length(5), // Input
                ])
                .split(search_area);

            f.render_widget(Clear, search_area);
            f.render_widget(create_search_title("Filename Search"), search_chunks[0]);
            f.render_widget(
                create_search_input(&app.search_input, "Enter search query..."),
                search_chunks[1],
            );
        }
    }
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

    // Note list - use filtered items if searching, otherwise use all items
    let items_to_show = if app.is_searching {
        &app.filtered_browse_items
    } else {
        &app.browse_items
    };

    let notes: Vec<ListItem> = items_to_show
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
    let help_text = if app.is_searching {
        match app.search_mode {
            SearchMode::LiveGrep => {
                "↑↓/j/k: Navigate | Type to search | Enter: Open File | Esc: Cancel | Q: Quit"
            }
            SearchMode::FuzzySearch => {
                "Type to search | Esc: Cancel Search | Enter: Exit Search | Q: Quit"
            }
        }
    } else {
        "↑↓ Navigate | /: Search | Space/→: Expand/Collapse | Enter: Open | N: New Note | F: New Folder | G: Group Notes | Esc: Back | Q: Quit"
    };
    let footer = Paragraph::new(help_text)
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[2]);

    // Render floating search window if searching
    if app.is_searching {
        render_search_overlay(f, app);
    }
}

/// New Note screen - shows popup dialog for entering note name with autocomplete
fn render_editing_screen(f: &mut Frame, app: &mut App) {
    // Create a centered popup dialog - larger to accommodate suggestions
    let popup_area = centered_rect(70, 50, f.area());

    // Split the popup into sections
    let popup_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Title
            Constraint::Length(5),  // Input field
            Constraint::Min(5),     // Suggestions list
            Constraint::Length(3),  // Help text
        ])
        .split(popup_area);

    // Title
    let title = Paragraph::new("New Note (default: root directory)")
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
        "Enter note name or path (e.g., folder/note.md)...".to_string()
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
        .block(Block::default().borders(Borders::ALL).title("Note Name/Path"))
        .wrap(ratatui::widgets::Wrap { trim: false });
    f.render_widget(input, popup_chunks[1]);

    // Autocomplete suggestions
    if !app.autocomplete_suggestions.is_empty() {
        let suggestion_items: Vec<ListItem> = app
            .autocomplete_suggestions
            .iter()
            .enumerate()
            .map(|(idx, suggestion)| {
                let style = if app.autocomplete_selected == Some(idx) {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                ListItem::new(format!("📁 {}", suggestion)).style(style)
            })
            .collect();

        let suggestions_list = List::new(suggestion_items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Folder Suggestions (Tab to accept)"),
            )
            .highlight_style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            );
        f.render_widget(suggestions_list, popup_chunks[2]);
    } else {
        let empty_text = Paragraph::new("No folder suggestions")
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::ALL).title("Folder Suggestions"));
        f.render_widget(empty_text, popup_chunks[2]);
    }

    // Help text
    let help_text = "Enter: Create & Edit | Tab: Accept suggestion | ↑↓: Navigate | Esc: Cancel";
    let footer = Paragraph::new(help_text)
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, popup_chunks[3]);
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

/// Grouping screen - shows confirmation, preview, and progress for note grouping
fn render_grouping_screen(f: &mut Frame, app: &mut App) {
    let popup_area = centered_rect(85, 85, f.area());
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Title
            Constraint::Min(5),     // Content area
            Constraint::Length(3),  // Help text
        ])
        .split(popup_area);
    
    f.render_widget(Clear, popup_area);
    
    let title = Paragraph::new("Group Notes by Similarity")
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);
    
    if app.is_grouping {
        // Show progress
        let status_text = format!(
            "{}\n\nPlease wait...",
            app.grouping_progress.as_deref().unwrap_or("Processing...")
        );
        let status = Paragraph::new(status_text)
            .style(Style::default().fg(Color::White))
            .block(Block::default().borders(Borders::ALL).title("Status"))
            .wrap(ratatui::widgets::Wrap { trim: false });
        f.render_widget(status, chunks[1]);
    } else if let Some(groups) = &app.grouping_result {
        // Show groups preview with navigation
        let content_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(40), // Group list
                Constraint::Percentage(60), // Group details
            ])
            .split(chunks[1]);
        
        // Group list
        let group_items: Vec<ListItem> = groups
            .iter()
            .enumerate()
            .map(|(idx, group)| {
                let style = if app.grouping_selected == Some(idx) {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                ListItem::new(format!("{}. {} ({} notes)", idx + 1, group.name, group.notes.len()))
                    .style(style)
            })
            .collect();
        
        let group_list = List::new(group_items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!("Groups ({})", groups.len())),
            )
            .highlight_style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            );
        
        // Create a temporary ListState for rendering
        let mut list_state = ratatui::widgets::ListState::default();
        list_state.select(app.grouping_selected);
        f.render_stateful_widget(group_list, content_chunks[0], &mut list_state);
        
        // Group details
        let details_text = if let Some(selected_idx) = app.grouping_selected {
            if let Some(group) = groups.get(selected_idx) {
                let notes_list: String = group
                    .notes
                    .iter()
                    .enumerate()
                    .map(|(i, path)| {
                        let name = path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("unknown");
                        format!("  {}. {}", i + 1, name)
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                format!("Group: {}\n\nNotes in this group:\n{}", group.name, notes_list)
            } else {
                "Select a group to see details".to_string()
            }
        } else if !groups.is_empty() {
            format!(
                "{} groups found.\n\nUse ↑↓ to navigate groups.\n\nPress Enter to apply grouping.\nPress Esc to cancel.",
                groups.len()
            )
        } else {
            "No groups found. Press Esc to cancel.".to_string()
        };
        
        let details = Paragraph::new(details_text)
            .style(Style::default().fg(Color::White))
            .block(Block::default().borders(Borders::ALL).title("Group Details"))
            .wrap(ratatui::widgets::Wrap { trim: false });
        f.render_widget(details, content_chunks[1]);
    } else {
        // Initial screen
        let status_text = "Press Enter to compute groups.\n\nThis will:\n  1. Flatten all notes to root directory\n  2. Analyze content similarity\n  3. Show groups for review\n  4. Apply grouping if confirmed\n\nWarning: This will reorganize your notes!".to_string();
        let status = Paragraph::new(status_text)
            .style(Style::default().fg(Color::White))
            .block(Block::default().borders(Borders::ALL).title("Status"))
            .wrap(ratatui::widgets::Wrap { trim: false });
        f.render_widget(status, chunks[1]);
    }
    
    let help = if app.is_grouping {
        "Processing... Please wait"
    } else if app.grouping_result.is_some() {
        if app.grouping_applied {
            "Esc: Return to Browse"
        } else {
            "↑↓: Navigate | Enter: Apply Grouping | Esc: Cancel"
        }
    } else {
        "Enter: Compute Groups | Esc: Cancel"
    };
    
    let help_widget = Paragraph::new(help)
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(help_widget, chunks[2]);
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
                        app.target_directory = None; // Default to root directory
                        app.update_autocomplete_suggestions();
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
                    if app.is_searching {
                        handle_search_input(key.code, app);
                    } else {
                        // Normal browse mode
                        match key.code {
                            KeyCode::Esc => {
                                app.current_screen = CurrentScreen::Main;
                            }
                            KeyCode::Char('q') | KeyCode::Char('Q') => {
                                app.current_screen = CurrentScreen::Exiting;
                            }
                            KeyCode::Char('/') => {
                                start_search(app, SearchMode::FuzzySearch);
                            }
                            KeyCode::Char('?') => {
                                start_search(app, SearchMode::LiveGrep);
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
                                    if let Err(_e) = launch_editor(file_path, &app.settings.editor)
                                    {
                                        // Error launching editor - continue in TUI
                                    }
                                    app.current_file =
                                        Some(file_path.to_string_lossy().to_string());
                                    // Reload browse items to reflect any changes made in the editor
                                    app.load_browse_items();
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
                                app.update_autocomplete_suggestions();
                                app.current_screen = CurrentScreen::Editing;
                            }
                            KeyCode::Char('f') | KeyCode::Char('F') => {
                                // Create new folder - go to folder creation screen
                                app.target_directory = Some(app.get_selected_directory());
                                app.folder_name_input.clear();
                                app.current_screen = CurrentScreen::CreatingFolder;
                            }
                            KeyCode::Char('g') | KeyCode::Char('G') => {
                                // Start grouping operation
                                app.current_screen = CurrentScreen::GroupingNotes;
                                app.is_grouping = false;
                                app.grouping_progress = None;
                                app.grouping_result = None;
                                app.grouping_selected = None;
                                app.grouping_applied = false;
                            }
                            _ => {}
                        }
                    }
                }
                CurrentScreen::GroupingNotes => {
                    match key.code {
                        KeyCode::Enter => {
                            if app.is_grouping {
                                // Still processing, ignore
                            } else if let Some(groups) = &app.grouping_result {
                                // Groups computed, apply them (flatten and organize)
                                if !app.grouping_applied && !groups.is_empty() {
                                    match crate::grouping::apply_grouping(&app.settings, groups) {
                                        Ok(_) => {
                                            app.grouping_applied = true;
                                            app.grouping_progress = Some(format!("Applied {} groups", groups.len()));
                                            // Reload browse items to show new structure
                                            app.load_browse_items();
                                        }
                                        Err(e) => {
                                            app.grouping_progress = Some(format!("Error applying: {}", e));
                                        }
                                    }
                                }
                            } else {
                                // Compute groups
                                app.is_grouping = true;
                                app.grouping_progress = Some("Flattening notes...".to_string());
                                
                                let config = crate::grouping::GroupingConfig::default();
                                match crate::grouping::compute_groups(&app.settings, config) {
                                    Ok(groups) => {
                                        let num_groups = groups.len();
                                        app.grouping_progress = Some(format!("Computed {} groups", num_groups));
                                        app.grouping_result = Some(groups);
                                        app.grouping_selected = if num_groups > 0 { Some(0) } else { None };
                                        app.grouping_applied = false;
                                        app.is_grouping = false;
                                    }
                                    Err(e) => {
                                        app.grouping_progress = Some(format!("Error: {}", e));
                                        app.is_grouping = false;
                                    }
                                }
                            }
                        }
                        KeyCode::Up => {
                            if let Some(groups) = &app.grouping_result {
                                if !groups.is_empty() {
                                    let new_idx = match app.grouping_selected {
                                        Some(idx) if idx > 0 => Some(idx - 1),
                                        Some(_) => Some(0),
                                        None => Some(0),
                                    };
                                    app.grouping_selected = new_idx;
                                }
                            }
                        }
                        KeyCode::Down => {
                            if let Some(groups) = &app.grouping_result {
                                if !groups.is_empty() {
                                    let max_idx = groups.len() - 1;
                                    let new_idx = match app.grouping_selected {
                                        Some(idx) if idx < max_idx => Some(idx + 1),
                                        Some(_) => Some(max_idx),
                                        None => Some(0),
                                    };
                                    app.grouping_selected = new_idx;
                                }
                            }
                        }
                        KeyCode::Esc => {
                            app.current_screen = CurrentScreen::Browsing;
                            app.is_grouping = false;
                            app.grouping_progress = None;
                            app.grouping_result = None;
                            app.grouping_selected = None;
                            app.grouping_applied = false;
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
                                    let target_dir = app.target_directory.take();

                                    // Launch editor with the new note
                                    if let Err(_e) = launch_editor(&file_path, &app.settings.editor)
                                    {
                                        // Error launching editor - continue in TUI
                                    }

                                    // Return to appropriate screen after editor exits
                                    if target_dir.is_some() {
                                        // Came from browse screen, return there
                                        app.current_screen = CurrentScreen::Browsing;
                                        // Expand the target directory and reload to show new note
                                        if let Some(dir) = target_dir {
                                            app.expanded_folders.insert(dir);
                                        }
                                        app.load_browse_items(); // Reload to show new note
                                    } else {
                                        // Came from main screen
                                        app.current_screen = CurrentScreen::Main;
                                    }
                                    app.note_name_input.clear();
                                    app.autocomplete_suggestions.clear();
                                    app.autocomplete_selected = None;
                                    app.current_file =
                                        Some(file_path.to_string_lossy().to_string());
                                }
                                Err(e) => {
                                    eprintln!("Error creating note file: {}", e);
                                    // Stay in editing screen on error
                                }
                            }
                        }
                        KeyCode::Tab => {
                            // Accept selected autocomplete suggestion
                            if let Some(selected_idx) = app.autocomplete_selected {
                                if let Some(suggestion) = app.autocomplete_suggestions.get(selected_idx) {
                                    app.note_name_input = suggestion.clone();
                                    app.update_autocomplete_suggestions();
                                }
                            } else if !app.autocomplete_suggestions.is_empty() {
                                // If nothing selected, use first suggestion
                                app.note_name_input = app.autocomplete_suggestions[0].clone();
                                app.update_autocomplete_suggestions();
                            }
                        }
                        KeyCode::Up => {
                            // Navigate up in autocomplete suggestions
                            if !app.autocomplete_suggestions.is_empty() {
                                let new_idx = match app.autocomplete_selected {
                                    Some(idx) if idx > 0 => Some(idx - 1),
                                    Some(_) => Some(0),
                                    None => Some(0),
                                };
                                app.autocomplete_selected = new_idx;
                            }
                        }
                        KeyCode::Down => {
                            // Navigate down in autocomplete suggestions
                            if !app.autocomplete_suggestions.is_empty() {
                                let max_idx = app.autocomplete_suggestions.len() - 1;
                                let new_idx = match app.autocomplete_selected {
                                    Some(idx) if idx < max_idx => Some(idx + 1),
                                    Some(_) => Some(max_idx),
                                    None => Some(0),
                                };
                                app.autocomplete_selected = new_idx;
                            }
                        }
                        KeyCode::Backspace => {
                            // Remove last character
                            app.note_name_input.pop();
                            app.update_autocomplete_suggestions();
                        }
                        KeyCode::Esc => {
                            // Cancel and return to previous screen
                            if app.target_directory.is_some() {
                                app.current_screen = CurrentScreen::Browsing;
                            } else {
                                app.current_screen = CurrentScreen::Main;
                            }
                            app.note_name_input.clear();
                            app.autocomplete_suggestions.clear();
                            app.autocomplete_selected = None;
                            app.target_directory = None;
                            app.current_file = None;
                        }
                        KeyCode::Char(c) => {
                            // Add character to input (allow alphanumeric, spaces, dashes, underscores, dots, slashes)
                            if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' || c == '.' || c == '/' {
                                app.note_name_input.push(c);
                                app.update_autocomplete_suggestions();
                            }
                        }
                        _ => {}
                    }
                }
                CurrentScreen::CreatingFolder => {
                    match key.code {
                        KeyCode::Enter => {
                            // Create folder (load_browse_items is called inside create_new_folder)
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
