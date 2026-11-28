//! The results `files-search` adds: a text search's batches and the files a Quick Open query
//! scores best. Lines and columns count from 0, columns in UTF-16 code units, and every range's
//! end is exclusive.

use serde::{Deserialize, Serialize};

use crate::value::{FilePath, List, SafeInteger};

/// Where a range lies: `[startLine, startColumn, endLine, endColumn]`.
pub type SearchSpan = [SafeInteger; 4];

/// One match range, in the file and in its match's preview.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SearchRange {
    /// Where it lies in the file.
    pub source: SearchSpan,
    /// Where it lies in the preview, whose first line is line 0.
    pub preview: SearchSpan,
}

/// One matched line, or the lines one match spans, with its ranges. A line's ranges may arrive
/// in more than one match, each with the same preview.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SearchMatch {
    /// The ranges, in order.
    pub ranges: List<SearchRange, 1, 20_000>,
    /// The preview text, as the file holds it.
    pub preview: String,
}

/// One line of context around a match.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SearchContextLine {
    /// The line's number.
    pub line: SafeInteger,
    /// The line, without its line break.
    pub text: String,
}

/// Results in one file. A file's results may span consecutive entries and batches, in order.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SearchFile {
    /// The file.
    pub path: FilePath,
    /// Its matches, in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub matches: Vec<SearchMatch>,
    /// Its context lines, in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub context: Vec<SearchContextLine>,
}

/// `search.next`: the results since the previous batch.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SearchNextResult {
    /// The results, in the order rg reported them.
    pub files: Vec<SearchFile>,
    /// The search ended: this is its last batch.
    pub done: bool,
    /// The search stopped at `maxResults`.
    pub limit_hit: bool,
    /// Matched and context lines left out so far because they are too long to carry.
    pub omitted_long_lines: SafeInteger,
    /// Folders searched again without following symbolic links, which form too many loops in
    /// them, so their results may be incomplete; each is reported once.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub symlink_loops: Vec<FilePath>,
}

/// One file a Quick Open query matches.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FoundFile {
    /// The file.
    pub path: FilePath,
    /// VS Code's fuzzy score of the file's path relative to its folder.
    pub score: SafeInteger,
    /// The matched `[start, end]` ranges of the file name.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub label_matches: Vec<[SafeInteger; 2]>,
    /// The matched `[start, end]` ranges of the relative path of the file's folder.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub description_matches: Vec<[SafeInteger; 2]>,
}

/// `search.find-files`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SearchFindFilesResult {
    /// The best-scored files, best first.
    pub results: List<FoundFile, 0, 512>,
    /// The file list stopped at its bound, so some files are missing from it.
    pub truncated: bool,
    /// More files matched than `maxResults`, or than one result carries.
    pub limit_hit: bool,
}
