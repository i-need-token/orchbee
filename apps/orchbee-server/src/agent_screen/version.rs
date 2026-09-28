//! Dotted numeric rule versions shared by admission, cache selection and updating.
use std::cmp::Ordering;
#[derive(Eq, PartialEq)]
pub(super) struct Version(Vec<u64>);
impl Version {
    pub(super) fn parse(value: &str) -> Option<Self> {
        let value = value.trim();
        if value.is_empty() || value.len() > 128 {
            return None;
        }
        let mut parts = value
            .split('.')
            .map(|part| {
                (!part.is_empty() && part.bytes().all(|c| c.is_ascii_digit()))
                    .then(|| part.parse::<u64>().ok())
                    .flatten()
            })
            .collect::<Option<Vec<_>>>()?;
        while parts.last() == Some(&0) {
            parts.pop();
        }
        Some(Self(parts))
    }
}
impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}
impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
