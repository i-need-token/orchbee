//! rg's arguments for a text search and for a file listing, as VS Code's `getRgArgs` and
//! `ripgrepFileSearch.ts` build them at ef5ca45, with the one departure the File Bar makes: a
//! folder inside a Git working tree below its top keeps the `.gitignore` files up to the top.
//! rg runs in the folder, so every path it reports is relative to it.

use std::{collections::HashSet, path::Path};

use super::{glob, js, pattern, regexp::Rejected};

/// What a text search looks for.
pub(super) struct TextQuery<'a> {
    pub(super) pattern: &'a str,
    pub(super) regex: bool,
    pub(super) case_sensitive: bool,
    pub(super) whole_word: bool,
    pub(super) multiline: bool,
    /// The encoding the request named, before VS Code's renaming.
    pub(super) encoding: Option<&'a str>,
    /// Lines of context before and after each match; 0 for none.
    pub(super) context: u32,
}

/// Where and how one folder is searched or listed.
pub(super) struct FolderOptions<'a> {
    /// The request's includes, then the folder's own, each once.
    pub(super) includes: &'a [String],
    /// The ordinary excludes, without exceptions.
    pub(super) excludes: &'a [String],
    pub(super) use_ignore_files: bool,
    pub(super) use_parent_ignore_files: bool,
    pub(super) follow_symlinks: bool,
    pub(super) ignore_glob_case: bool,
    /// The folder lies in a Git working tree below its top, so the files up to the top apply.
    pub(super) below_top: bool,
}

impl FolderOptions<'_> {
    /// Whether the run that reads ignore files keeps the `.gitignore` files above the folder.
    const fn keeps_parent_ignores(&self) -> bool {
        self.use_ignore_files && !self.use_parent_ignore_files && self.below_top
    }
}

/// Whether `folder` lies in a Git working tree below its top: the first folder on the walk up
/// from it that holds `.git`, a folder or a linked worktree's file, is above it.
pub(super) fn below_top(folder: &Path) -> bool {
    folder
        .ancestors()
        .find(|ancestor| {
            std::fs::metadata(ancestor.join(".git"))
                .is_ok_and(|metadata| metadata.is_dir() || metadata.is_file())
        })
        .is_some_and(|top| top != folder)
}

/// The arguments of a text search's run in one folder; `exceptions` makes it the second run,
/// which lists only what exclude exceptions re-include.
///
/// # Errors
///
/// [`Rejected`] for a whole-word regular expression JavaScript rejects.
pub(super) fn text(
    query: &TextQuery<'_>,
    folder: &FolderOptions<'_>,
    exceptions: Option<&[String]>,
) -> Result<Vec<String>, Rejected> {
    let parents = folder.keeps_parent_ignores() && exceptions.is_none();
    let mut arguments = vec!["--hidden".to_owned()];
    if !parents {
        arguments.push("--no-require-git".to_owned());
    }
    arguments.push(
        if query.case_sensitive {
            "--case-sensitive"
        } else {
            "--ignore-case"
        }
        .to_owned(),
    );
    if folder.ignore_glob_case {
        arguments.extend(
            ["--glob-case-insensitive", "--ignore-file-case-insensitive"].map(String::from),
        );
    }
    match exceptions {
        Some(exceptions) => push_globs(&mut arguments, exception_globs(exceptions)),
        None => push_include_and_exclude_globs(&mut arguments, folder),
    }
    if folder.use_ignore_files && exceptions.is_none() {
        if !folder.use_parent_ignore_files && !parents {
            arguments.push("--no-ignore-parent".to_owned());
        }
    } else {
        arguments.push("--no-ignore".to_owned());
    }
    if folder.follow_symlinks {
        arguments.push("--follow".to_owned());
    }
    if let Some(encoding) = query.encoding.map(canonical_encoding)
        && !encoding.is_empty()
        && encoding != "utf8"
    {
        arguments.extend(["--encoding".to_owned(), encoding]);
    }
    let (mut search, mut regex) = (query.pattern.to_owned(), query.regex);
    // rg reads `--` alone as the end of its options.
    if search == "--" {
        (search, regex) = ("\\-\\-".to_owned(), true);
    }
    if query.multiline && !regex {
        (search, regex) = (js::escape_regexp(&search), true);
    }
    arguments.push("--crlf".to_owned());
    if regex {
        search = pattern::unicode_escapes(&search);
        arguments.extend(["--engine", "auto"].map(String::from));
    }
    let mut after_separator = None;
    if query.whole_word {
        arguments.extend(["--regexp".to_owned(), pattern::whole_word(&search, regex)?]);
    } else if regex {
        let fixed = pattern::fix_newline(&pattern::fix_regex_newline(&search));
        arguments.extend(["--regexp".to_owned(), fixed]);
    } else {
        after_separator = Some(search);
        arguments.push("--fixed-strings".to_owned());
    }
    arguments.extend(["--no-config", "--no-ignore-global", "--json"].map(String::from));
    if query.multiline {
        arguments.push("--multiline".to_owned());
    }
    if query.context > 0 {
        let lines = query.context.to_string();
        arguments.extend([
            "--before-context".to_owned(),
            lines.clone(),
            "--after-context".to_owned(),
            lines,
        ]);
    }
    arguments.push("--".to_owned());
    arguments.extend(after_separator.filter(|search| !search.is_empty()));
    arguments.push(".".to_owned());
    Ok(arguments)
}

