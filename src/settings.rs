use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub notes_directory: String,
    pub editor: String,
    pub default_file_format: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            notes_directory: dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("notes")
                .to_string_lossy()
                .to_string(),
            editor: "nvim".to_string(),
            default_file_format: "md".to_string(),
        }
    }
}

impl Settings {
    /// Get the path to the settings file
    fn settings_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("escritoire")
            .join("settings.json")
    }

    /// Load settings from JSON file, or return default if file doesn't exist
    pub fn load() -> Self {
        let path = Self::settings_path();
        
        if path.exists() {
            match fs::read_to_string(&path) {
                Ok(content) => {
                    match serde_json::from_str::<Settings>(&content) {
                        Ok(settings) => settings,
                        Err(e) => {
                            eprintln!("Error parsing settings file: {}. Using defaults.", e);
                            Self::default()
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error reading settings file: {}. Using defaults.", e);
                    Self::default()
                }
            }
        } else {
            // Create default settings and save them
            let default = Self::default();
            if let Err(e) = default.save() {
                eprintln!("Warning: Could not save default settings: {}", e);
            }
            default
        }
    }

    /// Save settings to JSON file
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = Self::settings_path();
        
        // Create directory if it doesn't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Serialize to JSON
        let json = serde_json::to_string_pretty(self)?;
        
        // Write to file
        fs::write(&path, json)?;
        
        Ok(())
    }
}
