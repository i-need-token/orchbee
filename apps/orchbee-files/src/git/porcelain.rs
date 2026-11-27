//! Reads `git status --porcelain=v2 -z --branch` output as it arrives, split anywhere: the branch
//! headers and the entries in Git's order, up to a limit past which the rest is not wanted.

/// A record longer than this is not one Git writes for a path any filesystem holds.
const RECORD_LIMIT: usize = 256 * 1024;

/// HEAD and its upstream as the `# branch.*` headers report them.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct Branch {
    /// The commit HEAD names; none while its branch has no commit (`(initial)`).
    pub(super) commit: Option<String>,
    /// The branch HEAD names; none while HEAD is detached.
    pub(super) head: Option<String>,
    pub(super) upstream: Option<String>,
    /// Commits ahead of and behind the upstream, present only while the upstream exists.
    pub(super) divergence: Option<(u64, u64)>,
}

/// One status entry. `x` and `y` are Git's index and working tree letters, `.` for unchanged; the
/// paths are relative to the repository and spelled as Git spells them, with `/`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Entry {
    pub(super) kind: EntryKind,
    pub(super) x: u8,
    pub(super) y: u8,
    pub(super) submodule: bool,
    pub(super) path: Vec<u8>,
    /// The path a rename or copy started from.
    pub(super) original: Option<Vec<u8>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum EntryKind {
    Changed,
    Renamed,
    Unmerged,
    Untracked,
    Ignored,
}

/// A whole status: entries past the reader's limit are left out and mark it truncated.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct Status {
    pub(super) branch: Branch,
    pub(super) entries: Vec<Entry>,
    pub(super) truncated: bool,
}

/// Output that is not porcelain v2 as this reader knows it. It carries nothing of the output, which
/// may hold paths.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Malformed;

/// Whether the reader takes more output.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Progress {
    More,
    /// An entry past the limit arrived: the status is complete as truncated, and the command can be
    /// ended.
    Full,
}

pub(super) struct StatusReader {
    limit: usize,
    record: Vec<u8>,
    /// A rename or copy whose original path is the next record.
    renamed: Option<Entry>,
    status: Status,
}

impl StatusReader {
    pub(super) fn new(limit: usize) -> Self {
        Self {
            limit,
            record: Vec::new(),
            renamed: None,
            status: Status::default(),
        }
    }

    /// Takes the next bytes of output, which may end anywhere.
    pub(super) fn read(&mut self, mut bytes: &[u8]) -> Result<Progress, Malformed> {
        if self.status.truncated {
            return Ok(Progress::Full);
        }
        while let Some(end) = bytes.iter().position(|byte| *byte == 0) {
            let (record, rest) = bytes.split_at(end);
            self.extend(record)?;
            bytes = rest.get(1..).unwrap_or_default();
            let record = std::mem::take(&mut self.record);
            if self.complete(record)? == Progress::Full {
                return Ok(Progress::Full);
            }
        }
        self.extend(bytes)?;
        Ok(Progress::More)
    }

    /// Ends the output. Unless the status is truncated, its last record must be whole.
    pub(super) fn finish(self) -> Result<Status, Malformed> {
        if !self.status.truncated && (!self.record.is_empty() || self.renamed.is_some()) {
            return Err(Malformed);
        }
        Ok(self.status)
    }

    fn extend(&mut self, bytes: &[u8]) -> Result<(), Malformed> {
        if self.record.len() + bytes.len() > RECORD_LIMIT {
            return Err(Malformed);
        }
        self.record.extend_from_slice(bytes);
        Ok(())
    }

    fn complete(&mut self, record: Vec<u8>) -> Result<Progress, Malformed> {
        if let Some(mut entry) = self.renamed.take() {
            if record.is_empty() {
                return Err(Malformed);
            }
            entry.original = Some(record);
            return Ok(self.push(entry));
        }
        match record.first() {
            Some(b'#') => {
                self.header(&record)?;
                Ok(Progress::More)
            }
            Some(b'2') => {
                self.renamed = Some(entry(&record, 10, EntryKind::Renamed)?);
                Ok(Progress::More)
            }
            Some(b'1') => Ok(self.push(entry(&record, 9, EntryKind::Changed)?)),
            Some(b'u') => Ok(self.push(entry(&record, 11, EntryKind::Unmerged)?)),
            Some(kind @ (b'?' | b'!')) => {
                let path = record.strip_prefix(&[*kind, b' ']).ok_or(Malformed)?;
                if path.is_empty() {
                    return Err(Malformed);
                }
                Ok(self.push(Entry {
                    kind: if *kind == b'?' {
                        EntryKind::Untracked
                    } else {
                        EntryKind::Ignored
                    },
                    x: *kind,
                    y: *kind,
                    submodule: false,
                    path: path.to_vec(),
                    original: None,
                }))
            }
            _ => Err(Malformed),
        }
    }

    fn push(&mut self, entry: Entry) -> Progress {
        if self.status.entries.len() == self.limit {
            self.status.truncated = true;
            return Progress::Full;
        }
        self.status.entries.push(entry);
        Progress::More
    }

    fn header(&mut self, record: &[u8]) -> Result<(), Malformed> {
        let text = String::from_utf8_lossy(record);
        let branch = &mut self.status.branch;
        if let Some(value) = text.strip_prefix("# branch.oid ") {
            branch.commit = (value != "(initial)").then(|| value.to_owned());
        } else if let Some(value) = text.strip_prefix("# branch.head ") {
            branch.head = (value != "(detached)").then(|| value.to_owned());
        } else if let Some(value) = text.strip_prefix("# branch.upstream ") {
            branch.upstream = Some(value.to_owned());
        } else if let Some(value) = text.strip_prefix("# branch.ab ") {
            let (ahead, behind) = value.split_once(' ').ok_or(Malformed)?;
            let count = |value: Option<&str>| {
                value
                    .and_then(|digits| digits.parse::<u64>().ok())
                    .ok_or(Malformed)
            };
            branch.divergence = Some((
                count(ahead.strip_prefix('+'))?,
                count(behind.strip_prefix('-'))?,
            ));
        }
        // Other headers, such as `# stash`, carry nothing a summary shows.
        Ok(())
    }
}

/// An entry record of `fields` space-separated fields, of which the last, the path, may itself
/// hold spaces. The second field holds the two status letters and the third the submodule state.
fn entry(record: &[u8], fields: usize, kind: EntryKind) -> Result<Entry, Malformed> {
    let mut parts = record.splitn(fields, |byte| *byte == b' ');
    let _kind = parts.next();
    let letters = parts.next().ok_or(Malformed)?;
    let submodule = parts.next().ok_or(Malformed)?;
    let path = parts.nth(fields - 4).ok_or(Malformed)?;
    let [x, y] = <[u8; 2]>::try_from(letters).map_err(|_| Malformed)?;
    if submodule.len() != 4 || path.is_empty() {
        return Err(Malformed);
    }
    Ok(Entry {
        kind,
        x,
        y,
        submodule: submodule.first() == Some(&b'S'),
        path: path.to_vec(),
        original: None,
    })
}

#[cfg(test)]
mod tests;
