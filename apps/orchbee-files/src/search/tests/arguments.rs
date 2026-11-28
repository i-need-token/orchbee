//! rg's arguments against VS Code's own, and the one departure: a folder below the top of its Git
//! working tree keeps the `.gitignore` files up to the top.

use serde::Deserialize;

use super::super::args::{self, FolderOptions, TextQuery};

#[derive(Deserialize)]
struct Corpus {
    text: Vec<TextCase>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TextCase {
    pattern: String,
    regex: bool,
    case_sensitive: bool,
    whole_word: bool,
    multiline: bool,
    #[serde(flatten)]
    folder: Folder,
    encoding: Option<String>,
    context: u32,
    expected: Expected,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Folder {
    includes: Vec<String>,
    excludes: Vec<String>,
    exceptions: Option<Vec<String>>,
    use_ignore_files: bool,
    use_parent_ignore_files: bool,
    follow_symlinks: bool,
    #[serde(default)]
    ignore_glob_case: bool,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Expected {
    Arguments(Vec<String>),
    /// `"rejected"`: VS Code throws.
    Rejected(String),
}

impl Folder {
    fn options(&self, below_top: bool) -> FolderOptions<'_> {
        FolderOptions {
            includes: &self.includes,
            excludes: &self.excludes,
            use_ignore_files: self.use_ignore_files,
            use_parent_ignore_files: self.use_parent_ignore_files,
            follow_symlinks: self.follow_symlinks,
            ignore_glob_case: self.ignore_glob_case,
            below_top,
        }
    }

    /// VS Code's `arguments` for a folder below the top of a working tree: the run that reads
    /// ignore files but not those of parent folders lets rg read the parents and look for Git.
    fn below_top(&self, arguments: &[String]) -> Vec<String> {
        if !self.use_ignore_files || self.use_parent_ignore_files || self.exceptions.is_some() {
            return arguments.to_vec();
        }
        arguments
            .iter()
            .filter(|argument| *argument != "--no-require-git" && *argument != "--no-ignore-parent")
            .cloned()
            .collect()
    }
}

fn corpus() -> Corpus {
    serde_json::from_str(include_str!("arguments.json")).unwrap()
}

#[test]
fn text_search_arguments_match_vs_code() {
    for case in corpus().text {
        let query = TextQuery {
            pattern: &case.pattern,
            regex: case.regex,
            case_sensitive: case.case_sensitive,
            whole_word: case.whole_word,
            multiline: case.multiline,
            encoding: case.encoding.as_deref(),
            context: case.context,
        };
        let exceptions = case.folder.exceptions.as_deref();
        for below_top in [false, true] {
            let actual = args::text(&query, &case.folder.options(below_top), exceptions);
            match (&case.expected, actual) {
                (Expected::Arguments(expected), Ok(actual)) => {
                    let expected = if below_top {
                        case.folder.below_top(expected)
                    } else {
                        expected.clone()
                    };
                    assert_eq!(actual, expected, "pattern {:?}", case.pattern);
                }
                (Expected::Rejected(rejected), Err(_)) => assert_eq!(rejected, "rejected"),
                (Expected::Arguments(expected), Err(_)) => {
                    panic!(
                        "pattern {:?}: rejected, VS Code passes {expected:?}",
                        case.pattern
                    )
                }
                (Expected::Rejected(_), Ok(actual)) => {
                    panic!(
                        "pattern {:?}: VS Code rejects, this passes {actual:?}",
                        case.pattern
                    )
                }
            }
        }
    }
}
