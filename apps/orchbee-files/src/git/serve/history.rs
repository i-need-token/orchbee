//! The `files-history` side of the Git dispatch: the history reads, whose continuations name the
//! index the next page starts at.

use super::{
    super::{
        blame::BlameQuery,
        history::{ChangesQuery, LogQuery},
        records,
    },
    Service, replying,
};
use crate::calls::Call;
use orchbee_files_protocol::{GitBlame, GitChanges, GitLog, GitMergeBase, GitName, GitStashes};

impl Service {
    /// `git.log`.
    pub(crate) fn log(&self, call: Call, request: &GitLog) {
        let Some((git, call)) = self.engine(call) else {
            return;
        };
        let query = LogQuery {
            revisions: request
                .revisions
                .as_ref()
                .map_or_else(Vec::new, |revisions| {
                    revisions.as_slice().iter().map(name).collect()
                }),
            all: request.all,
            path: request.path.as_ref().map(|path| path.as_str().to_owned()),
            follow: request.follow,
            author: request
                .author
                .as_ref()
                .map(|author| author.as_str().to_owned()),
            grep: request.grep.as_ref().map(|grep| grep.as_str().to_owned()),
            statistics: request.statistics,
            // At most 1,000, which every target's `usize` holds.
            limit: usize::try_from(request.limit.get()).unwrap_or(usize::MAX),
            skip: request.cursor.map_or(0, records::page_index),
        };
        let cancel = call.cancellation();
        git.log(
            request.workspace_id,
            request.repository.as_str().to_owned(),
            query,
            cancel,
            replying(call, records::log),
        );
    }

    /// `git.blame`.
    pub(crate) fn blame(&self, call: Call, request: &GitBlame) {
        let Some((git, call)) = self.engine(call) else {
            return;
        };
        let query = BlameQuery {
            path: request.path.as_str().to_owned(),
            commit: request
                .commit
                .as_ref()
                .map(|commit| commit.as_str().to_owned()),
            ignore_whitespace: request.ignore_whitespace,
        };
        let cancel = call.cancellation();
        git.blame(
            request.workspace_id,
            request.repository.as_str().to_owned(),
            query,
            request.cursor.map_or(0, records::page_index),
            cancel,
            replying(call, records::blame),
        );
    }

    /// `git.changes`.
    pub(crate) fn changes(&self, call: Call, request: &GitChanges) {
        let Some((git, call)) = self.engine(call) else {
            return;
        };
        let query = ChangesQuery {
            from: request
                .from
                .as_ref()
                .map(|commit| commit.as_str().to_owned()),
            to: request.to.as_str().to_owned(),
        };
        let cancel = call.cancellation();
        git.changes(
            request.workspace_id,
            request.repository.as_str().to_owned(),
            query,
            request.cursor.map_or(0, records::page_index),
            cancel,
            replying(call, records::changes),
        );
    }

    /// `git.merge-base`.
    pub(crate) fn merge_base(&self, call: Call, request: &GitMergeBase) {
        let Some((git, call)) = self.engine(call) else {
            return;
        };
        let cancel = call.cancellation();
        git.merge_base(
            request.workspace_id,
            request.repository.as_str().to_owned(),
            name(&request.first),
            name(&request.second),
            cancel,
            replying(call, records::merge_base),
        );
    }

    /// `git.stashes`.
    pub(crate) fn stashes(&self, call: Call, request: &GitStashes) {
        let Some((git, call)) = self.engine(call) else {
            return;
        };
        let cancel = call.cancellation();
        git.stashes(
            request.workspace_id,
            request.repository.as_str().to_owned(),
            request.cursor.map_or(0, records::page_index),
            cancel,
            replying(call, records::stashes),
        );
    }
}

fn name(name: &GitName) -> String {
    name.as_str().to_owned()
}
