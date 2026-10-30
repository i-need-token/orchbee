//! Workbench Client runtime for the native application: projected Workbench state, views,
//! notices, input dispatch, terminal presentation, Agent and machine workflows, and Client
//! communication lanes. The `orchbee` binary owns process entry, command-line grammar, and the
//! terminal shell loop; this crate never depends on them.

mod action_menu;
mod action_projection;
mod agent_provider;
mod agent_runtime;
mod agent_start;
mod background_thread;
mod client_actions;
mod client_context;
mod client_view;
mod exchange_values;
mod host_appearance;
mod host_capabilities;
mod host_graphics;
mod local_activation;
mod local_client;
mod localization;
mod machine_agents;
mod machine_discovery;
mod machine_observation;
mod machine_picker;
mod machine_spaces;
mod machine_ssh;
mod machine_tunnel;
mod machines;
mod native_settings;
mod notifications;
mod onboarding;
mod startup_status;
mod terminal_buffer_text;
mod terminal_content;
mod terminal_copy;
mod terminal_copy_mode;
mod terminal_copy_query;
mod terminal_device;
mod terminal_events;
mod terminal_graphics;
mod terminal_host_appearance;
mod terminal_input;
mod terminal_lifecycle;
mod terminal_link_activation;
mod terminal_links;
mod terminal_live_view;
mod terminal_pointer_admission;
mod terminal_projection;
mod terminal_scrollbar;
mod terminal_text;
mod terminal_title;
mod termination_signal;
mod transient_notice;
mod tui_capabilities;
mod tui_composition;
mod tui_graphics;
mod tui_layout;
mod tui_pointer;
mod tui_presentation;
mod tui_render;
mod tui_shortcut_help;
mod tui_theme;
#[cfg(windows)]
mod windows_console_input;
#[cfg(windows)]
mod windows_signal;
mod workbench_client;
mod worktree_destination;

/// The surface the `orchbee` application consumes, mirroring the owning module paths. Every
/// other item stays inside its private implementation module.
pub mod application {
    pub mod agent_provider {
        pub use crate::agent_provider::{
            CANONICAL_IDS, executable, integration_id, reported_label,
        };
    }

    pub mod agent_runtime {
        pub use crate::agent_runtime::{AgentObservationRead, SendVerdict, wait_backoff};

        pub mod processes {
            pub use crate::agent_runtime::processes::{
                Foreground, Selection, ServerIdentity, filetime_utc, foreground, interpret,
                program_name, provider, readable_command,
            };
        }

        pub mod startup {
            pub use crate::agent_runtime::startup::{AdmissionError, admit, obap_identity};

            pub mod binding {
                pub use crate::agent_runtime::startup::binding::{Binding, PrimaryError};
            }

            pub mod observation {
                pub use crate::agent_runtime::startup::observation::{Error, read};
            }

            pub mod processes {
                pub use crate::agent_runtime::startup::processes::{
                    Error, HostFailure, Observation, attach, read, read_attached,
                };
            }

            pub mod send {
                pub use crate::agent_runtime::startup::send::{
                    Error, InputFailure, Receipt, prompt_records, submit, verdict,
                };
            }

            pub mod snapshot {
                pub use crate::agent_runtime::startup::snapshot::{Error, read};
            }
        }
    }

    pub mod agent_start {
        pub use crate::agent_start::{
            ExchangeFailure, Failure, Report, Request, ReservationState, StartFailure, Submission,
            Verdict, command_valid, run_on_client, step_deadline,
        };
    }

    pub mod background_thread {
        pub use crate::background_thread::{BackgroundThread, EXIT_BUDGET};
    }

    pub mod client_view {
        pub use crate::client_view::{
            Bindings, ClientView, KeyCode, KeyKind, Modifiers, Notice, Overlay,
            format_configuration_binding,
        };
    }

    pub mod local_activation {
        pub use crate::local_activation::{
            ActivationCause, ActivationFailure, StartupFailure, activate_local_session,
            check_deadline, pause, runtime_location, selected_session_name,
        };
        #[cfg(windows)]
        pub use crate::local_activation::{inside_selected_session, select_session};
    }

