use super::ReadyAttachment;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) enum CreateOriginKind {
    Ready,
    Empty,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum CreateAnchor {
    Ready(ReadyAttachment),
    Empty,
}

impl CreateAnchor {
    pub(super) const fn kind(&self) -> CreateOriginKind {
        match self {
            Self::Ready(_) => CreateOriginKind::Ready,
            Self::Empty => CreateOriginKind::Empty,
        }
    }
    pub(super) const fn ready(&self) -> Option<&ReadyAttachment> {
        match self {
            Self::Ready(ready) => Some(ready),
            Self::Empty => None,
        }
    }
    pub(super) fn ready_mut(&mut self) -> Option<&mut ReadyAttachment> {
        match self {
            Self::Ready(ready) => Some(ready),
            Self::Empty => None,
        }
    }
}
