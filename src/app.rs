use ratatui::widgets::ListState;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::browse;
use crate::search;
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SearchMode {
    LiveGrep,
    FuzzySearch,
}

pub struct App {
    pub current_file: Option<String>,
    pub current_screen: CurrentScreen,
    pub note_name_input: String,   // For entering new note name
    pub folder_name_input: String, // For entering new folder name
    pub settings: crate::settings::Settings,
    pub settings_field_inputs: [String; 3], // Input buffers for each settings field
    pub active_settings_field: Option<SettingsField>, // Which field is currently being edited
    pub browse_list_state: ListState,       // State for browse list selection
    pub browse_items: Vec<(String, bool)>,  // (display_text, is_file) pairs for browse items
    pub browse_paths: Vec<Option<std::path::PathBuf>>, // Corresponding paths (None for folder headers)
    pub expanded_folders: HashSet<PathBuf>,            // Set of expanded folder paths
    pub target_directory: Option<PathBuf>, // Directory where new note/folder should be created (from browse)
    pub search_input: String,              // Search query input
    pub is_searching: bool,                // Whether search mode is active
    pub search_mode: SearchMode,           // Current search mode (FuzzySearch or LiveGrep)
    pub filtered_browse_items: Vec<(String, bool)>, // Filtered items based on search
    pub filtered_browse_paths: Vec<Option<PathBuf>>, // Filtered paths based on search
    pub grep_matches: Vec<crate::search::MatchInfo>, // Match information for live grep
    pub autocomplete_suggestions: Vec<String>,      // Folder suggestions for autocomplete
    pub autocomplete_selected: Option<usize>,       // Selected suggestion index
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
            search_input: String::new(),
            is_searching: false,
            search_mode: SearchMode::FuzzySearch,
            filtered_browse_items: Vec::new(),
            filtered_browse_paths: Vec::new(),
            grep_matches: Vec::new(),
            autocomplete_suggestions: Vec::new(),
            autocomplete_selected: None,
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
        // Preserve the currently selected path or folder header before reloading
        let selected_idx = self.browse_list_state.selected();
        let selected_path = selected_idx
            .and_then(|idx| self.browse_paths.get(idx))
            .and_then(|path_opt| path_opt.as_ref())
            .cloned();

        // Also preserve the display text if it was a folder header (path is None)
        let selected_display = selected_idx
            .and_then(|idx| self.browse_items.get(idx))
            .map(|(text, _)| text.clone());

