//! The local source grammar: the existing Windows disk-absolute grammar, independent of the
//! compiling host, and on macOS and Linux also an absolute POSIX path, whose only separator is `/`.
use std::path::Path;

pub(super) struct LocalPath {
    text: String,
    /// An absolute POSIX path rather than a Windows disk-absolute one.
    posix: bool,
}

impl LocalPath {
    pub(super) fn parse(value: &str) -> Option<Self> {
        if value.len() > 4096 || value.contains(['\0', '\r', '\n']) {
            return None;
        }
        if cfg!(unix) && value.starts_with('/') {
            return (!value.split('/').any(|part| part == "..")).then(|| Self {
                text: value.to_owned(),
                posix: true,
            });
        }
        let bytes = value.as_bytes();
        if !matches!(bytes, [drive, b':', b'/' | b'\\', ..] if drive.is_ascii_alphabetic())
            || value[3..].split(['/', '\\']).any(|part| part == "..")
        {
            return None;
        }
        Some(Self {
            text: value.to_owned(),
            posix: false,
        })
    }

    /// `directory` joined with `name` in the grammar its spelling takes: `/` after an absolute POSIX
    /// directory on macOS and Linux, `\` otherwise.
    pub(super) fn join(directory: &str, name: &str) -> Option<Self> {
        let separator = if cfg!(unix) && directory.starts_with('/') {
            '/'
        } else {
            '\\'
        };
        Self::parse(&format!(
            "{}{separator}{name}",
            directory.trim_end_matches(['/', '\\'])
        ))
    }

    pub(super) fn as_path(&self) -> &Path {
        Path::new(&self.text)
    }

    /// The separators that end a component, and the length of the root that precedes the first.
    fn grammar(&self) -> (&'static [char], usize) {
        if self.posix {
            (&['/'], 1)
        } else {
            (&['/', '\\'], 3)
        }
    }

    /// Windows non-verbatim components and POSIX components alike skip repeated separators and
    /// current-directory parts.
    pub(super) fn parent_and_name(&self) -> Option<(&str, &str)> {
        let (separators, root) = self.grammar();
        let mut parent_end = root;
        let mut name = None;
        let mut start = root;
        for part in self.text[root..].split(separators) {
            let end = start + part.len();
            if !part.is_empty() && part != "." {
                if let Some((_, previous_end)) = name {
                    parent_end = previous_end;
                }
                name = Some((start, end));
            }
            start = end + 1;
        }
        name.map(|(start, end)| (&self.text[..parent_end], &self.text[start..end]))
    }

    /// The separator a join after `parent` adds: none after a separator, else the grammar's own.
    fn separator(&self, parent: &str) -> &'static str {
        let (separators, _) = self.grammar();
        match (parent.ends_with(separators), self.posix) {
            (true, _) => "",
            (false, true) => "/",
            (false, false) => "\\",
        }
    }

    /// The sibling named by this file's name followed by `suffix`, such as the signature document
    /// beside a manifest. The caller supplies a fixed portable suffix.
    pub(super) fn with_suffix(&self, suffix: &str) -> Option<Self> {
        let (parent, name) = self.parent_and_name()?;
        let separator = self.separator(parent);
        Some(Self {
            text: format!("{parent}{separator}{name}{suffix}"),
            posix: self.posix,
        })
    }

    /// The caller admits the portable relative artifact grammar before joining.
    pub(super) fn artifact(&self, relative: &str) -> Option<Self> {
        let (parent, _) = self.parent_and_name()?;
        let separator = self.separator(parent);
        // As with PathBuf::join, the external source's 4096-byte bound is not reapplied here.
        Some(Self {
            text: format!("{parent}{separator}{relative}"),
            posix: self.posix,
        })
    }
}
