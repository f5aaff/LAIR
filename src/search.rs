use fuzzy_search::automata::LevenshteinAutomata;
use grep::regex::RegexMatcher;
use grep::searcher::sinks::UTF8;
use grep::searcher::SearcherBuilder;
use std::cell::Cell;
use std::io;
use std::path::PathBuf;

use crate::browse::get_all_files;
use crate::settings::Settings;

pub fn fuzzy_search(
    query: &str,
    all_entries: Vec<(String, bool)>,
    max_edits: usize,
) -> Vec<String> {
    let files: Vec<String> = all_entries
        .into_iter()
        .filter_map(|(s, b)| b.then_some(s))
        .collect();

    if query.is_empty() {
        return files;
    }

    let automata = LevenshteinAutomata::new(query, max_edits);
    automata.fuzzy_search(&files)
}

pub fn live_grep(
    query: &str,
    settings: &Settings,
) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let files = get_all_files(settings)?;
    // Create a case-insensitive regex matcher
    let pattern = format!(r"(?i){}", regex::escape(query));
    let matcher = RegexMatcher::new(&pattern)?;
    let mut searcher = SearcherBuilder::new()
        .binary_detection(grep::searcher::BinaryDetection::quit(b'\x00'))
        .build();

    let mut matching_files = Vec::new();
    for file_path in files {
        let matched = Cell::new(false);
        let sink = UTF8(|_line_num: u64, _line: &str| -> io::Result<bool> {
            matched.set(true);
            Ok(false)
        });

        // Search the file
        if let Ok(reader) = std::fs::File::open(&file_path) {
            let _ = searcher.search_file(&matcher, &reader, sink);
            if matched.get() {
                matching_files.push(file_path);
            }
        }
    }
    Ok(matching_files)
}