        match crate::browse::get_files_as_list_items_with_paths(
            &self.settings,
            &self.expanded_folders,
        ) {
            Ok((items, paths)) => {
                self.browse_items = items;
                self.browse_paths = paths;

                // If searching, reapply search filter
                if self.is_searching {
                    self.apply_search_filter(SearchMode::FuzzySearch);
                } else {
                    // Not searching, clear filtered items
                    self.filtered_browse_items.clear();
                    self.filtered_browse_paths.clear();
                }

                // Try to restore selection
                if let Some(path_to_find) = selected_path {
                    // Find the index of the path we had selected before
                    if let Some(new_idx) = self
                        .browse_paths
                        .iter()
                        .position(|p| p.as_ref().map(|p2| p2 == &path_to_find).unwrap_or(false))
                    {
                        self.browse_list_state.select(Some(new_idx));
                    } else if !self.browse_items.is_empty() {
                        // Path not found, try to maintain approximate position
                        let old_idx = selected_idx.unwrap_or(0);
                        let new_idx = old_idx.min(self.browse_items.len().saturating_sub(1));
                        self.browse_list_state.select(Some(new_idx));
                    } else {
                        self.browse_list_state.select(None);
                    }
                } else if let Some(display_to_find) = selected_display {
                    // Was a folder header, try to find the same header
                    if let Some(new_idx) = self
                        .browse_items
                        .iter()
                        .position(|(text, _)| text == &display_to_find)
                    {
                        self.browse_list_state.select(Some(new_idx));
                    } else if !self.browse_items.is_empty() {
                        // Header not found, try to maintain approximate position
                        let old_idx = selected_idx.unwrap_or(0);
                        let new_idx = old_idx.min(self.browse_items.len().saturating_sub(1));
                        self.browse_list_state.select(Some(new_idx));
                    } else {
                        self.browse_list_state.select(None);
                    }
                } else if !self.browse_items.is_empty() {
                    // No previous selection, select first item
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
        let items_to_use = if self.is_searching {
            &self.filtered_browse_items
        } else {
            &self.browse_items
        };

        if let Some(selected) = self.browse_list_state.selected() {
            if selected > 0 {
                self.browse_list_state.select(Some(selected - 1));
            }
        } else if !items_to_use.is_empty() {
            self.browse_list_state.select(Some(0));
        }
    }

    /// Navigate down in browse list
    pub fn browse_down(&mut self) {
        let items_to_use = if self.is_searching {
            &self.filtered_browse_items
        } else {
            &self.browse_items
        };

        if let Some(selected) = self.browse_list_state.selected() {
            if selected < items_to_use.len().saturating_sub(1) {
                self.browse_list_state.select(Some(selected + 1));
            }
        } else if !items_to_use.is_empty() {
            self.browse_list_state.select(Some(0));
        }
    }

    /// Get the selected file path (if a file is selected)
    pub fn get_selected_file_path(&self) -> Option<&std::path::PathBuf> {
        let selected = self.browse_list_state.selected()?;
        let paths_to_use = if self.is_searching {
            &self.filtered_browse_paths
        } else {
            &self.browse_paths
        };
        let path = paths_to_use.get(selected)?.as_ref()?;
        if path.is_file() { Some(path) } else { None }
    }

    /// Get the selected directory path (if a directory is selected) or parent of selected file
    /// Returns the directory where new items should be created
    /// Uses filtered paths when searching, otherwise uses all paths
    pub fn get_selected_directory(&self) -> PathBuf {
        let paths_to_use = if self.is_searching {
            &self.filtered_browse_paths
        } else {
            &self.browse_paths
        };

        let selected = match self.browse_list_state.selected() {
            Some(s) => s,
            None => {
                return PathBuf::from(&self.settings.notes_directory);
            }
        };

        let path = match paths_to_use.get(selected) {
            Some(Some(p)) => p,
            _ => {
                return PathBuf::from(&self.settings.notes_directory);
            }
        };

        if path.is_dir() {
            // If a directory is selected, use that directory
            path.clone()
        } else if path.is_file() {
            // If a file is selected, use its parent directory
            path.parent()
                .unwrap_or_else(|| Path::new(&self.settings.notes_directory))
                .to_path_buf()
        } else {
            // Nothing selected or invalid selection, use base notes directory
            PathBuf::from(&self.settings.notes_directory)
        }
    }

    /// Create a new folder in the target directory (or selected directory if target not set)
    pub fn create_new_folder(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let parent_folder = self
            .target_directory
            .clone()
            .unwrap_or_else(|| self.get_selected_directory());

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
        let target_dir = self.target_directory.take();

        // Reload browse items to show the new folder
        self.load_browse_items();

        // If we were creating in a specific directory, expand it so the new folder is visible
        if let Some(dir) = target_dir {
            self.expanded_folders.insert(dir);
            // Reload again to show the expanded folder's contents
            self.load_browse_items();
        }

        Ok(())
    }

    /// Update autocomplete suggestions based on current note name input
    pub fn update_autocomplete_suggestions(&mut self) {
        self.autocomplete_suggestions.clear();
        self.autocomplete_selected = None;

        let input = self.note_name_input.trim();
        
        // Get all directories
        let dirs = match browse::get_all_directories(&self.settings) {
            Ok(d) => d,
            Err(_) => return,
        };

        if input.is_empty() {
            // If input is empty, show top-level directories only
            let base_dir = Path::new(&self.settings.notes_directory);
            for dir in dirs {
                // Only show directories that are direct children of base_dir
                let dir_path = base_dir.join(&dir);
                if let Some(parent) = dir_path.parent() {
                    if parent == base_dir {
                        self.autocomplete_suggestions.push(format!("{}/", dir));
                    }
                }
            }
            // Limit to 10 suggestions
            if self.autocomplete_suggestions.len() > 10 {
                self.autocomplete_suggestions.truncate(10);
            }
            return;
        }

        // Check if input contains a path separator
        let (prefix, suffix) = if let Some(last_slash) = input.rfind('/') {
            let (p, s) = input.split_at(last_slash + 1);
            (p.trim_end_matches('/'), s)
        } else {
            ("", input)
        };

        let base_dir = Path::new(&self.settings.notes_directory);
        let prefix_path = if prefix.is_empty() {
            base_dir.to_path_buf()
        } else {
            base_dir.join(prefix)
        };

        // Find directories that match the prefix and suffix
        for dir in dirs {
            let dir_path = base_dir.join(&dir);
            
            // Check if this directory is under the prefix path
            if dir_path.starts_with(&prefix_path) {
                // Get the relative path from prefix
                if let Ok(rel_path) = dir_path.strip_prefix(&prefix_path) {
                    if let Some(rel_str) = rel_path.to_str() {
                        if !rel_str.is_empty() {
                            // Get the first component of the relative path
                            let first_component = rel_path.components().next()
                                .and_then(|c| c.as_os_str().to_str())
                                .unwrap_or("");
                            
                            // Check if it matches the suffix (case-insensitive)
                            if first_component.to_lowercase().starts_with(&suffix.to_lowercase()) {
                                // Build the suggestion
                                let suggestion = if prefix.is_empty() {
                                    format!("{}/", first_component)
                                } else {
                                    format!("{}/{}/", prefix, first_component)
                                };
                                
                                if !self.autocomplete_suggestions.contains(&suggestion) {
                                    self.autocomplete_suggestions.push(suggestion);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Limit to 10 suggestions
        if self.autocomplete_suggestions.len() > 10 {
            self.autocomplete_suggestions.truncate(10);
        }
    }

    pub fn toggle_folder_expansion(&mut self) {
        let selected = match self.browse_list_state.selected() {
            Some(s) => s,
            None => return,
        };

        let path = match self.browse_paths.get(selected) {
            Some(Some(p)) if p.is_dir() => p,
            _ => return,
        };

        if self.expanded_folders.contains(path) {
            self.expanded_folders.remove(path);
        } else {
            self.expanded_folders.insert(path.clone());
        }
        // Reload items to reflect expansion state (preserves selection)
        self.load_browse_items();
    }

    /// Apply search filter to browse items
    /// Searches through ALL files in the notes directory, regardless of folder expansion
    pub fn apply_search_filter(&mut self, search_mode: SearchMode) {
        self.search_mode = search_mode;
        
        if self.search_input.trim().is_empty() {
            // No search query, show all items
            self.filtered_browse_items = self.browse_items.clone();
            self.filtered_browse_paths = self.browse_paths.clone();
            self.grep_matches.clear();
            return;
        }

        match search_mode {
            SearchMode::LiveGrep => {
                // Content search using live_grep
                match search::live_grep(&self.search_input, &self.settings) {
                    Ok(matches) => {
                        // Store all matches
                        self.grep_matches = matches.clone();
                        
                        // Group matches by file and count matches per file
                        use std::collections::HashMap;
                        let mut file_match_counts: HashMap<PathBuf, usize> = HashMap::new();
                        for m in &matches {
                            *file_match_counts.entry(m.file_path.clone()).or_insert(0) += 1;
                        }
                        
                        let mut filtered_items = Vec::new();
                        let mut filtered_paths = Vec::new();
                        let base_dir = Path::new(&self.settings.notes_directory);

                        // Build display items for each unique file
                        for (file_path, match_count) in file_match_counts {
                            let filename = file_path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .map(|s| s.to_string())
                                .unwrap_or_default();

                            let display_text =
                                if let Ok(relative) = file_path.strip_prefix(base_dir) {
                                    if let Some(parent) = relative.parent() {
                                        let parent_str = if parent.as_os_str().is_empty() {
                                            "Root".to_string()
                                        } else {
                                            parent
                                                .components()
                                                .filter_map(|c| c.as_os_str().to_str())
                                                .collect::<Vec<_>>()
                                                .join(" / ")
                                        };
                                        format!("📂 {} / 📄 {} ({} matches)", parent_str, filename, match_count)
                                    } else {
                                        format!("📄 {} ({} matches)", filename, match_count)
                                    }
                                } else {
                                    format!("📄 {} ({} matches)", filename, match_count)
                                };

                            filtered_items.push((display_text, true));
                            filtered_paths.push(Some(file_path));
                        }

                        self.filtered_browse_items = filtered_items;
                        self.filtered_browse_paths = filtered_paths;
                    }
                    Err(_) => {
                        self.filtered_browse_items.clear();
                        self.filtered_browse_paths.clear();
                        self.grep_matches.clear();
                    }
                }
            }
            SearchMode::FuzzySearch => {
                // Filename search using fuzzy/substring matching
                let all_files = match browse::get_all_files(&self.settings) {
                    Ok(files) => files,
                    Err(_) => {
                        self.filtered_browse_items.clear();
                        self.filtered_browse_paths.clear();
                        return;
                    }
                };

                let query_lower = self.search_input.to_lowercase();
                let max_edits = 3;

                // Extract filenames from all files
                let all_file_entries: Vec<(String, bool)> = all_files
                    .iter()
                    .filter_map(|path| {
                        path.file_name()
                            .and_then(|n| n.to_str())
                            .map(|s| (s.to_string(), true))
                    })
                    .collect();

                let all_file_entries_lower: Vec<(String, bool)> = all_file_entries
                    .iter()
                    .map(|(name, is_file)| (name.to_lowercase(), *is_file))
                    .collect();

                let mut matching_filenames: HashSet<String> = HashSet::new();

                // First, try substring matching
                for (name, _) in &all_file_entries_lower {
                    if name.contains(&query_lower) {
                        matching_filenames.insert(name.clone());
                    }
                }

                // If no substring matches, try fuzzy search
                if matching_filenames.is_empty() {
                    let search_results_lower = search::fuzzy_search(
                        &query_lower,
                        all_file_entries_lower.clone(),
                        max_edits,
                    );
                    matching_filenames = search_results_lower.into_iter().collect();
                }

                // Build filtered results
                let mut filtered_items = Vec::new();
                let mut filtered_paths = Vec::new();

                for file_path in all_files {
                    let filename = file_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map(|s| s.to_string())
                        .unwrap_or_default();

                    let filename_lower = filename.to_lowercase();

                    if matching_filenames.contains(&filename_lower) {
                        let base_dir = Path::new(&self.settings.notes_directory);
                        let display_text = if let Ok(relative) = file_path.strip_prefix(base_dir) {
                            if let Some(parent) = relative.parent() {
                                let parent_str = if parent.as_os_str().is_empty() {
                                    "Root".to_string()
                                } else {
                                    parent
                                        .components()
                                        .filter_map(|c| c.as_os_str().to_str())
                                        .collect::<Vec<_>>()
                                        .join(" / ")
                                };
                                format!("📂 {} / 📄 {}", parent_str, filename)
                            } else {
                                format!("📄 {}", filename)
                            }
                        } else {
                            format!("📄 {}", filename)
                        };

                        filtered_items.push((display_text, true));
                        filtered_paths.push(Some(file_path));
                    }
                }

                self.filtered_browse_items = filtered_items;
                self.filtered_browse_paths = filtered_paths;
            }
        }

        // Reset selection
        if !self.filtered_browse_items.is_empty() {
            self.browse_list_state.select(Some(0));
        } else {
            self.browse_list_state.select(None);
        }
    }
}