/// The arguments of a file listing's run in one folder; `exceptions` makes it the second run.
pub(super) fn files(folder: &FolderOptions<'_>, exceptions: Option<&[String]>) -> Vec<String> {
    let parents = folder.keeps_parent_ignores() && exceptions.is_none();
    let mut arguments = ["--files", "--hidden", "--case-sensitive"]
        .map(String::from)
        .to_vec();
    if !parents {
        arguments.push("--no-require-git".to_owned());
    }
    if folder.ignore_glob_case {
        arguments.extend(
            ["--glob-case-insensitive", "--ignore-file-case-insensitive"].map(String::from),
        );
    }
    match exceptions {
        Some(exceptions) => push_globs(&mut arguments, exception_globs(exceptions)),
        None => {
            let includes = listing_globs(folder.includes).map(|include| anchor(&include));
            push_globs(&mut arguments, includes);
            let excludes =
                listing_globs(folder.excludes).map(|exclude| format!("!{}", anchor(&exclude)));
            push_globs(&mut arguments, excludes);
        }
    }
    if exceptions.is_some() || !folder.use_ignore_files {
        arguments.push("--no-ignore".to_owned());
    } else if !folder.use_parent_ignore_files && !parents {
        arguments.push("--no-ignore-parent".to_owned());
    }
    if folder.follow_symlinks {
        arguments.push("--follow".to_owned());
    }
    arguments.extend(["--no-config", "--no-ignore-global"].map(String::from));
    arguments
}

fn push_globs(arguments: &mut Vec<String>, globs: impl IntoIterator<Item = String>) {
    for glob in globs {
        arguments.push("-g".to_owned());
        arguments.push(glob);
    }
}

/// `pushIncludeAndExcludeGlobs`: includes that do not start with `**` exclude everything else at
/// the top and let rg into each of their parent folders; the others pass as they are; each
/// exclude is anchored and negated, and comes last, so it wins.
fn push_include_and_exclude_globs(arguments: &mut Vec<String>, folder: &FolderOptions<'_>) {
    let (double_star, others): (Vec<&String>, Vec<&String>) = folder
        .includes
        .iter()
        .partition(|include| include.starts_with("**"));
    if !others.is_empty() {
        arguments.extend(["-g", "!*"].map(String::from));
        let mut seen = HashSet::new();
        let unique = others
            .into_iter()
            .filter(|other| seen.insert(other.as_str()));
        push_globs(
            arguments,
            unique
                .flat_map(|other| spread(other))
                .map(|glob| anchor(&glob)),
        );
    }
    push_globs(arguments, double_star.into_iter().cloned());
    let excludes = folder
        .excludes
        .iter()
        .map(|exclude| format!("!{}", anchor(exclude)));
    push_globs(arguments, excludes);
}

/// `getExcludeExceptionGlobs`: unless an exception can match at any depth, everything at the top
/// is excluded and only the folders that lead to the exceptions are walked; then each exception
/// and everything under it; then the version control folders no exception names.
pub(super) fn exception_globs(exceptions: &[String]) -> Vec<String> {
    let expanded: Vec<String> = exceptions
        .iter()
        .flat_map(|exception| expand_braces(exception))
        .collect();
    let mut globs = Vec::new();
    let mut add = |glob: String| {
        if !globs.contains(&glob) {
            globs.push(glob);
        }
    };
    if !expanded.iter().any(|exception| exception.starts_with("**")) {
        add("!*".to_owned());
        for exception in &expanded {
            let segments = glob::split_glob_aware(exception, '/');
            for end in 1..segments.len() {
                add(format!("{}/", anchor(&segments[..end].join("/"))));
            }
        }
    }
    for exception in &expanded {
        add(anchor(exception));
        add(anchor(&format!("{exception}/**")));
    }
    for folder in glob::skipped_folders(exceptions) {
        add(format!("!**/{folder}"));
    }
    globs
}

