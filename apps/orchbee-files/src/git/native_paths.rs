//! Paths as this machine spells them: the separator, whether names compare without case, as VS Code
//! compares them, and the lexical form a Client's path must have before it names anything.

/// The separator of a native path.
pub(super) const SEPARATOR: char = if cfg!(windows) { '\\' } else { '/' };
/// Windows and macOS file systems compare names without case by default; Linux compares bytes.
const CASE_INSENSITIVE: bool = cfg!(any(windows, target_os = "macos"));
/// The longest path a request may carry, in UTF-8 bytes.
const PATH_LIMIT: usize = 4096;

/// The part of `path` below `parent`, `""` for `parent` itself, compared component by component;
/// `None` when `path` is not `parent` or inside it.
pub(super) fn relative<'a>(parent: &str, path: &'a str) -> Option<&'a str> {
    let parent = parent.trim_end_matches(SEPARATOR);
    let head = path.get(..parent.len())?;
    let same = if CASE_INSENSITIVE {
        head.to_lowercase() == parent.to_lowercase()
    } else {
        head == parent
    };
    if !same {
        return None;
    }
    let rest = path.get(parent.len()..)?;
    if rest.is_empty() {
        return Some("");
    }
    rest.strip_prefix(SEPARATOR)
}

/// Whether two native paths name the same location by this machine's comparison.
pub(super) fn same(left: &str, right: &str) -> bool {
    relative(left, right) == Some("")
}

/// `relative`, a path Git printed below the repository with `/` separators, joined to `toplevel`.
pub(super) fn join(toplevel: &str, relative: &str) -> String {
    let mut path = String::with_capacity(toplevel.len() + 1 + relative.len());
    path.push_str(toplevel.trim_end_matches(SEPARATOR));
    path.push(SEPARATOR);
    if SEPARATOR == '/' {
        path.push_str(relative);
    } else {
        path.extend(
            relative
                .chars()
                .map(|c| if c == '/' { SEPARATOR } else { c }),
        );
    }
    path
}

/// Git's spelling of a path below the repository: `/` separators.
pub(super) fn git_relative(relative: &str) -> String {
    if SEPARATOR == '/' {
        relative.to_owned()
    } else {
        relative.replace(SEPARATOR, "/")
    }
}

/// Whether a Client's path has the form every request path must have: absolute, at most 4,096
/// bytes, without NUL, without empty, `.` or `..` segments, and on Windows a drive path rather than
/// a verbatim, device or UNC one. Containment is checked separately.
pub(super) fn well_formed(path: &str) -> bool {
    if path.len() > PATH_LIMIT || path.contains('\0') {
        return false;
    }
    let rest = if SEPARATOR == '\\' {
        let bytes = path.as_bytes();
        let drive = bytes.len() >= 3
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && bytes[2] == b'\\';
        if !drive || path.contains('/') {
            return false;
        }
        &path[3..]
    } else {
        match path.strip_prefix('/') {
            Some(rest) => rest,
            None => return false,
        }
    };
    rest.is_empty()
        || rest
            .split(SEPARATOR)
            .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}
