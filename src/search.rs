use fuzzy_search::automata::LevenshteinAutomata;

pub fn fuzzy_search(query: &str, all_entries: Vec<(String, bool)>, max_edits: usize) -> Vec<String> {
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
