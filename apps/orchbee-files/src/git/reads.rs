//! The reads Source Control asks for besides status: which paths Git ignores, the references and
//! remotes a branch picker lists, and the user's identity, each as VS Code's Git extension reads
//! it at ef5ca45.

use super::{
    command::{self, Access},
    discovery::Found,
    errors::{Classifier, GitError, GitErrorCode},
    json::escaped_len,
    native_paths,
    output::GitOutput,
    runner::{self, Limits},
};
use orchbee_files_protocol::{ErrorCode, RefKind};
use std::{collections::HashMap, sync::Mutex};

/// The most references one page carries, and the most bytes the JSON of its references and
/// remotes takes, inside a 128 KiB result.
const PAGE_REFS: usize = 2000;
const PAGE_BYTES: usize = 120 * 1024;
/// What a reference's JSON takes beside its names, with an upstream and a separator.
const REF_OVERHEAD: usize = 80;
/// What a remote's JSON takes beside its name and URLs, with a separator.
const REMOTE_OVERHEAD: usize = 40;
/// The longest reference name, remote name or identity value an answer carries.
const TEXT_LIMIT: usize = 1024;
/// The most remotes an answer lists.
const REMOTE_LIMIT: usize = 256;
/// Git 2.16 added `%(upstream:remotename)` and `%(upstream:remoteref)`.
const UPSTREAM_PARTS_VERSION: (u32, u32, u32) = (2, 16, 0);

/// `git.check-ignore`: which of `paths` Git ignores, as VS Code asks `check-ignore -v`. A path
/// that only a negated pattern matches is not ignored, nor is a path outside the repository.
pub(super) fn check_ignore(
    found: &Found,
    paths: &[String],
    limits: &Limits,
    output: &Mutex<GitOutput>,
) -> Result<Vec<String>, GitError> {
    let asked = paths
        .iter()
        .filter_map(|path| {
            native_paths::relative(&found.toplevel, path)
                .filter(|relative| !relative.is_empty())
                .map(|relative| (pathspec(&native_paths::git_relative(relative)), path))
        })
        .collect::<Vec<_>>();
    if asked.is_empty() {
        return Ok(Vec::new());
    }
    let input = asked
        .iter()
        .map(|(relative, _)| relative.as_str())
        .collect::<Vec<_>>()
        .join("\0");
    let native = asked
        .iter()
        .map(|(relative, path)| (relative.as_str(), *path))
        .collect::<HashMap<_, _>>();
    let mut command = found.command(Access::Read, None);
    command.args(["check-ignore", "-v", "-z", "--stdin"]);
    let ran = runner::run(command, input.into_bytes(), limits, output)?;
    if ran.exited_with(1) {
        return Ok(Vec::new());
    }
    if !ran.exited_with(0) {
        let in_submodule = String::from_utf8_lossy(&ran.stderr).contains(" is in submodule ");
        let mut error = GitError::failed(Classifier::General, &ran.stdout, &ran.stderr);
        if in_submodule {
            error.git_error_code = Some(GitErrorCode::IsInSubmodule);
        }
        return Err(error);
    }
    let text = String::from_utf8_lossy(&ran.stdout);
    let fields = text.split('\0').collect::<Vec<_>>();
    Ok(fields
        .chunks(4)
        .filter_map(|record| match record {
            [_, _, pattern, path] if !pattern.is_empty() && !pattern.starts_with('!') => {
                native.get(*path).map(|path| (*path).clone())
            }
            _ => None,
        })
        .collect())
}

/// A relative path as `check-ignore`, which refuses `--literal-pathspecs`, reads it literally: a
/// leading `:` would start pathspec magic, which `./` keeps from starting.
fn pathspec(relative: &str) -> String {
    if relative.starts_with(':') {
        format!("./{relative}")
    } else {
        relative.to_owned()
    }
}

