//! Request and outcome values of Client Connection exchanges that Client state owners build and
//! settle. Transport handles, clocks, and submission stay behind `local_client`.

pub(crate) mod workbench {
    pub(crate) use orchbee_local_client::workbench::{
        WorkbenchTabCloseAccepted, WorkbenchTabClosePrecommit, WorkbenchTabCreateCommit,
        WorkbenchTabCreatePrecommit, WorkbenchTabCreateRequest, WorkbenchTabMoveCommit,
        WorkbenchTabMovePrecommit, WorkbenchWorkspaceCloseAccepted,
    };

    pub(crate) mod label_rename {
        pub(crate) use orchbee_local_client::workbench::label_rename::{
            WorkbenchLabelRenamePrecommit, WorkbenchLabelRenameRequest,
        };
    }

    pub(crate) mod panel_close {
        pub(crate) use orchbee_local_client::workbench::panel_close::{
            WorkbenchPanelCloseAccepted, WorkbenchPanelClosePrecommit,
        };
    }

    pub(crate) mod panel_placement {
        pub(crate) use orchbee_local_client::workbench::panel_placement::{
            WorkbenchPanelPlacementCommit, WorkbenchPanelPlacementPrecommit,
        };
    }

    pub(crate) mod panel_split {
        pub(crate) use orchbee_local_client::workbench::panel_split::{
            WorkbenchPanelSplitCommit, WorkbenchPanelSplitPrecommit,
        };
    }

    pub(crate) mod workspace_launch {
        pub(crate) use orchbee_local_client::workbench::workspace_launch::WorkspaceLaunchCommit;
    }

    pub(crate) mod workspace_setup {
        pub(crate) use orchbee_local_client::workbench::workspace_setup::{
            WorkbenchSetupCommit, WorkbenchSetupPrecommit, WorkbenchSetupRequest,
        };
    }
}
