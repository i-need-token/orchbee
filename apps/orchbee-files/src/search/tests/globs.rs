//! The glob recheck against VS Code's own matching, with this platform's path module.

use serde::Deserialize;

use super::super::{
    glob::{self, Excludes, Globs},
    js,
};

#[derive(Deserialize)]
struct Corpus {
    platforms: Platforms,
}

#[derive(Deserialize)]
struct Platforms {
    posix: Platform,
    windows: Platform,
}

#[derive(Deserialize)]
struct Platform {
    paths: Vec<String>,
    basenames: Vec<String>,
    single: Vec<Expression>,
    expressions: Vec<Expression>,
    excludes: Vec<Exclude>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Expression {
    keys: Vec<String>,
    ignore_case: bool,
    with_basename: String,
    without_basename: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Exclude {
    keys: Vec<String>,
    ignore_case: bool,
    exceptions: Vec<String>,
    matches: String,
    excepted: String,
    excepted_without_basename: String,
}

fn bits(values: impl Iterator<Item = bool>) -> String {
    values.map(|value| if value { '1' } else { '0' }).collect()
}

/// This platform's cases, with each path and basename as UTF-16.
fn platform() -> (Platform, Vec<Vec<u16>>, Vec<Vec<u16>>) {
    let corpus: Corpus = serde_json::from_str(include_str!("globs.json")).unwrap();
    let platform = if cfg!(windows) {
        corpus.platforms.windows
    } else {
        corpus.platforms.posix
    };
    let paths = platform.paths.iter().map(|path| js::units(path)).collect();
    let basenames = platform
        .basenames
        .iter()
        .map(|name| js::units(name))
        .collect();
    (platform, paths, basenames)
}

#[test]
fn includes_match_as_vs_code_matches() {
    let (platform, paths, basenames) = platform();
    for expression in platform.single.iter().chain(&platform.expressions) {
        let globs = Globs::new(
            expression.keys.iter().map(String::as_str),
            expression.ignore_case,
        );
        let with = bits(
            paths
                .iter()
                .zip(&basenames)
                .map(|(path, name)| globs.matches(path, Some(name))),
        );
        let without = bits(paths.iter().map(|path| globs.matches(path, None)));
        let context = (&expression.keys, expression.ignore_case);
        assert_eq!(with, expression.with_basename, "{context:?}");
        assert_eq!(without, expression.without_basename, "{context:?}");
    }
}

#[test]
fn excludes_and_exceptions_match_as_vs_code_matches() {
    let (platform, paths, basenames) = platform();
    for exclude in &platform.excludes {
        let excludes = Excludes::new(&exclude.keys, exclude.ignore_case);
        let context = (&exclude.keys, exclude.ignore_case);
        assert_eq!(
            glob::exceptions(&exclude.keys),
            exclude.exceptions,
            "{context:?}"
        );
        let pairs = || paths.iter().zip(&basenames);
        let matches = bits(pairs().map(|(path, name)| excludes.matches(path, Some(name))));
        let excepted = bits(pairs().map(|(path, name)| excludes.is_excepted(path, Some(name))));
        let without = bits(paths.iter().map(|path| excludes.is_excepted(path, None)));
        assert_eq!(matches, exclude.matches, "{context:?}");
        assert_eq!(excepted, exclude.excepted, "{context:?}");
        assert_eq!(without, exclude.excepted_without_basename, "{context:?}");
    }
}
