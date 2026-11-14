//! Correlated, single-flight copy motion and search reads on an independent connection.

use crate::{
    terminal_content::ContentSource,
    terminal_copy::{
        CopyMotion, CopySearchCase, CopySearchDirection, CopySearchResult, TerminalTextPoint,
        TerminalTextRange,
    },
    terminal_copy_mode::{EpisodeId, QueryId},
};

pub(crate) mod lane;

/// Exact copy episode and query identities carried through the worker lane.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct QueryIdentity {
    pub(crate) episode: EpisodeId,
    pub(crate) query: QueryId,
}

/// One bounded read-only query.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Request {
    pub(crate) identity: QueryIdentity,
    pub(crate) source: ContentSource,
    pub(crate) action: Action,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Action {
    Motion {
        cursor: TerminalTextPoint,
        motion: CopyMotion,
    },
    Search {
        cursor: TerminalTextPoint,
        query: Box<str>,
        case: CopySearchCase,
        direction: CopySearchDirection,
        previous: Option<TerminalTextRange>,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Value {
    Motion(TerminalTextPoint),
    Search(CopySearchResult),
}

/// A terminal result for exactly one admitted identity and complete source fence.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Outcome {
    pub(crate) identity: QueryIdentity,
    pub(crate) source: ContentSource,
    pub(crate) result: Result<Value, Failure>,
}

/// Immediate admission never queues behind an in-flight query.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Admission {
    Accepted,
    Busy,
    Unavailable,
    LimitExceeded,
}

/// One visible, non-retrying query refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Failure {
    SourceChanged,
    LimitExceeded,
    PatternTooComplex,
    IdentityUnavailable,
    TransportUnavailable,
    ProtocolInvalid,
    DetachUnknown,
    Unexpected,
}
