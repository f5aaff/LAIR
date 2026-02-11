use fuzzy_search::automata::LevenshteinAutomata;
use grep::regex::RegexMatcher;
use grep::searcher::sinks::UTF8;
use grep::searcher::SearcherBuilder;
use std::cell::RefCell;
use std::io;
use std::path::PathBuf;

use crate::browse::get_all_files;
use crate::settings::Settings;

#[derive(Debug, Clone)]
pub struct MatchInfo {
    pub file_path: PathBuf,
    pub line_number: u64,
    pub line_content: String,
}

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
) -> Result<Vec<MatchInfo>, Box<dyn std::error::Error>> {
    let files = get_all_files(settings)?;
    // Create a case-insensitive regex matcher
    let pattern = format!(r"(?i){}", regex::escape(query));
    let matcher = RegexMatcher::new(&pattern)?;
    let mut searcher = SearcherBuilder::new()
        .binary_detection(grep::searcher::BinaryDetection::quit(b'\x00'))
        .build();

    let mut matches = Vec::new();
    for file_path in files {
        let file_matches = RefCell::new(Vec::new());
        let sink = UTF8(|line_num: u64, line: &str| -> io::Result<bool> {
            file_matches.borrow_mut().push((line_num, line.to_string()));
            Ok(false) // Continue searching
        });

        // Search the file
        if let Ok(reader) = std::fs::File::open(&file_path) {
            let _ = searcher.search_file(&matcher, &reader, sink);
            // Add all matches from this file
            for (line_num, line_content) in file_matches.into_inner() {
                matches.push(MatchInfo {
                    file_path: file_path.clone(),
                    line_number: line_num,
                    line_content,
                });
            }
        }
    }
    Ok(matches)
}