    pub mod local_client {
        pub use crate::local_client::{
            LocalClient, LocalClientConnectError, LocalClientOrigin, LocalOperationClock,
            OutcomeUnknownReason, SubmissionError, exchange_wake_fault, exchange_wake_ready,
        };

        pub mod agent {
            pub use crate::local_client::agent::{
                AgentExchangeError, AgentExchangeFailure, AgentReadOperation,
            };
        }

        pub mod agent_integration {
            pub use crate::local_client::workbench::agent_integration::AgentIntegrationJobEnd;
        }

        pub mod cancellation {
            pub use crate::local_client::cancellation::{Cancellation, OwnedCancellation};
        }

        pub mod workbench {
            pub use crate::local_client::workbench::{
                LaunchProfileCatalogReadError, LaunchProfileCatalogReadFailure, SnapshotReadError,
                SnapshotReadFailure, WorkbenchTabCloseAccepted, WorkbenchTabCloseFailure,
                WorkbenchTabCloseNotSubmitted, WorkbenchTabClosePrecommit,
                WorkbenchTabCloseRequest, WorkbenchTabCloseResumeSubmission,
                WorkbenchTabCloseSubmission, WorkbenchTabCreateCommit,
                WorkbenchTabCreateNotSubmitted, WorkbenchTabCreateOutcomeUnknownReason,
                WorkbenchTabCreatePrecommit, WorkbenchTabCreateRequest,
                WorkbenchTabCreateSubmission, WorkbenchTabMoveCommit, WorkbenchTabMoveNotSubmitted,
                WorkbenchTabMoveOutcomeUnknownReason, WorkbenchTabMovePrecommit,
                WorkbenchTabMoveRequest, WorkbenchTabMoveSubmission,
                WorkbenchWorkspaceCloseAccepted, WorkbenchWorkspaceCloseFailure,
                WorkbenchWorkspaceCloseNotSubmitted, WorkbenchWorkspaceClosePrecommit,
                WorkbenchWorkspaceCloseReadError, WorkbenchWorkspaceCloseRequest,
                WorkbenchWorkspaceCloseResumeSubmission, WorkbenchWorkspaceCloseSubmission,
                WorktreeListReadError, WorktreeListReadFailure,
            };

            pub mod client_view {
                pub use crate::local_client::workbench::client_view::ClientViewExchangeError;
            }

            pub mod creation {
                pub use crate::local_client::workbench::creation::{
                    CreationRequest, CreationSubmission,
                };
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
                    WorkbenchLaunchProfileMutationPrecommit,
                    WorkbenchLaunchProfileMutationSubmission,
                };
            }

            pub mod metadata_exchange {
                pub use crate::local_client::workbench::metadata_exchange::MetadataExchangeReason;
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
                    WorkbenchPanelRelocateCommit, WorkbenchPanelRelocateNotSubmitted,
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

            pub mod workspace_launch {
                pub use crate::local_client::workbench::workspace_launch::{
                    WorkspaceLaunchRequest, WorkspaceLaunchSubmission,
                };
            }

            pub mod workspace_setup {
                pub use crate::local_client::workbench::workspace_setup::{
                    WorkbenchSetupCommit, WorkbenchSetupNotSubmitted,
                    WorkbenchSetupOutcomeUnknownReason, WorkbenchSetupPrecommit,
                    WorkbenchSetupRequest, WorkbenchSetupSubmission,
                };
            }

            pub mod worktree_query {
                pub use crate::local_client::workbench::worktree_query::WorktreeQueryError;
            }
        }
    }

    pub mod localization {
        pub use crate::localization::{Locale, locale_source, select_locale};
    }

    pub mod machine_agents {
        pub use crate::machine_agents::{
            Collector, StateCheckpoint, Target, confirm, read, read_states_at, verify,
        };
    }

    pub mod machine_discovery {
        pub use crate::machine_discovery::{
            Connection, Context, Discovery, display_label, same_endpoint,
        };
    }

    pub mod machine_observation {
        pub use crate::machine_observation::{Source, snapshot};
    }

    pub mod machine_picker {
        pub use crate::machine_picker::{Choice, SelectionOrigin, SelectionScope};
    }

    pub mod machine_spaces {
        pub use crate::machine_spaces::{Collector, read_metadata, read_with_git};
    }

