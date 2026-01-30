use crate::settings::Settings;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::fs::create_dir_all;

/// Check if a path should be shown based on expanded folders
/// A path is shown if all its parent directories (except base) are expanded
fn should_show_path(path: &Path, base_dir: &Path, expanded_folders: &HashSet<PathBuf>) -> bool {
    if path == base_dir {
        return false;
    }
    
    // Check all parent directories up to base_dir
    let mut current = path;
    while let Some(parent) = current.parent() {
        if parent == base_dir {
            // Reached base directory, all parents are expanded
            return true;
        }
        
        // If this parent is not expanded, don't show the path
        if !expanded_folders.contains(&parent.to_path_buf()) {
            return false;
        }
        
        current = parent;
    }
    
    true
}

// Return both list items and their corresponding paths, filtered by expanded folders
pub fn get_files_as_list_items_with_paths(
    settings: &Settings,
    expanded_folders: &HashSet<PathBuf>,
) -> Result<(Vec<(String, bool)>, Vec<Option<PathBuf>>), Box<dyn std::error::Error>> {
    let base_dir = Path::new(&settings.notes_directory);
    let pattern = base_dir.join("**/*").to_string_lossy().to_string();

    let mut items: Vec<(String, bool)> = Vec::new(); // (display_text, is_file)
    let mut paths: Vec<Option<PathBuf>> = Vec::new();
    let mut current_folder: Option<PathBuf> = None;

    for entry in glob::glob(&pattern)? {
        let path = entry?;

        if path == base_dir {
            continue;
        }

        // Only show paths whose parent folders are expanded
        if !should_show_path(&path, base_dir, expanded_folders) {
            continue;
        }

        if let Some(parent) = path.parent() {
            // Add folder header when folder changes
            if current_folder.as_deref() != Some(parent) {
                if let Ok(folder_name) = parent.strip_prefix(base_dir) {
                    let folder_display = if folder_name.as_os_str().is_empty() {
                        "Root".to_string()
                    } else {
                        folder_name.to_string_lossy().to_string()
                    };
                    items.push((format!("📂 {}", folder_display), false));
                    paths.push(None); // Folder headers have no path
                }
                current_folder = Some(parent.to_path_buf());
            }

            // Add file/folder item
            let display_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();

            let is_file = path.is_file();
            let is_expanded = path.is_dir() && expanded_folders.contains(&path);
            let expand_indicator = if is_expanded { "▼ " } else { "▶ " };
            let display_text = if path.is_dir() {
                format!("  {}📁 {}", expand_indicator, display_name)
            } else {
                format!("  📄 {}", display_name)
            };

            items.push((display_text, is_file));
            paths.push(Some(path));
        }
    }

    Ok((items, paths))
}

pub fn make_new_folder(parent_folder: &Path, new_folder: &Path) ->Result<(), Box<dyn std::error::Error>> {
    let new_folder_str = format!("{}/{}",parent_folder.display(),new_folder.display());
    let new_folder_path = Path::new(&new_folder_str);

    create_dir_all(new_folder_path)?;
    Ok(())
}
