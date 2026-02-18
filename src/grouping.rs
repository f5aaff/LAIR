use crate::settings::Settings;
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Represents a group of similar notes
#[derive(Debug, Clone)]
pub struct NoteGroup {
    pub name: String,
    pub notes: Vec<PathBuf>,
}

/// Grouping configuration
#[derive(Debug, Clone)]
pub struct GroupingConfig {
    pub min_similarity: f64,       // Minimum similarity threshold (0.0-1.0)
    pub min_group_size: usize,     // Minimum notes per group
    pub max_groups: Option<usize>, // Optional limit on number of groups
}

impl Default for GroupingConfig {
    fn default() -> Self {
        GroupingConfig {
            min_similarity: 0.15, // Lower threshold for better grouping
            min_group_size: 2,
            max_groups: None,
        }
    }
}

/// Flatten the file system structure by moving all notes to root
pub fn flatten_notes_directory(
    settings: &Settings,
) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let base_dir = Path::new(&settings.notes_directory);
    let mut moved_files = Vec::new();

    // Recursively collect all note files
    fn collect_files(
        dir: &Path,
        base_dir: &Path,
        file_format: &str,
        files: &mut Vec<PathBuf>,
    ) -> io::Result<()> {
        let entries = fs::read_dir(dir)?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                    match ext == file_format {
                        true => {
                            files.push(path);
                        }
                        false => (),
                    }
                }
            } else if path.is_dir() && path != base_dir {
                collect_files(&path, base_dir, file_format, files)?;
            }
        }
        Ok(())
    }

    let mut all_files = Vec::new();
    collect_files(
        base_dir,
        base_dir,
        &settings.default_file_format,
        &mut all_files,
    )?;

    // Move all files to root, handling name conflicts
    for file_path in all_files {
        if let Some(file_name) = file_path.file_name() {
            let target_path = base_dir.join(file_name);

            // Handle name conflicts by adding a number
            let mut final_path = target_path.clone();
            let mut counter = 1;
            while final_path.exists() && final_path != file_path {
                let stem = target_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("note");
                let ext = target_path
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or(&settings.default_file_format);
                final_path = base_dir.join(format!("{}_{}.{}", stem, counter, ext));
                counter += 1;
            }

            if final_path != file_path {
                fs::rename(&file_path, &final_path)?;
                moved_files.push(final_path);
            } else {
                moved_files.push(file_path);
            }
        }
    }

    // Remove empty directories
    remove_empty_directories(base_dir, base_dir)?;

    Ok(moved_files)
}

/// Remove empty directories recursively
fn remove_empty_directories(dir: &Path, base_dir: &Path) -> io::Result<()> {
    if dir == base_dir {
        return Ok(()); // Don't remove the base directory
    }

    let entries: Vec<_> = fs::read_dir(dir)?.collect();
    let mut has_content = false;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            remove_empty_directories(&path, base_dir)?;
            // Check if directory is now empty
            if let Ok(mut dir_entries) = fs::read_dir(&path) {
                if dir_entries.next().is_none() {
                    // Directory is empty, will be removed
                } else {
                    has_content = true;
                }
            }
        } else {
            has_content = true;
        }
    }

    if !has_content {
        fs::remove_dir(dir)?;
    }

    Ok(())
}

/// Read note content and extract text (stripping markdown formatting but keeping content)
fn extract_note_content(file_path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    // Read the full file content, similar to how grep works
    let content = fs::read_to_string(file_path)?;

    // Strip markdown formatting but keep the actual text content
    // This is more permissive - we keep headers, lists, etc., just remove formatting chars
    let text = content
        .lines()
        .map(|line| {
            let trimmed = line.trim();
            // Remove markdown formatting characters but keep the text
            trimmed
                // Remove header markers but keep the text after
                .trim_start_matches('#')
                .trim()
                // Remove list markers but keep the text
                .trim_start_matches('-')
                .trim_start_matches('*')
                .trim_start_matches('+')
                .trim()
                .trim_start_matches(|c: char| c.is_ascii_digit() && c != '0')
                .trim_start_matches('.')
                .trim()
                // Remove markdown link/image syntax but keep text
                .replace("![", "")
                .replace("](", " ")
                .replace(')', "")
                // Remove emphasis but keep text
                .replace("**", "")
                .replace('*', "")
                .replace("__", "")
                .replace('_', "")
                // Remove code backticks but keep content
                .replace("```", "")
                .replace('`', "")
                // Remove brackets but keep content
                .replace('[', "")
                .replace(']', "")
        })
        .filter(|line| !line.is_empty()) // Only remove completely empty lines
        .collect::<Vec<_>>()
        .join(" ");

    Ok(text)
}