    pub mod machine_ssh {
        pub use crate::machine_ssh::{
            AdmissionError, Client, client, command, executable, execute, literal, program,
            protocols,
        };
    }

    pub mod machine_tunnel {
        #[cfg(windows)]
        pub use crate::machine_tunnel::serve;
    }

    pub mod machines {
        pub use crate::machines::{
            Catalog, Draft, Edit, Error, Profile, ProfileId, Store, valid_label, valid_target,
        };
    }

    pub mod notifications {
        pub use crate::notifications::paint::render;
        pub use crate::notifications::system::sound;
        pub use crate::workbench_client::notification_available as available;

        pub mod system {
            pub use crate::notifications::system::SystemNotifications;
        }
    }

    pub mod onboarding {
        pub use crate::onboarding::State;
    }

    pub mod terminal_buffer_text {
        pub use crate::terminal_buffer_text::TerminalTextPoint;
    }

    pub mod terminal_content {
        pub use crate::terminal_content::process_directory;
    }

    pub mod terminal_copy {
        pub use crate::terminal_copy::{
            CopyError, CopyMotion, CopySearchDirection, CopySearchResult, TerminalTextPoint,
            TerminalTextRange, read_motion_with, search,
        };
    }

    pub mod terminal_device {
        pub use crate::terminal_device::{
            InheritedTerminal, TerminalAdmissionError, WindowDimensions,
        };
    }

    pub mod terminal_graphics {
        pub use crate::terminal_graphics::Measurement;
    }

    pub mod terminal_host_appearance {
        pub use crate::terminal_host_appearance::Query;
    }

    pub mod terminal_input {
        pub use crate::terminal_input::{
            DecodedItem, DecodedKind, DispatchOutcome, InputDispatcher, LegacyDecoder, ModeRequest,
            MouseAction, MouseButton, MouseEvent, PaneEncode, PaneMouseEvent,
            PointerCaptureRequest, TerminalInputPort, TerminalObservation,
            UnavailableTerminalInputPort, encode_cursor_arrows, encode_pane_report,
        };
    }

    pub mod terminal_lifecycle {
        pub use crate::terminal_lifecycle::{
            InheritedTerminalModes, PointerTrackingLevel, SYNCHRONIZED_UPDATE_BEGIN,
            SYNCHRONIZED_UPDATE_END, SynchronizedUpdateLease, TerminalModeGuard, TerminalModes,
            write_ansi_command,
        };
    }

    pub mod terminal_link_activation {
        pub use crate::terminal_link_activation::{Activation, Error, Guard, SnapshotMember};

        pub mod context {
            pub use crate::terminal_link_activation::context::{fence_member, observe};
        }

        pub mod plugin {
            pub use crate::terminal_link_activation::plugin::{
                Invocation, Preflight, Selection, select, submit,
            };
        }
    }

    pub mod terminal_links {
        pub use crate::terminal_links::{LinkRead, ResolvedLink, explicit_url, resolve};
    }

    pub mod terminal_projection {
        pub use crate::terminal_projection::{
            ProjectionOperation, ProjectionRejection, ProjectionStage, TerminalProjection,
        };
    }

    pub mod terminal_text {
        pub use crate::terminal_text::{
            display_width, escape, is_forbidden_terminal_scalar, quoted, sanitize_terminal_symbol,
            unquoted,
        };
    }

    pub mod terminal_title {
        pub use crate::terminal_title::stripped;
    }

    pub mod termination_signal {
        pub use crate::termination_signal::{TerminationSignal, TerminationSource};
    }

    pub mod tui_capabilities {
        #[cfg(not(windows))]
        pub use crate::host_capabilities::Unsupported;
        pub use crate::tui_capabilities::{Capabilities, Capability};
    }

    pub mod tui_composition {
        pub use crate::tui_composition::{
            CaptureClaims, CaptureOwner, ClientCaptureClaims, WorkbenchCaptureOwner,
            project_capture,
        };
    }

    pub mod tui_graphics {
        pub use crate::tui_graphics::Surface;
    }

    pub mod tui_layout {
        pub use crate::tui_layout::{
            DividerDrag, DrawnSidebarWidth, PaneFrame, ResizeProjection, SidebarWidthDrag,
            ViewportTier, active_pane_area, native_terminal_area, pane_geometries,
            project_shell_areas, terminal_geometry,
        };
    }

