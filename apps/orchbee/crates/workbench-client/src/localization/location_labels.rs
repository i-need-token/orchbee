use super::Locale;

/// The kind of location named by rename feedback.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LocationLabelTarget {
    Project,
    Workspace,
}

/// Observed rename result; committed and unknown outcomes remain distinct.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LocationLabelStatus {
    Unavailable,
    NotApplied,
    /// Refused because the owner changed elsewhere; the typed text is kept for retry.
    Stale,
    Unchanged,
    Renamed,
    CommittedNeedRefresh,
    OutcomeUnknown,
}

impl Locale {
    pub(crate) const fn unnamed_workspace(self) -> &'static str {
        match self {
            Self::EnUs => "Unnamed Workspace",
            Self::ZhCn => "未命名工作区",
        }
    }

    pub(crate) fn workspace_display_label(self, label: &str) -> &str {
        if label.trim().is_empty() {
            self.unnamed_workspace()
        } else {
            label
        }
    }

    /// What a rename says, if anything: a rename that landed, or changed nothing, shows itself.
    pub(crate) const fn location_label_status(
        self,
        target: LocationLabelTarget,
        status: LocationLabelStatus,
    ) -> Option<&'static str> {
        use LocationLabelStatus as Status;
        use LocationLabelTarget as Target;
        Some(match (self, target, status) {
            (_, _, Status::Unchanged | Status::Renamed) => return None,
            (_, _, Status::OutcomeUnknown) => self.outcome_unknown(),
            (
                Self::EnUs,
                Target::Project,
                Status::Unavailable | Status::NotApplied | Status::Stale,
            ) => "Couldn't rename the Project.",
            (
                Self::ZhCn,
                Target::Project,
                Status::Unavailable | Status::NotApplied | Status::Stale,
            ) => "没能重命名项目。",
            (
                Self::EnUs,
                Target::Workspace,
                Status::Unavailable | Status::NotApplied | Status::Stale,
            ) => "Couldn't rename the Workspace.",
            (
                Self::ZhCn,
                Target::Workspace,
                Status::Unavailable | Status::NotApplied | Status::Stale,
            ) => "没能重命名工作区。",
            // The rename landed but this window cannot show it until the person refreshes.
            (Self::EnUs, _, Status::CommittedNeedRefresh) => "Couldn't show the new name.",
            (Self::ZhCn, _, Status::CommittedNeedRefresh) => "没能显示新名称。",
        })
    }

    /// After the connection ends, a rename that landed says so, so the person does not repeat it.
    pub(crate) const fn location_rename_committed_fact(self) -> &'static str {
        match self {
            Self::EnUs => "The new name was saved.",
            Self::ZhCn => "新名称已保存。",
        }
    }

    pub(crate) const fn location_rename_title(self, project: bool) -> &'static str {
        match (self, project) {
            (Self::EnUs, true) => "Rename Project",
            (Self::ZhCn, true) => "重命名项目",
            (Self::EnUs, false) => "Rename Workspace",
            (Self::ZhCn, false) => "重命名工作区",
        }
    }
}