/// Compute TF-IDF vectors for notes with content, returning vectors and filtered notes
fn compute_tfidf_vectors(
    notes: &[PathBuf],
) -> Result<(Vec<Vec<f64>>, Vec<PathBuf>), Box<dyn std::error::Error>> {
    let mut documents: Vec<Vec<String>> = Vec::new();
    let mut term_counts: HashMap<String, usize> = HashMap::new();
    let mut notes_with_content: Vec<PathBuf> = Vec::new();

    // Extract terms from each document
    for note_path in notes {
        let content = extract_note_content(note_path)?;

        // If content is empty or too short, skip this note
        if content.trim().len() < 10 {
            continue;
        }

        // Extract meaningful terms from content
        // More lenient filtering to capture more content
        let terms: Vec<String> = content
            .split_whitespace()
            .map(|s| {
                // Remove punctuation from word boundaries
                s.trim_matches(|c: char| !c.is_alphanumeric())
            })
            .filter(|s| s.len() > 1) // More lenient - keep 2+ char words
            .map(|s| s.to_lowercase())
            .collect();

        // Only add document if it has meaningful terms
        if !terms.is_empty() {
            for term in &terms {
                *term_counts.entry(term.clone()).or_insert(0) += 1;
            }
            documents.push(terms);
            notes_with_content.push(note_path.clone());
        }
    }

    let total_docs = documents.len() as f64;

    // Compute TF-IDF vectors
    let mut vectors = Vec::new();
    let all_terms: Vec<String> = term_counts.keys().cloned().collect();

    for doc_terms in &documents {
        let mut vector = Vec::new();
        let doc_len = doc_terms.len().max(1) as f64;

        for term in &all_terms {
            // Term Frequency (raw count normalized by document length)
            let term_count = doc_terms.iter().filter(|&t| t == term).count() as f64;
            let tf = term_count / doc_len;

            // Inverse Document Frequency (log to reduce impact of very common terms)
            let df = term_counts.get(term).copied().unwrap_or(0) as f64;
            let idf = if df > 0.0 && df < total_docs {
                // Standard IDF formula
                (total_docs / df).ln()
            } else {
                0.0
            };

            vector.push(tf * idf);
        }

        // Normalize the vector to unit length for better cosine similarity
        let norm: f64 = vector.iter().map(|x| x * x).sum::<f64>().sqrt();
        if norm > 0.0 {
            for val in &mut vector {
                *val /= norm;
            }
        }

        vectors.push(vector);
    }

    Ok((vectors, notes_with_content))
}

/// Compute cosine similarity between two vectors
fn cosine_similarity(a: &[f64], b: &[f64]) -> f64 {
    if a.len() != b.len() {
        return 0.0;
    }

    let dot_product: f64 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f64 = a.iter().map(|x| x * x).sum::<f64>().sqrt();
    let norm_b: f64 = b.iter().map(|x| x * x).sum::<f64>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot_product / (norm_a * norm_b)
}

