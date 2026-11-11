//! The changes the person asked for, kept in order: one in flight and the rest waiting their turn.
//! Each is sent once the previous one is answered, so the Server still gets one row's keys per
//! edit, and nothing the person did is dropped silently.
use super::{Row, Token};
use orchbee_workbench_protocol::{SettingChange, UserSettingsValues};
use serde_json::{Map, Value};
use std::collections::VecDeque;

/// Changes that may wait behind the one in flight; a newer request beyond them is refused.
pub(super) const WAITING_LIMIT: usize = 16;

/// One row's change: the edit it sends and what the row shows until the Server answers.
#[derive(Clone, Debug)]
pub(super) struct Change {
    row: Row,
    changes: Vec<SettingChange>,
    /// For a text key, the text asked for (`None` restores the default).
    text: Option<Option<String>>,
    /// The members of the serialized shown values this change sets.
    overlay: Map<String, Value>,
}

impl Change {
    /// A change to `row` that sends `changes` and turns the shown `before` into `after`.
    pub(super) fn new(
        row: Row,
        changes: Vec<SettingChange>,
        text: Option<Option<String>>,
        before: &UserSettingsValues,
        after: &UserSettingsValues,
    ) -> Self {
        let mut overlay = Map::new();
        if let (Ok(Value::Object(before)), Ok(Value::Object(after))) =
            (serde_json::to_value(before), serde_json::to_value(after))
        {
            for (member, value) in after {
                if before.get(&member) != Some(&value) {
                    overlay.insert(member, value);
                }
            }
        }
        Self {
            row,
            changes,
            text,
            overlay,
        }
    }

    pub(super) fn changes(&self) -> &[SettingChange] {
        &self.changes
    }

    /// A newer request for the same row, made before this one was sent, replaces it.
    fn replace_with(&mut self, newer: Self) {
        for change in newer.changes {
            let key = key_of(&change);
            if let Some(existing) = self
                .changes
                .iter_mut()
                .find(|existing| key_of(existing) == key)
            {
                *existing = change;
            } else {
                self.changes.push(change);
            }
        }
        if newer.text.is_some() {
            self.text = newer.text;
        }
        self.overlay.extend(newer.overlay);
    }
}

fn key_of(change: &SettingChange) -> &str {
    match change {
        SettingChange::Set { key, .. } | SettingChange::Restore { key } => key.as_str(),
    }
}

#[derive(Debug, Default)]
pub(super) struct Changes {
    sent: Option<(Token, Change)>,
    waiting: VecDeque<Change>,
}

impl Changes {
    pub(super) const fn in_flight(&self) -> bool {
        self.sent.is_some()
    }

    /// The row has a change in flight or waiting.
    pub(super) fn waits(&self, row: Row) -> bool {
        self.all().any(|change| change.row == row)
    }

    /// The text a text-key row last asked for, if it has a change in flight or waiting.
    pub(super) fn text(&self, row: Row) -> Option<Option<&str>> {
        self.all()
            .filter(|change| change.row == row)
            .filter_map(|change| change.text.as_ref())
            .last()
            .map(Option::as_deref)
    }

    /// Keeps a request in order. A row that already has a waiting change has it replaced; a
    /// request beyond the waiting limit is refused and returns `false`.
    pub(super) fn push(&mut self, change: Change) -> bool {
        if let Some(waiting) = self
            .waiting
            .iter_mut()
            .find(|waiting| waiting.row == change.row)
        {
            waiting.replace_with(change);
            return true;
        }
        if self.waiting.len() >= WAITING_LIMIT {
            return false;
        }
        self.waiting.push_back(change);
        true
    }

    /// Whether a waiting change can be sent now: nothing is in flight.
    pub(super) fn ready(&self) -> bool {
        self.sent.is_none() && !self.waiting.is_empty()
    }

    /// Sends the oldest waiting change under `token`.
    pub(super) fn send(&mut self, token: Token) -> Option<&Change> {
        if self.sent.is_some() {
            return None;
        }
        let change = self.waiting.pop_front()?;
        Some(&self.sent.insert((token, change)).1)
    }

    /// The Server answered `token`. A refusal also drops the waiting changes of the same row,
    /// which were based on a value that never existed. Returns whether `token` was in flight.
    pub(super) fn settle(&mut self, token: Token, refused: bool) -> bool {
        if self.sent.as_ref().is_none_or(|(sent, _)| *sent != token) {
            return false;
        }
        let (_, change) = self.sent.take().expect("the change in flight");
        if refused {
            self.waiting.retain(|waiting| waiting.row != change.row);
        }
        true
    }

    /// Drops every waiting change; returns whether there was one.
    pub(super) fn clear_waiting(&mut self) -> bool {
        let any = !self.waiting.is_empty();
        self.waiting.clear();
        any
    }

    /// The confirmed values with every change in flight or waiting laid over them in order, so
    /// each row shows the last value asked for.
    pub(super) fn shown(&self, confirmed: &UserSettingsValues) -> UserSettingsValues {
        if self.sent.is_none() && self.waiting.is_empty() {
            return confirmed.clone();
        }
        let Ok(Value::Object(mut values)) = serde_json::to_value(confirmed) else {
            return confirmed.clone();
        };
        for change in self.all() {
            values.extend(change.overlay.clone());
        }
        serde_json::from_value(Value::Object(values)).unwrap_or_else(|_| confirmed.clone())
    }

    fn all(&self) -> impl Iterator<Item = &Change> {
        self.sent
            .iter()
            .map(|(_, change)| change)
            .chain(&self.waiting)
    }
}
