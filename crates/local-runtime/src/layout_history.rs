//! Names in the `layouts` folder an earlier build kept beside the three checkpoint slots: one entry
//! per UTC save second, `-2`, `-3` and so on for a second already taken, and one staging file.
//! Nothing reads or writes the folder; Session deletion admits exactly these names and removes them
//! with the rest of the Session folder.

/// The folder beside the three checkpoint slots.
pub(crate) const DIRECTORY: &str = "layouts";
/// The one staging file a recording wrote before its atomic rename.
const PENDING: &str = "layout.pending";
const SUFFIX: &str = ".json";

/// Whether `name` belongs in the folder: an entry or the staging file.
pub(crate) fn is_owned_name(name: &str) -> bool {
    name == PENDING
        || name
            .strip_suffix(SUFFIX)
            .and_then(crate::time_name::parse)
            .is_some()
}