/// Group notes by similarity using threshold-based clustering
pub fn group_notes_by_similarity(
    notes: &[PathBuf],
    config: &GroupingConfig,
) -> Result<Vec<NoteGroup>, Box<dyn std::error::Error>> {
    if notes.len() < 2 {
        return Ok(vec![]);
    }

    // Compute TF-IDF vectors - returns vectors and filtered notes (aligned)
    let (vectors, notes_with_content) = compute_tfidf_vectors(notes)?;

    if notes_with_content.len() < 2 || vectors.len() != notes_with_content.len() {
        return Ok(vec![]);
    }

    // Compute similarity matrix using the filtered notes
    let num_notes = notes_with_content.len();
    let mut similarity_matrix: Vec<Vec<f64>> = vec![vec![0.0; num_notes]; num_notes];

    for i in 0..num_notes {
        similarity_matrix[i][i] = 1.0; // Self-similarity
        for j in i + 1..num_notes {
            let similarity = cosine_similarity(&vectors[i], &vectors[j]);
            similarity_matrix[i][j] = similarity;
            similarity_matrix[j][i] = similarity;
        }
    }

    // Improved clustering: use transitive similarity (if A similar to B and B similar to C, group all)
    let mut groups: Vec<NoteGroup> = Vec::new();
    let mut used = vec![false; num_notes];

    for i in 0..num_notes {
        if used[i] {
            continue;
        }

        // Start a new group with this note
        let mut group_indices = vec![i];
        used[i] = true;

        // Use a queue to find all transitively similar notes
        let mut queue = vec![i];
        while let Some(current_idx) = queue.pop() {
            // Find all notes similar to the current note
            for j in 0..num_notes {
                if !used[j] && similarity_matrix[current_idx][j] >= config.min_similarity {
                    group_indices.push(j);
                    used[j] = true;
                    queue.push(j); // Also check notes similar to this one (transitive)
                }
            }
        }

        // Only create group if it meets minimum size
        if group_indices.len() >= config.min_group_size {
            let group_notes: Vec<PathBuf> = group_indices
                .iter()
                .map(|&idx| notes_with_content[idx].clone())
                .collect();

            // Generate group name from first note's content (first few words)
            let group_name = if let Ok(content) = extract_note_content(&group_notes[0]) {
                let words: Vec<&str> = content
                    .split_whitespace()
                    .take(3)
                    .map(|s| s.trim_matches(|c: char| !c.is_alphanumeric()))
                    .filter(|s| !s.is_empty())
                    .collect();
                if !words.is_empty() {
                    words.join("_").chars().take(30).collect()
                } else {
                    group_notes[0]
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| format!("group_{}", groups.len() + 1))
                }
            } else {
                group_notes[0]
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| format!("group_{}", groups.len() + 1))
            };

            groups.push(NoteGroup {
                name: group_name,
                notes: group_notes,
            });
        }
    }

    // Limit number of groups if specified
    if let Some(max) = config.max_groups {
        if groups.len() > max {
            // Sort by size (largest first) and take top N
            groups.sort_by(|a, b| b.notes.len().cmp(&a.notes.len()));
            groups.truncate(max);
        }
    }

    Ok(groups)
}

/// Sanitize folder name to be filesystem-safe
fn sanitize_folder_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(30) // Limit length
        .collect()
}

/// Organize notes into folders based on groups
pub fn organize_notes_into_groups(
    settings: &Settings,
    groups: &[NoteGroup],
) -> Result<(), Box<dyn std::error::Error>> {
    let base_dir = Path::new(&settings.notes_directory);

    for (idx, group) in groups.iter().enumerate() {
        // Create folder for group
        let folder_name = format!("group_{:02}_{}", idx + 1, sanitize_folder_name(&group.name));
        let group_dir = base_dir.join(&folder_name);
        fs::create_dir_all(&group_dir)?;

        // Move notes into group folder
        for note_path in &group.notes {
            if let Some(file_name) = note_path.file_name() {
                let target_path = group_dir.join(file_name);

                // Handle name conflicts
                let mut final_path = target_path.clone();
                let mut counter = 1;
                while final_path.exists() && &final_path != note_path {
                    let stem = target_path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("note");
                    let ext = target_path
                        .extension()
                        .and_then(|s| s.to_str())
                        .unwrap_or("md");
                    final_path = group_dir.join(format!("{}_{}.{}", stem, counter, ext));
                    counter += 1;
                }

                if &final_path != note_path {
                    fs::rename(note_path, &final_path)?;
                }
            }
        }
    }

    Ok(())
}

/// Get all note files without moving them (read-only)
fn get_all_note_files(settings: &Settings) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    use crate::browse::get_all_files;
    get_all_files(settings)
}

/// Compute groups without organizing files (for preview) - completely read-only
pub fn compute_groups(
    settings: &Settings,
    config: GroupingConfig,
) -> Result<Vec<NoteGroup>, Box<dyn std::error::Error>> {
    // Get all note files WITHOUT moving them (read-only operation)
    let all_notes = get_all_note_files(settings)?;

    if all_notes.is_empty() {
        return Ok(vec![]);
    }

    // Group notes by similarity (without organizing or moving files)
    let groups = group_notes_by_similarity(&all_notes, &config)?;

    Ok(groups)
}

/// Apply grouping: flatten notes and organize into groups (only call after user confirmation)
pub fn apply_grouping(
    settings: &Settings,
    groups: &[NoteGroup],
) -> Result<(), Box<dyn std::error::Error>> {
    // Step 1: Flatten the directory structure (only when applying)
    flatten_notes_directory(settings)?;

    // Step 2: Organize into folders
    if !groups.is_empty() {
        organize_notes_into_groups(settings, groups)?;
    }

    Ok(())
}
