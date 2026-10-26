//! The existing Windows disk-absolute source grammar, independent of the compiling host.
use std::path::Path;

pub(super) struct LocalPath(String);

impl LocalPath {
    pub(super) fn parse(value: &str) -> Option<Self> {
        let bytes = value.as_bytes();
        if value.len() > 4096
            || value.contains(['\0', '\r', '\n'])
            || !matches!(bytes, [drive, b':', b'/' | b'\\', ..] if drive.is_ascii_alphabetic())
            || value[3..].split(['/', '\\']).any(|part| part == "..")
        {
            return None;
        }
        Some(Self(value.to_owned()))
    }

    pub(super) fn as_path(&self) -> &Path {
        Path::new(&self.0)
    }

    /// Windows non-verbatim components skip repeated separators and current-directory parts.
    pub(super) fn parent_and_name(&self) -> Option<(&str, &str)> {
        let mut parent_end = 3;
        let mut name = None;
        let mut start = 3;
        for part in self.0[3..].split(['/', '\\']) {
            let end = start + part.len();
            if !part.is_empty() && part != "." {
                if let Some((_, previous_end)) = name {
                    parent_end = previous_end;
                }
                name = Some((start, end));
            }
            start = end + 1;
        }
        name.map(|(start, end)| (&self.0[..parent_end], &self.0[start..end]))
    }

    /// The caller admits the portable relative artifact grammar before joining.
    pub(super) fn artifact(&self, relative: &str) -> Option<Self> {
        let (parent, _) = self.parent_and_name()?;
        let separator = if parent.ends_with(['/', '\\']) {
            ""
        } else {
            "\\"
        };
        // As with PathBuf::join, the external source's 4096-byte bound is not reapplied here.
        Some(Self(format!("{parent}{separator}{relative}")))
    }
}
