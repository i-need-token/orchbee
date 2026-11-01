//! Application-private Client Connection transport for the Orchbee native application.

mod local_client;

pub use local_client::{
    LocalClient, LocalClientConnectError, LocalClientOrigin, LocalClientShutdown,
    LocalOperationClock, OutcomeUnknownReason, ResultKind, RetainedCarrier, SubmissionError,
    exchange_wake_fault, exchange_wake_ready,
};

pub mod agent {
    pub use crate::local_client::agent::{
        AgentExchangeError, AgentExchangeFailure, AgentReadOperation,
    };
}

pub mod cancellation {
    pub use crate::local_client::cancellation::{Cancellation, OwnedCancellation};
}

pub mod multiplex {
    pub use crate::local_client::multiplex::{
        StreamEvent, Subscription, ViewerReadyWatch, terminal_failure,
    };
}

pub mod subscription {
    pub use crate::local_client::subscription::{ChangeEvent, ChangeSubscription, Request, notify};
}

pub mod workbench {
    pub use crate::local_client::workbench::{
        GroupCloseExchangeError, LaunchProfileCatalogReadError, LaunchProfileCatalogReadFailure,
        PublicationReadError, PublicationReadFailure, SnapshotReadError, SnapshotReadFailure,
        WorkbenchTabCloseAccepted, WorkbenchTabCloseFailure, WorkbenchTabCloseNotSubmitted,
        WorkbenchTabClosePrecommit, WorkbenchTabCloseReadError, WorkbenchTabCloseRequest,
        WorkbenchTabCloseResumeSubmission, WorkbenchTabCloseSubmission, WorkbenchTabCreateCommit,
        WorkbenchTabCreateNotSubmitted, WorkbenchTabCreateOutcomeUnknownReason,
        WorkbenchTabCreatePrecommit, WorkbenchTabCreateRequest, WorkbenchTabCreateSubmission,
        WorkbenchTabMoveCommit, WorkbenchTabMoveNotSubmitted, WorkbenchTabMoveOutcomeUnknownReason,
        WorkbenchTabMovePrecommit, WorkbenchTabMoveRequest, WorkbenchTabMoveSubmission,
        WorkbenchWorkspaceCloseAccepted, WorkbenchWorkspaceCloseFailure,
        WorkbenchWorkspaceCloseNotSubmitted, WorkbenchWorkspaceClosePrecommit,
        WorkbenchWorkspaceCloseReadError, WorkbenchWorkspaceCloseRequest,
        WorkbenchWorkspaceCloseResumeSubmission, WorkbenchWorkspaceCloseSubmission,
        WorkspaceGitError, WorktreeExchangeError, WorktreeListReadError, WorktreeListReadFailure,
    };

    pub mod agent_integration {
        pub use crate::local_client::workbench::agent_integration::AgentIntegrationJobEnd;
    }

    pub mod client_view {
        pub use crate::local_client::workbench::client_view::ClientViewExchangeError;
    }

    pub mod creation {
        pub use crate::local_client::workbench::creation::{CreationRequest, CreationSubmission};
    }

    pub mod label_rename {
        pub use crate::local_client::workbench::label_rename::{
            WorkbenchLabelRenameNotSubmitted, WorkbenchLabelRenameOutcomeUnknownReason,
            WorkbenchLabelRenamePrecommit, WorkbenchLabelRenameRequest,
            WorkbenchLabelRenameSubmission,
        };
    }

    pub mod launch_profile {
        pub use crate::local_client::workbench::launch_profile::{
            LaunchProfileMutationNotSubmitted, LaunchProfileOutcomeUnknownReason,
            WorkbenchLaunchProfileCreateRequest, WorkbenchLaunchProfileDeleteRequest,
            WorkbenchLaunchProfileMutationPrecommit, WorkbenchLaunchProfileMutationSubmission,
        };
    }

    pub mod metadata_exchange {
        pub use crate::local_client::workbench::metadata_exchange::{
            MetadataExchangeError, MetadataExchangeReason,
        };
    }

    pub mod panel_close {
        pub use crate::local_client::workbench::panel_close::{
            WorkbenchPanelCloseAccepted, WorkbenchPanelCloseFailure,
            WorkbenchPanelCloseNotSubmitted, WorkbenchPanelClosePrecommit,
            WorkbenchPanelCloseRequest, WorkbenchPanelCloseSubmission,
        };
    }

    pub mod panel_input {
        pub use crate::local_client::workbench::panel_input::{
            WorkbenchPanelInputNotSubmitted, WorkbenchPanelInputOutcomeUnknownReason,
            WorkbenchPanelInputPrecommit, WorkbenchPanelInputRequest,
            WorkbenchPanelInputSubmission,
        };
    }

    pub mod panel_placement {
        pub use crate::local_client::workbench::panel_placement::{
            WorkbenchPanelPlacementCommit, WorkbenchPanelPlacementNotSubmitted,
            WorkbenchPanelPlacementOutcomeUnknownReason, WorkbenchPanelPlacementPrecommit,
            WorkbenchPanelPlacementRequest, WorkbenchPanelPlacementSubmission,
        };
    }

    pub mod panel_relocate {
        pub use crate::local_client::workbench::panel_relocate::{
            RelocatedSplitIds, WorkbenchPanelRelocateCommit, WorkbenchPanelRelocateNotSubmitted,
            WorkbenchPanelRelocateOutcomeUnknownReason, WorkbenchPanelRelocatePrecommit,
            WorkbenchPanelRelocateRequest, WorkbenchPanelRelocateSubmission,
        };
    }

    pub mod panel_split {
        pub use crate::local_client::workbench::panel_split::{
            WorkbenchPanelSplitCommit, WorkbenchPanelSplitNotSubmitted,
            WorkbenchPanelSplitOutcomeUnknownReason, WorkbenchPanelSplitPrecommit,
            WorkbenchPanelSplitRequest, WorkbenchPanelSplitSubmission,
        };
    }

    pub mod split_ratio {
        pub use crate::local_client::workbench::split_ratio::SplitRatioFailure;
    }

    pub mod workspace_launch {
        pub use crate::local_client::workbench::workspace_launch::{
            WorkspaceLaunchCommit, WorkspaceLaunchRequest, WorkspaceLaunchSubmission,
        };
    }

    pub mod workspace_setup {
        pub use crate::local_client::workbench::workspace_setup::{
            WorkbenchSetupCommit, WorkbenchSetupNotSubmitted, WorkbenchSetupOutcomeUnknownReason,
            WorkbenchSetupPrecommit, WorkbenchSetupRequest, WorkbenchSetupSubmission,
        };
    }

    pub mod worktree_query {
        pub use crate::local_client::workbench::worktree_query::WorktreeQueryError;
    }
}

#[cfg(any(test, feature = "test-support"))]
pub use local_client::test_support;