/// `spreadGlobComponents`: each brace alternative and every leading run of its segments.
fn spread(include: &str) -> Vec<String> {
    expand_braces(include)
        .iter()
        .flat_map(|alternative| {
            let segments = glob::split_glob_aware(alternative, '/');
            (1..=segments.len())
                .map(|end| segments[..end].join("/"))
                .collect::<Vec<_>>()
        })
        .collect()
}

/// `anchorGlob`: a glob that starts with neither `**` nor `/` is anchored at the folder.
pub(super) fn anchor(glob: &str) -> String {
    if glob.starts_with("**") || glob.starts_with('/') {
        glob.to_owned()
    } else {
        format!("/{glob}")
    }
}

/// `globExprsToRgGlobs` for a listing: trailing slashes trimmed, backslashes made slashes except
/// a UNC prefix, which then matches everything under it, and a `c:/` drive dropped.
fn listing_globs(keys: &[String]) -> impl Iterator<Item = String> + '_ {
    keys.iter().filter(|key| !key.is_empty()).map(|key| {
        let trimmed = key.trim_end_matches('\\').trim_end_matches('/');
        let unc = trimmed.starts_with("\\\\");
        let mut glob = if unc {
            format!("\\\\{}", trimmed[2..].replace('\\', "/"))
        } else {
            trimmed.replace('\\', "/")
        };
        if unc {
            glob.push_str("**");
        }
        let drive = glob.as_bytes();
        if drive.len() >= 3
            && drive[0].eq_ignore_ascii_case(&b'c')
            && drive[1] == b':'
            && drive[2] == b'/'
        {
            glob.replace_range(..3, "/");
        }
        glob
    })
}

/// `performBraceExpansionForRipgrep`: the first brace group outside escapes expands into its
/// alternatives, and the rest of the pattern recursively; an escaped brace loses its backslash,
/// and a nested group is left for rg to reject.
pub(super) fn expand_braces(pattern: &str) -> Vec<String> {
    let (start, inside, end) = match split_braces(pattern) {
        Split::Group { start, inside, end } => (start, inside, end),
        Split::Whole(whole) => return vec![whole],
    };
    let mut alternatives = glob::split_glob_aware(&inside, ',');
    if alternatives.is_empty() {
        alternatives.push(String::new());
    }
    let ends = expand_braces(end);
    let start = start.as_str();
    alternatives
        .iter()
        .flat_map(|alternative| {
            ends.iter()
                .map(move |end| format!("{start}{alternative}{end}"))
        })
        .collect()
}

enum Split<'a> {
    Group {
        start: String,
        inside: String,
        end: &'a str,
    },
    Whole(String),
}

/// `getEscapeAwareSplitStringForRipgrep`.
fn split_braces(pattern: &str) -> Split<'_> {
    let (mut in_braces, mut escaped) = (false, false);
    let (mut start, mut inside) = (String::new(), String::new());
    for (index, ch) in pattern.char_indices() {
        let target = if in_braces { &mut inside } else { &mut start };
        match ch {
            '\\' if escaped => {
                target.push_str("\\\\");
                escaped = false;
            }
            '\\' => escaped = true,
            '{' | '}' if escaped => {
                target.push(ch);
                escaped = false;
            }
            '{' if in_braces => {
                return Split::Whole(format!("{start}{{{inside}{{{}", &pattern[index + 1..]));
            }
            '{' => in_braces = true,
            '}' if in_braces => {
                return Split::Group {
                    start,
                    inside,
                    end: &pattern[index + 1..],
                };
            }
            _ => {
                if escaped {
                    target.push('\\');
                }
                target.push(ch);
                escaped = false;
            }
        }
    }
    if in_braces {
        start.push('{');
        start.push_str(&inside);
    }
    Split::Whole(start)
}

/// `toCanonicalName`: VS Code's encoding names as rg reads them.
fn canonical_encoding(encoding: &str) -> String {
    let renamed = match encoding {
        "shiftjis" => "shift-jis",
        "utf16le" => "utf-16le",
        "utf16be" => "utf-16be",
        "big5hkscs" => "big5-hkscs",
        "eucjp" => "euc-jp",
        "euckr" => "euc-kr",
        "koi8r" => "koi8-r",
        "koi8u" => "koi8-u",
        "macroman" => "x-mac-roman",
        "utf8bom" => "utf8",
        _ => {
            let windows = encoding.match_indices("windows").find_map(|(at, _)| {
                let digits = &encoding[at + 7..];
                let length = digits.bytes().take_while(u8::is_ascii_digit).count();
                (length > 0).then(|| format!("windows-{}", &digits[..length]))
            });
            return windows.unwrap_or_else(|| encoding.to_owned());
        }
    };
    renamed.to_owned()
}