    pub mod tui_pointer {
        pub use crate::tui_pointer::HitMapBuilder;
    }

    pub mod tui_render {
        pub use crate::tui_render::{
            render_divider_preview, render_sidebar_reorder_preview, render_tab_drag_preview,
            render_terminal_shell_with_resize, render_width_preview,
        };
    }

    pub mod tui_theme {
        pub use crate::tui_theme::{Theme, ThemeName, cursor_style, resolve_color_level};
    }

    #[cfg(windows)]
    pub mod windows_signal {
        pub use crate::windows_signal::ControlInterrupt;
    }

    pub mod workbench_client {
        pub use crate::workbench_client::{
            ClientContextLocation, ClientContextTarget, ClientMemberTarget, ClientStartupTarget,
            ContextNavigationAdmission, DisplayPhase, DrawReceipt, RetainedFrameBank,
            StartupDirectory, TerminalStatus, UiWakeup, WorkbenchClient, WorkbenchClientView,
        };

        pub mod terminal_viewport {
            pub use crate::workbench_client::terminal_viewport::{
                TerminalViewport, ViewportEffect, ViewportNav, ViewportReadRejection, ViewportStep,
                ViewportSurface, ViewportWindow, ViewportWindowFailure, decode_history_changed,
                decode_viewport_window,
            };
        }
    }

    pub mod worktree_destination {
        pub use crate::worktree_destination::destination;
    }
}

/// Fixtures and scenario runners the application's tests reach, mirroring the owning module
/// paths. Production builds never compile them.
#[cfg(any(test, feature = "test-support"))]
pub mod test_support {
    pub mod agent_runtime {

        pub mod startup {

            pub mod send {
                pub use crate::agent_runtime::startup::send::{Context, ReceiptTestAccess};
            }
        }
    }

    pub mod agent_start {
        pub use crate::agent_start::{DeliveryDisposition, ReportFixture};
    }

    pub mod client_view {
        pub use crate::client_view::{
            ClientAction, ClientActionDisposition, ClientKey, ClientViewTestAccess, KeyEvent,
            PrefixInput, SidebarSurface,
        };
    }

    pub mod local_client {
        pub use crate::local_client::{wake_fault_io, wake_ready_io};

        pub mod multiplex_peer {
            pub use crate::local_client::test_support::multiplex_peer::Peer;
        }

        pub mod server {
            pub use crate::local_client::test_support::server::FakeServer;
        }
    }

    pub mod localization {
        pub use crate::localization::{LocaleTestAccess, ShellTextTestAccess};
    }

    pub mod machine_agents {
        pub use crate::machine_agents::test_support::{
            checkpoint, fixture, physical, row, state_checkpoint,
        };
        pub use crate::machine_agents::{Checkpoint, Observation, RowTestAccess, TargetTestAccess};
    }

    pub mod machine_discovery {
        pub use crate::machine_discovery::{Observation, Overview, OverviewFixture};
    }

    pub mod machine_picker {
        pub use crate::machine_picker::{
            Control, DrawnPickerTargetTestAccess, Mode, PickerAction, PickerSourceTestAccess,
            PickerTestAccess, Selection, SidebarTarget,
        };
    }

    pub mod notifications {
        pub use crate::notifications::StateTestAccess;
    }

    pub mod terminal_device {
        #[cfg(windows)]
        pub use crate::terminal_device::InheritedTerminalTestAccess;
    }

    pub mod terminal_events {
        pub use crate::terminal_events::DecodedItemTestAccess;

        pub mod keys {
            pub use crate::terminal_events::keys::KeyEventTestAccess;
        }
    }

    pub mod terminal_graphics {
        pub use crate::terminal_graphics::{
            Facts, FactsTestAccess, Probe, ProbeTestAccess, Query, QueryFixture, Reply,
        };
    }

    pub mod terminal_host_appearance {
        pub use crate::host_appearance::HostAppearance;
    }

