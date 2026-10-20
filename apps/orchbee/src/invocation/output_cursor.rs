//! Opaque, terminal-scoped boundaries for newly produced complete physical rows.

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct OutputCursor {
    pub(crate) terminal_id: String,
    pub(crate) host_epoch: String,
    pub(crate) generation: u64,
    pub(crate) output_epoch: u64,
    pub(crate) revision: u64,
    pub(crate) row: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum OutputAfter {
    Now,
    Cursor(OutputCursor),
}

impl OutputAfter {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        if value == "now" {
            return Some(Self::Now);
        }
        let fields: Vec<_> = value.split(':').collect();
        let [
            version,
            terminal_id,
            host_epoch,
            generation,
            output_epoch,
            revision,
            row,
        ] = fields.as_slice()
        else {
            return None;
        };
        if *version != "o1" || !identity(terminal_id) || !identity(host_epoch) {
            return None;
        }
        let generation = decimal(generation)?;
        let output_epoch = decimal(output_epoch)?;
        if generation == 0 || output_epoch == 0 {
            return None;
        }
        Some(Self::Cursor(OutputCursor {
            terminal_id: (*terminal_id).to_owned(),
            host_epoch: (*host_epoch).to_owned(),
            generation,
            output_epoch,
            revision: decimal(revision)?,
            row: decimal(row)?,
        }))
    }
}

impl OutputCursor {
    pub(crate) fn encode(&self) -> String {
        format!(
            "o1:{}:{}:{}:{}:{}:{}",
            self.terminal_id,
            self.host_epoch,
            self.generation,
            self.output_epoch,
            self.revision,
            self.row
        )
    }
}

fn identity(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        && value.bytes().any(|byte| byte != b'0')
}

fn decimal(value: &str) -> Option<u64> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    value.parse().ok()
}

#[cfg(test)]
mod tests;