/// The branch a local branch tracks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct RefUpstream {
    pub(super) remote: String,
    pub(super) name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Ref {
    /// The branch, `remote/branch` or tag name.
    pub(super) name: String,
    pub(super) kind: RefKind,
    /// The commit it names; a tag's peeled commit.
    pub(super) commit: String,
    /// A local branch's upstream, unless it tracks nothing or its upstream is gone.
    pub(super) upstream: Option<RefUpstream>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Remote {
    pub(super) name: String,
    pub(super) fetch_url: Option<String>,
    /// The first push URL, else the first URL.
    pub(super) push_url: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct RefsPage {
    pub(super) refs: Vec<Ref>,
    /// The remotes, on the first page only.
    pub(super) remotes: Vec<Remote>,
    pub(super) cursor: Option<u64>,
}

/// `git.refs`: one page of the local branches, remote branches and tags in reference order, from
/// `cursor`, and on the first page the remotes. Each page lists afresh, so references that change
/// between pages may repeat or be missed.
pub(super) fn refs(
    found: &Found,
    version: (u32, u32, u32),
    cursor: Option<u64>,
    limits: &Limits,
    output: &Mutex<GitOutput>,
) -> Result<RefsPage, GitError> {
    let mut format =
        String::from("--format=%(refname)%00%(objectname)%00%(*objectname)%00%(upstream:short)");
    format.push_str("%00%(upstream:track)");
    if version >= UPSTREAM_PARTS_VERSION {
        format.push_str("%00%(upstream:remotename)%00%(upstream:remoteref)");
    }
    let mut command = found.command(Access::Read, None);
    command.args([
        "for-each-ref",
        &format,
        "refs/heads",
        "refs/remotes",
        "refs/tags",
    ]);
    let ran = runner::run(command, Vec::new(), limits, output)?.checked(Classifier::General)?;
    let text = String::from_utf8_lossy(&ran.stdout);
    let all = text.lines().filter_map(parse_ref).collect::<Vec<_>>();
    let start = usize::try_from(cursor.unwrap_or(0))
        .map_err(|_| GitError::new(ErrorCode::InvalidRequest))?;
    let mut page = RefsPage::default();
    let mut bytes = 0;
    // The first page lists the remotes, which take up to half of it first.
    if cursor.is_none() {
        for remote in remotes(found, limits, output)? {
            let size = REMOTE_OVERHEAD
                + escaped_len(&remote.name)
                + remote.fetch_url.as_deref().map_or(0, escaped_len)
                + remote.push_url.as_deref().map_or(0, escaped_len);
            if bytes + size > PAGE_BYTES / 2 {
                break;
            }
            bytes += size;
            page.remotes.push(remote);
        }
    }
    for listed in all.iter().skip(start) {
        let size = REF_OVERHEAD
            + escaped_len(&listed.name)
            + listed.commit.len()
            + listed.upstream.as_ref().map_or(0, |upstream| {
                escaped_len(&upstream.remote) + escaped_len(&upstream.name)
            });
        if page.refs.len() == PAGE_REFS || (!page.refs.is_empty() && bytes + size > PAGE_BYTES) {
            break;
        }
        bytes += size;
        page.refs.push(listed.clone());
    }
    let next = start + page.refs.len();
    if next < all.len() {
        page.cursor = Some(next as u64);
    }
    Ok(page)
}

/// One `for-each-ref` line, as VS Code's reference and branch parsers read theirs.
fn parse_ref(line: &str) -> Option<Ref> {
    let mut fields = line.split('\0');
    let refname = fields.next()?;
    let object = fields.next()?;
    let peeled = fields.next().unwrap_or_default();
    let short = fields.next().unwrap_or_default();
    let track = fields.next().unwrap_or_default();
    let remote_name = fields.next().unwrap_or_default();
    let remote_ref = fields.next().unwrap_or_default();
    if !command::is_object_name(object) {
        return None;
    }
    let (name, kind, commit, upstream) = if let Some(name) = refname.strip_prefix("refs/heads/") {
        let upstream = (!short.is_empty() && track != "[gone]").then(|| {
            let (remote, branch) = short.split_once('/').unwrap_or(("", short));
            RefUpstream {
                remote: if remote_name.is_empty() {
                    remote
                } else {
                    remote_name
                }
                .to_owned(),
                name: remote_ref
                    .strip_prefix("refs/heads/")
                    .unwrap_or(branch)
                    .to_owned(),
            }
        });
        (name, RefKind::Head, object, upstream)
    } else if let Some(name) = refname.strip_prefix("refs/remotes/") {
        name.split_once('/')
            .filter(|(remote, branch)| !remote.is_empty() && !branch.is_empty())?;
        (name, RefKind::RemoteHead, object, None)
    } else {
        let name = refname.strip_prefix("refs/tags/")?;
        let commit = if command::is_object_name(peeled) {
            peeled
        } else {
            object
        };
        (name, RefKind::Tag, commit, None)
    };
    let fits = |text: &str| !text.is_empty() && text.len() <= TEXT_LIMIT;
    let upstream = upstream.filter(|upstream| fits(&upstream.remote) && fits(&upstream.name));
    fits(name).then(|| Ref {
        name: name.to_owned(),
        kind,
        commit: commit.to_owned(),
        upstream,
    })
}

/// The repository's remotes as its configuration names them, in order of first appearance:
/// the first URL fetches, and the first push URL, else the first URL, pushes, as VS Code reads
/// them from the configuration file.
pub(super) fn remotes(
    found: &Found,
    limits: &Limits,
    output: &Mutex<GitOutput>,
) -> Result<Vec<Remote>, GitError> {
    let mut command = found.command(Access::Read, None);
    command.args([
        "config",
        "--local",
        "--includes",
        "-z",
        "--get-regexp",
        r"^remote\.",
    ]);
    let ran = runner::run(command, Vec::new(), limits, output)?;
    if ran.exited_with(1) {
        return Ok(Vec::new());
    }
    let ran = ran.checked(Classifier::General)?;
    let text = String::from_utf8_lossy(&ran.stdout);
    let mut remotes: Vec<Remote> = Vec::new();
    for entry in text.split('\0') {
        let Some((key, value)) = entry.split_once('\n') else {
            continue;
        };
        let Some((name, variable)) = key
            .strip_prefix("remote.")
            .and_then(|rest| rest.rsplit_once('.'))
        else {
            continue;
        };
        if name.is_empty() || name.len() > TEXT_LIMIT {
            continue;
        }
        let index = match remotes.iter().position(|remote| remote.name == name) {
            Some(index) => index,
            None if remotes.len() < REMOTE_LIMIT => {
                remotes.push(Remote {
                    name: name.to_owned(),
                    fetch_url: None,
                    push_url: None,
                });
                remotes.len() - 1
            }
            None => continue,
        };
        let Some(remote) = remotes.get_mut(index) else {
            continue;
        };
        match variable {
            "url" if remote.fetch_url.is_none() => remote.fetch_url = Some(value.to_owned()),
            "pushurl" if remote.push_url.is_none() => remote.push_url = Some(value.to_owned()),
            _ => {}
        }
    }
    for remote in &mut remotes {
        if remote.push_url.is_none() {
            remote.push_url.clone_from(&remote.fetch_url);
        }
    }
    Ok(remotes)
}

/// The identity a commit would take from the user's Git configuration.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct UserConfig {
    pub(super) name: Option<String>,
    pub(super) email: Option<String>,
}

/// `git.user-config`: `user.name` and `user.email` as Git resolves them for the repository, the
/// last value of each winning; a value past 1,024 bytes is left out.
pub(super) fn user_config(
    found: &Found,
    limits: &Limits,
    output: &Mutex<GitOutput>,
) -> Result<UserConfig, GitError> {
    let mut command = found.command(Access::Read, None);
    command.args(["config", "-z", "--get-regexp", r"^user\.(name|email)$"]);
    let ran = runner::run(command, Vec::new(), limits, output)?;
    let mut config = UserConfig::default();
    if ran.exited_with(1) {
        return Ok(config);
    }
    let ran = ran.checked(Classifier::General)?;
    let text = String::from_utf8_lossy(&ran.stdout);
    for entry in text.split('\0') {
        let Some((key, value)) = entry.split_once('\n') else {
            continue;
        };
        let value = (value.len() <= TEXT_LIMIT).then(|| value.to_owned());
        match key {
            "user.name" => config.name = value,
            "user.email" => config.email = value,
            _ => {}
        }
    }
    Ok(config)
}