    pub mod terminal_input {
        pub use crate::terminal_input::{
            CloseDecision, HostReply, PaneAdmission, PaneInput, PointerFrameFacts, PointerPane,
            PointerProbe, PointerSelectionIntent, PointerTab, SurfaceAction, SurfaceInput,
            TerminalInputUnavailable,
        };
        pub use crate::workbench_client::input::{
            PointerFrameFactsFixture, PointerPaneFixture, PointerPaneTestAccess,
            PointerProbeFixture, PointerProbeTestAccess, PointerTabFixture,
        };
    }

    pub mod terminal_lifecycle {
        pub use crate::terminal_lifecycle::test_support::{
            FailingOrderedWireWriter, FakeTerminalModes, OrderedWire, OrderedWireModes,
            OrderedWireWriter, lock_panic_hook_tests, synchronized_update_lease,
        };
        pub use crate::terminal_lifecycle::{
            SynchronizedUpdateLeaseTestAccess, TerminalModeGuardTestAccess,
        };

        pub mod graphics {
            pub use crate::terminal_lifecycle::graphics::{ImageLeaseTestAccess, delete};
        }
    }

    pub mod terminal_link_activation {

        pub mod plugin {
            pub use crate::terminal_link_activation::plugin::{
                SelectionTestAccess, select_entries,
            };
        }
    }

    pub mod terminal_links {
        pub use crate::terminal_links::{LinkRegion, ResolvedLinkTestAccess};
    }

    pub mod terminal_pointer_admission {
        pub use crate::terminal_pointer_admission::{
            Capability, HostReply, ModeReportState, Probe, ProbeTestAccess, Query,
        };
    }

    pub mod terminal_text {
        pub use crate::terminal_text::{display_width, truncate_display};
    }

    pub mod termination_signal {
        #[cfg(unix)]
        pub use crate::termination_signal::arm_install_fault;
    }

    pub mod tui_graphics {
        pub use crate::tui_graphics::SurfaceTestAccess;
    }

    pub mod tui_layout {
        pub use crate::tui_layout::{
            DividerDragTestAccess, DividerSource, DividerSourceFixture, DrawnDivider,
            DrawnDividerFixture, DrawnDividerTestAccess, DrawnSidebarWidthTestAccess,
            ShellAreasTestAccess, SidebarAreaTestAccess, SidebarWidth, SidebarWidthTestAccess,
        };
    }

    pub mod tui_pointer {
        pub use crate::tui_pointer::{
            HelpControl, HitLayer, HitMapBuilderTestAccess, HitTarget, InteractionAvailability,
            InteractionSpec, InteractionSpecTestAccess, InteractionTarget, MotionObservability,
            PointerOutcome, PointerState, PointerStateTestAccess, PrimaryContract,
            ProjectedPointer,
        };
    }

    pub mod tui_render {
        pub use crate::tui_render::render_terminal_cells;

        pub mod test_capture {
            pub use crate::tui_render::test_capture::capture_screen;
        }
    }

    pub mod tui_theme {
        pub use crate::tui_theme::{ColorLevel, GlyphSetTestAccess, Role, ThemeTestAccess};
    }

    pub mod workbench_client {
        pub use crate::workbench_client::test_support::{
            RenameServer, SettingsFrameSource, TestResult,
            actual_native_shell_places_confirmed_pixels_inside_one_frame_and_clears_under_a_board,
            covering_client_owner_uses_confirmed_appearance_until_settings_returns,
            dispatch_close_input,
            draft_appearance_reaches_background_chrome_and_program_defaults_without_runtime_effects,
            external_confirmed_values_update_unedited_presentation_and_cancel_reveals_the_latest_snapshot,
            fixture_geometry, interactive_close_server, native_client, native_workbench_client,
            spin, spin_with_draw,
        };
        pub use crate::workbench_client::{
            CaptureContext, ClientMemberTargetFixture, CloseConfirmationToken, InputAction,
            PaneGraphicsCanvas, WorkbenchClientViewTestAccess, WorkspaceCloseToken,
        };

        pub mod input {
            pub use crate::workbench_client::input::InputActionTestAccess;
        }

        pub mod state {
            pub use crate::workbench_client::state::{
                AttachmentKey, AttachmentKeyTestAccess, PaneKey, PaneKeyTestAccess, PaneRef,
                PaneRefTestAccess,
            };
        }
    }
}
