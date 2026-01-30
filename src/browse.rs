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


/// Recursively add items for a directory and its children
fn add_directory_items(
    dir_path: &Path,
    base_dir: &Path,
    expanded_folders: &HashSet<PathBuf>,
    paths_by_parent: &std::collections::BTreeMap<PathBuf, Vec<PathBuf>>,
    items: &mut Vec<(String, bool)>,
    paths: &mut Vec<Option<PathBuf>>,
    depth: usize,
) {
    // Get children of this directory
    if let Some(children) = paths_by_parent.get(dir_path) {
        let mut sorted_children = children.clone();
        sorted_children.sort();
        
        for child_path in sorted_children {
            let display_name = child_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();

            let is_file = child_path.is_file();
            let is_expanded = child_path.is_dir() && expanded_folders.contains(&child_path);
            let expand_indicator = if is_expanded { "▼ " } else { "▶ " };
            
            // Indent based on depth
            let item_indent = "  ".repeat(depth);
            
            let display_text = if child_path.is_dir() {
                format!("{} {}📁 {}", item_indent, expand_indicator, display_name)
            } else {
                format!("{} 📄 {}", item_indent, display_name)
            };

            items.push((display_text, is_file));
            paths.push(Some(child_path.clone()));

            // If this is an expanded directory, recursively add its children
            if is_expanded {
                add_directory_items(&child_path, base_dir, expanded_folders, paths_by_parent, items, paths, depth + 1);
            }
        }
    }
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

    // Collect all paths first
    let mut all_paths: Vec<PathBuf> = Vec::new();
    for entry in glob::glob(&pattern)? {
        let path = entry?;
        if path != base_dir {
            all_paths.push(path);
        }
    }

    // Sort paths to ensure consistent ordering
    all_paths.sort();

    // Group paths by their parent directory
    let mut paths_by_parent: std::collections::BTreeMap<PathBuf, Vec<PathBuf>> = std::collections::BTreeMap::new();
    for path in all_paths {
        // Only show paths whose parent folders are expanded
        if !should_show_path(&path, base_dir, expanded_folders) {
            continue;
        }

        if let Some(parent) = path.parent() {
            let parent_path = parent.to_path_buf();
            paths_by_parent.entry(parent_path).or_insert_with(Vec::new).push(path);
        }
    }

    // Add root folder header
    items.push((format!("📂 Root"), false));
    paths.push(None); // Folder headers have no path

    // Recursively add items starting from root (depth 0 for root's children)
    add_directory_items(base_dir, base_dir, expanded_folders, &paths_by_parent, &mut items, &mut paths, 1);

    Ok((items, paths))
}

pub fn make_new_folder(parent_folder: &Path, new_folder: &Path) ->Result<(), Box<dyn std::error::Error>> {
    let new_folder_str = format!("{}/{}",parent_folder.display(),new_folder.display());
    let new_folder_path = Path::new(&new_folder_str);

    create_dir_all(new_folder_path)?;
    Ok(())
}
