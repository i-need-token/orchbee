//! Search's evidence: VS Code's own answers for rg's arguments, the glob recheck, rg's JSON output
//! and Quick Open's scoring, V8's for its sort, then batches, text searches and file lists run
//! against a scripted rg.

mod arguments;
mod array_sort;
mod batches;
mod file_lists;
mod globs;
mod output;
mod quick_open;
mod script;
mod text_search;
