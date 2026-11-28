//! Quick Open's answers against VS Code's own, for the separators and path case of this platform.

use serde::Deserialize;

use super::super::{file_list::FileList, js, scoring};
use crate::scope::path::NativePath;

#[derive(Deserialize)]
struct Corpus {
    lists: Vec<List>,
}

#[derive(Deserialize)]
struct List {
    folders: Vec<String>,
    files: Vec<(usize, String)>,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Case {
    query: String,
    max_results: usize,
    outcomes: Vec<Outcome>,
}

/// A file, its score and its label and description ranges.
type Scored = (usize, u64, Vec<[u64; 2]>, Vec<[u64; 2]>);

/// A result: its absolute path, its score and its label and description ranges.
type Answer = (String, u64, Vec<[u64; 2]>, Vec<[u64; 2]>);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Outcome {
    platforms: Vec<String>,
    limit_hit: bool,
    results: Vec<Scored>,
}

/// The platform whose separators and path case this build scores with.
const PLATFORM: &str = if cfg!(windows) {
    "windows"
} else if cfg!(target_os = "linux") {
    "linux"
} else {
    "macos"
};

const ROOT: &str = if cfg!(windows) {
    r"C:\corpus"
} else {
    "/corpus"
};

fn native(path: &str) -> String {
    if cfg!(windows) {
        path.replace('/', "\\")
    } else {
        path.to_owned()
    }
}

#[test]
fn scoring_matches_vs_code() {
    let corpus: Corpus = serde_json::from_str(include_str!("quick-open.json")).unwrap();
    let separator = if cfg!(windows) { '\\' } else { '/' };
    for list in corpus.lists {
        let folders: Vec<NativePath> = list
            .folders
            .iter()
            .map(|name| NativePath::parse(&format!("{ROOT}{separator}{name}")).unwrap())
            .collect();
        let mut files = FileList::new(&folders);
        for (folder, path) in &list.files {
            assert!(files.push(*folder, &js::units(&native(path))));
        }
        for case in &list.cases {
            let outcome = case
                .outcomes
                .iter()
                .find(|outcome| outcome.platforms.iter().any(|name| name == PLATFORM))
                .unwrap();
            let found = scoring::find(&files, &case.query, case.max_results, || false).unwrap();
            let actual: Vec<Answer> = found
                .results
                .iter()
                .map(|file| {
                    let spans = |spans: &[[orchbee_files_protocol::SafeInteger; 2]]| {
                        spans
                            .iter()
                            .map(|[start, end]| [start.get(), end.get()])
                            .collect()
                    };
                    (
                        file.path.as_str().to_owned(),
                        file.score.get(),
                        spans(&file.label_matches),
                        spans(&file.description_matches),
                    )
                })
                .collect();
            let expected: Vec<Answer> = outcome
                .results
                .iter()
                .map(|(index, score, label, description)| {
                    let (folder, path) = &list.files[*index];
                    let path = format!("{}{separator}{}", folders[*folder].as_str(), native(path));
                    (path, *score, label.clone(), description.clone())
                })
                .collect();
            assert_eq!(actual, expected, "query {:?}", case.query);
            assert_eq!(found.limit_hit, outcome.limit_hit, "query {:?}", case.query);
        }
    }
}
