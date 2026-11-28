//! The requests `files-search` adds: a text search and the batches of its results, and the file
//! lists that Quick Open and Explorer find score. Patterns and globs are the user's or settings'
//! text, which `orchbee-files` passes to rg and its glob checks as VS Code does.

use serde::{Deserialize, Serialize};

use crate::{
    identity::{CacheId, SearchId, WorkspaceId},
    value::{EncodingName, FilePath, Integer, List, Text, some},
};

/// A glob a search includes or excludes; an exclude starting with `!` is an exception, whose
/// files are searched even where an exclude or an ignore file hides them.
pub type SearchGlob = Text<1, 4096>;

/// A folder a text search reads.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SearchFolder {
    /// The folder. It may lie outside the scope when the user typed it as an include; rg then
    /// reads it for this search only.
    pub path: FilePath,
    /// Globs relative to the folder that its files must match, beside the search's own.
    #[serde(default, skip_serializing_if = "is_empty")]
    pub includes: List<SearchGlob, 0, 256>,
}

/// The preview each matched line carries.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SearchPreview {
    /// Lines of a match to preview; with 1, a preview is one line cut around its matches.
    pub match_lines: Integer<1, 100>,
    /// UTF-16 code units a one-line preview keeps after each match's start.
    pub chars_per_line: Integer<1, 10_000>,
}

/// `search.text`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SearchText {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The search, which the Client numbers uniquely in its connection.
    pub search: SearchId,
    /// What to find.
    pub pattern: Text<1, 16_384>,
    /// The pattern is a JavaScript regular expression.
    pub regex: bool,
    /// Case matters.
    pub case_sensitive: bool,
    /// Matches start and end at word boundaries.
    pub whole_word: bool,
    /// A match may span lines.
    pub multiline: bool,
    /// The folders to search, each with rg of its own.
    pub folders: List<SearchFolder, 1, 64>,
    /// Globs relative to each folder that its files must match; none means every file.
    #[serde(default, skip_serializing_if = "is_empty")]
    pub includes: List<SearchGlob, 0, 256>,
    /// Globs relative to each folder whose files are left out, and exceptions.
    #[serde(default, skip_serializing_if = "is_empty")]
    pub excludes: List<SearchGlob, 0, 256>,
    /// Honor `.gitignore`, `.ignore` and `.rgignore` files.
    pub use_ignore_files: bool,
    /// Also honor those files in the folders above each folder.
    pub use_parent_ignore_files: bool,
    /// Search through symbolic links.
    pub follow_symlinks: bool,
    /// The encoding of the files to search, as VS Code names it; absent means UTF-8.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    pub encoding: Option<EncodingName>,
    /// The most match ranges to report.
    pub max_results: Integer<1, 20_000>,
    /// The preview each matched line carries.
    pub preview: SearchPreview,
    /// Lines of context before and after each match; absent means none.
    #[serde(
        default,
        deserialize_with = "some",
        skip_serializing_if = "Option::is_none"
    )]
    pub context: Option<Integer<1, 100>>,
}

/// A request about one search: `search.next`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SearchRequest {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The search.
    pub search: SearchId,
}

/// `search.find-files`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SearchFindFiles {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The file list to score, which the Client numbers uniquely in its connection, as VS Code's
    /// `cacheKey` names one; a list not yet held is made first.
    pub cache: CacheId,
    /// What the user typed; empty only makes the list.
    pub query: Text<0, 4096>,
    /// The folders whose files the list holds.
    pub folders: List<FilePath, 1, 64>,
    /// Globs relative to each folder that its files must match; none means every file.
    #[serde(default, skip_serializing_if = "is_empty")]
    pub includes: List<SearchGlob, 0, 256>,
    /// Globs relative to each folder whose files are left out, and exceptions.
    #[serde(default, skip_serializing_if = "is_empty")]
    pub excludes: List<SearchGlob, 0, 256>,
    /// Honor `.gitignore`, `.ignore` and `.rgignore` files.
    pub use_ignore_files: bool,
    /// Also honor those files in the folders above each folder.
    pub use_parent_ignore_files: bool,
    /// List through symbolic links.
    pub follow_symlinks: bool,
    /// The most files to return.
    pub max_results: Integer<1, 512>,
}

/// `search.close-cache`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SearchCloseCache {
    /// The Workspace.
    pub workspace_id: WorkspaceId,
    /// The file list.
    pub cache: CacheId,
}

/// `serde` predicate: an optional list is written only when it holds values.
fn is_empty<T, const MIN: usize, const MAX: usize>(list: &List<T, MIN, MAX>) -> bool {
    list.as_slice().is_empty()
}
