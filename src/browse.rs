use crate::settings::Settings;
use std::path::{Path, PathBuf};

// Return both list items and their corresponding paths
pub fn get_files_as_list_items_with_paths(
    settings: &Settings,
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
            let display_text = if path.is_dir() {
                format!("  📁 {}", display_name)
            } else {
                format!("  📄 {}", display_name)
            };

            items.push((display_text, is_file));
            paths.push(Some(path));
        }
    }

    Ok((items, paths))
}
