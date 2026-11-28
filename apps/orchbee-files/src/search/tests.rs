//! Search's evidence: VS Code's own answers for rg's arguments, the glob recheck and rg's JSON
//! output, then batches and text searches run against a scripted rg.

mod arguments;
mod batches;
mod globs;
mod output;
mod script;
mod text_search;
