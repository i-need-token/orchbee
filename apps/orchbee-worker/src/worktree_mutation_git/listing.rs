//! Parses only the registered checkout facts needed by the Git mutation owner.
use super::{AbsoluteCwd, GitReference};

pub(super) struct Entry {
    pub(super) path: AbsoluteCwd,
    pub(super) branch: Option<GitReference>,
    pub(super) bare: bool,
}

pub(super) fn parse(bytes: &[u8]) -> Result<Vec<Entry>, ()> {
    if !bytes.ends_with(&[0, 0]) {
        return Err(());
    }
    let mut entries = Vec::new();
    let mut fields = Vec::new();
    for field in bytes.split(|byte| *byte == 0) {
        if field.is_empty() {
            if !fields.is_empty() {
                entries.push(record(&fields)?);
                fields.clear();
            }
        } else {
            fields.push(field);
        }
    }
    if entries.is_empty() {
        return Err(());
    }
    Ok(entries)
}

fn record(fields: &[&[u8]]) -> Result<Entry, ()> {
    let path = fields
        .first()
        .and_then(|field| field.strip_prefix(b"worktree "))
        .ok_or(())?;
    let path = AbsoluteCwd::new(std::str::from_utf8(path).map_err(|_| ())?).map_err(|_| ())?;
    let mut branch = None;
    let mut bare = false;
    let mut detached = false;
    let mut head = false;
    let mut seen = std::collections::BTreeSet::new();
    for field in &fields[1..] {
        let (key, value) = field.split_at(
            field
                .iter()
                .position(|byte| *byte == b' ')
                .unwrap_or(field.len()),
        );
        if !seen.insert(key) {
            return Err(());
        }
        match key {
            b"worktree" => return Err(()),
            b"HEAD" => {
                let value = value.strip_prefix(b" ").ok_or(())?;
                if !matches!(value.len(), 40 | 64)
                    || !value
                        .iter()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
                {
                    return Err(());
                }
                head = true;
            }
            b"branch" => {
                let value = value.strip_prefix(b" refs/heads/").ok_or(())?;
                branch = Some(
                    GitReference::new(std::str::from_utf8(value).map_err(|_| ())?)
                        .map_err(|_| ())?,
                );
            }
            b"bare" if value.is_empty() => bare = true,
            b"detached" if value.is_empty() => detached = true,
            b"bare" | b"detached" => return Err(()),
            _ => {}
        }
    }
    if (bare && (head || branch.is_some() || detached))
        || (!bare && (!head || branch.is_some() == detached))
    {
        return Err(());
    }
    Ok(Entry { path, branch, bare })
}
