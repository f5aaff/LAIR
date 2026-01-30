use ratatui::widgets::ListState;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::browse;

pub enum CurrentScreen {
    Main,
    Browsing,
    Editing,
    CreatingFolder,
    Exiting,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SettingsField {
    NotesDirectory,
    Editor,
    FileFormat,
}

pub struct App {
    pub current_file: Option<String>,
    pub current_screen: CurrentScreen,
    pub note_name_input: String, // For entering new note name
    pub folder_name_input: String, // For entering new folder name
    pub settings: crate::settings::Settings,
    pub settings_field_inputs: [String; 3], // Input buffers for each settings field
    pub active_settings_field: Option<SettingsField>, // Which field is currently being edited
    pub browse_list_state: ListState,       // State for browse list selection
    pub browse_items: Vec<(String, bool)>,  // (display_text, is_file) pairs for browse items
    pub browse_paths: Vec<Option<std::path::PathBuf>>, // Corresponding paths (None for folder headers)
    pub expanded_folders: HashSet<PathBuf>, // Set of expanded folder paths
    pub target_directory: Option<PathBuf>, // Directory where new note/folder should be created (from browse)
}
impl App {
    pub fn new() -> App {
        let settings = crate::settings::Settings::load();
        let notes_dir = settings.notes_directory.clone();
        let editor = settings.editor.clone();
        let file_format = settings.default_file_format.clone();

        App {
            current_screen: CurrentScreen::Main,
            current_file: None,
            note_name_input: String::new(),
            folder_name_input: String::new(),
            settings,
            settings_field_inputs: [notes_dir, editor, file_format],
            active_settings_field: None,
            browse_list_state: ListState::default(),
            browse_items: Vec::new(),
            browse_paths: Vec::new(),
            expanded_folders: HashSet::new(),
            target_directory: None,
        }
    }

    /// Update settings from input buffers and save
    pub fn save_settings(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.settings.notes_directory = self.settings_field_inputs[0].clone();
        self.settings.editor = self.settings_field_inputs[1].clone();
        self.settings.default_file_format = self.settings_field_inputs[2].clone();
        self.settings.save()?;
        Ok(())
    }

    /// Reset settings inputs to current settings values
    pub fn reset_settings_inputs(&mut self) {
        self.settings_field_inputs[0] = self.settings.notes_directory.clone();
        self.settings_field_inputs[1] = self.settings.editor.clone();
        self.settings_field_inputs[2] = self.settings.default_file_format.clone();
    }

    pub fn load_browse_items(&mut self) {
        match crate::browse::get_files_as_list_items_with_paths(&self.settings, &self.expanded_folders) {
            Ok((items, paths)) => {
                self.browse_items = items;
                self.browse_paths = paths;

                // Reset selection to first item if available
                if !self.browse_items.is_empty() {
                    self.browse_list_state.select(Some(0));
                } else {
                    self.browse_list_state.select(None);
                }
            }
            Err(_) => {
                self.browse_items = vec![("Error loading notes".to_string(), false)];
                self.browse_paths = vec![None];
                self.browse_list_state.select(None);
            }
        }
    }
    /// Navigate up in browse list
    pub fn browse_up(&mut self) {
        if let Some(selected) = self.browse_list_state.selected() {
            if selected > 0 {
                self.browse_list_state.select(Some(selected - 1));
            }
        } else if !self.browse_items.is_empty() {
            self.browse_list_state.select(Some(0));
        }
    }

    /// Navigate down in browse list
    pub fn browse_down(&mut self) {
        if let Some(selected) = self.browse_list_state.selected() {
            if selected < self.browse_items.len().saturating_sub(1) {
                self.browse_list_state.select(Some(selected + 1));
            }
        } else if !self.browse_items.is_empty() {
            self.browse_list_state.select(Some(0));
        }
    }

    /// Get the selected file path (if a file is selected)
    pub fn get_selected_file_path(&self) -> Option<&std::path::PathBuf> {
        if let Some(selected) = self.browse_list_state.selected() {
            if let Some(Some(path)) = self.browse_paths.get(selected) {
                if path.is_file() {
                    return Some(path);
                }
            }
        }
        None
    }

    /// Get the selected directory path (if a directory is selected) or parent of selected file
    pub fn get_selected_directory(&self) -> PathBuf {
        if let Some(selected) = self.browse_list_state.selected() {
            if let Some(Some(path)) = self.browse_paths.get(selected) {
                if path.is_dir() {
                    return path.clone();
                } else if path.is_file() {
                    // If a file is selected, use its parent directory
                    return path.parent().unwrap_or_else(|| Path::new(&self.settings.notes_directory)).to_path_buf();
                }
            }
        }
        // Nothing selected or invalid selection, use base notes directory
        PathBuf::from(&self.settings.notes_directory)
    }

    /// Create a new folder in the target directory (or selected directory if target not set)
    pub fn create_new_folder(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let parent_folder = self.target_directory.clone().unwrap_or_else(|| self.get_selected_directory());
        
        // Use folder_name_input if provided, otherwise use timestamp
        let new_folder_name = if self.folder_name_input.trim().is_empty() {
            let datetime = chrono::Utc::now().format("%Y-%m-%d_%H-%M");
            datetime.to_string()
        } else {
            self.folder_name_input.trim().to_string()
        };
        
        let new_folder_path = Path::new(&new_folder_name);
        browse::make_new_folder(&parent_folder, new_folder_path)?;
        
        // Clear input and reset target directory
        self.folder_name_input.clear();
        self.target_directory = None;
        
        // Reload browse items to show the new folder
        self.load_browse_items();
        Ok(())
    }

    /// Toggle expand/collapse state of the selected folder
    pub fn toggle_folder_expansion(&mut self) {
        if let Some(selected) = self.browse_list_state.selected() {
            if let Some(Some(path)) = self.browse_paths.get(selected) {
                if path.is_dir() {
                    if self.expanded_folders.contains(path) {
                        self.expanded_folders.remove(path);
                    } else {
                        self.expanded_folders.insert(path.clone());
                    }
                    // Reload items to reflect expansion state
                    self.load_browse_items();
                }
            }
        }
    }
}
