export {
	DESKTOP_AGENT_INTEGRATION_IDS,
	DesktopAgentIntegrationAvailabilitySchema,
	DesktopAgentIntegrationIdSchema,
	DesktopAgentIntegrationIssueSchema,
	DesktopAgentIntegrationManagementFailureCodeSchema,
	DesktopAgentIntegrationManagementMainEventSchema,
	DesktopAgentIntegrationManagementModeSchema,
	DesktopAgentIntegrationManagementRendererCommandSchema,
	DesktopAgentIntegrationManagementRequestIdSchema,
	DesktopAgentIntegrationManagementSnapshotSchema,
	DesktopAgentIntegrationSnapshotItemSchema,
	DesktopAgentIntegrationStateSchema,
} from './desktop-agent-integration-management-contract';
export type {
	DesktopAgentIntegrationAvailability,
	DesktopAgentIntegrationId,
	DesktopAgentIntegrationIssue,
	DesktopAgentIntegrationManagementFailureCode,
	DesktopAgentIntegrationManagementMainEvent,
	DesktopAgentIntegrationManagementMode,
	DesktopAgentIntegrationManagementRendererCommand,
	DesktopAgentIntegrationManagementRequestId,
	DesktopAgentIntegrationManagementSnapshot,
	DesktopAgentIntegrationSnapshotItem,
	DesktopAgentIntegrationState,
} from './desktop-agent-integration-management-contract';
export {
	DESKTOP_AGENT_OBSERVATION_PROTOCOL_VERSION,
	DesktopAgentObservationMainEventSchema,
	DesktopAgentObservationRendererCommandSchema,
	DesktopAgentObservationRequestIdSchema,
} from './desktop-agent-observation-contract';
export type {
	DesktopAgentObservationMainEvent,
	DesktopAgentObservationRendererCommand,
	DesktopAgentObservationRequestId,
} from './desktop-agent-observation-contract';

export {
	DesktopClipboardFailureCodeSchema,
	DesktopClipboardMainEventSchema,
	DesktopClipboardReadResultSchema,
	DesktopClipboardRendererCommandSchema,
	DesktopClipboardRequestIdSchema,
	DesktopClipboardTextSchema,
} from './desktop-clipboard-contract';
export type {
	DesktopClipboardFailureCode,
	DesktopClipboardMainEvent,
	DesktopClipboardReadResult,
	DesktopClipboardRendererCommand,
	DesktopClipboardRequestId,
} from './desktop-clipboard-contract';
export {
	DesktopApplicationExitRequestIdSchema,
	DesktopApplicationLifetimeMainEventSchema,
	DesktopApplicationLifetimeRendererCommandSchema,
} from './desktop-application-lifetime-contract';
export type {
	DesktopApplicationExitChoice,
	DesktopApplicationExitFailure,
	DesktopApplicationExitMode,
	DesktopApplicationExitProgress,
	DesktopApplicationExitReason,
	DesktopApplicationExitRequestId,
	DesktopApplicationLifetimeMainEvent,
	DesktopApplicationLifetimeRendererCommand,
} from './desktop-application-lifetime-contract';
export {
	DESKTOP_RENDERER_BOOTSTRAP_IPC_CHANNEL,
	DESKTOP_RENDERER_BOOTSTRAP_WINDOW_MESSAGE,
	DESKTOP_RENDERER_PORT_ROLES,
	DesktopRendererBootstrapSchema,
	DesktopRendererBootstrapWindowMessageSchema,
} from './desktop-renderer-bootstrap-contract';
export type { DesktopRendererBootstrap } from './desktop-renderer-bootstrap-contract';
export {
	DesktopTerminalDefaultSettingsFailureCodeSchema,
	DesktopTerminalDefaultSettingsMainEventSchema,
	DesktopTerminalDefaultSettingsPersistenceIssueSchema,
	DesktopTerminalDefaultSettingsRendererCommandSchema,
	DesktopTerminalDefaultSettingsRequestIdSchema,
	DesktopTerminalDefaultSettingsSnapshotSchema,
} from './desktop-terminal-default-settings-contract';
export type {
	DesktopTerminalDefaultSettingsFailureCode,
	DesktopTerminalDefaultSettingsMainEvent,
	DesktopTerminalDefaultSettingsPersistenceIssue,
	DesktopTerminalDefaultSettingsRendererCommand,
	DesktopTerminalDefaultSettingsRequestId,
	DesktopTerminalDefaultSettingsSnapshot,
} from './desktop-terminal-default-settings-contract';
export { DESKTOP_FILE_PATH_BRIDGE_KEY } from './desktop-file-path-bridge-contract';
export {
	DesktopNewProjectFolderNameSchema,
	DesktopProjectFolderAcquisitionFailureCodeSchema,
	DesktopProjectFolderAcquisitionMainEventSchema,
	DesktopProjectFolderAcquisitionRendererCommandSchema,
	DesktopProjectFolderAcquisitionRequestIdSchema,
	DesktopProjectFolderAcquisitionResultSchema,
	DesktopProjectFolderPathSchema,
} from './desktop-project-folder-acquisition-contract';
export {
	DesktopProjectExistingWorktreeSchema,
	DesktopProjectWorktreeAdoptionResultSchema,
	DesktopProjectWorktreeCreationResultSchema,
	DesktopProjectWorktreeDiscoverySchema,
	DesktopProjectWorktreeFailureCodeSchema,
	DesktopProjectWorktreeMainEventSchema,
	DesktopProjectWorktreeRendererCommandSchema,
	DesktopProjectWorktreeRepositorySchema,
	DesktopProjectWorktreeRequestIdSchema,
	DesktopProjectWorktreeInventorySchema,
} from './desktop-project-worktree-contract';
export type {
	DesktopProjectWorktreeAdoptionResult,
	DesktopProjectWorktreeCreationResult,
	DesktopProjectWorktreeDiscovery,
	DesktopProjectWorktreeFailureCode,
	DesktopProjectWorktreeMainEvent,
	DesktopProjectWorktreeRendererCommand,
	DesktopProjectWorktreeRequestId,
} from './desktop-project-worktree-contract';
export type {
	DesktopProjectFolderAcquisitionFailureCode,
	DesktopProjectFolderAcquisitionMainEvent,
	DesktopProjectFolderAcquisitionRendererCommand,
	DesktopProjectFolderAcquisitionRequestId,
	DesktopProjectFolderAcquisitionResult,
} from './desktop-project-folder-acquisition-contract';

export {
	DESKTOP_BUILT_IN_TERMINAL_LAUNCH_PROFILE_ID,
	DESKTOP_TERMINAL_LAUNCH_PROFILE_LIMITS,
	DesktopTerminalLaunchProfileCatalogSchema,
	DesktopTerminalLaunchProfileCommandSchema,
	DesktopTerminalLaunchProfileFailureCodeSchema,
	DesktopTerminalLaunchProfileIdSchema,
	DesktopTerminalLaunchProfileMainEventSchema,
	DesktopTerminalLaunchProfileNameSchema,
	DesktopTerminalLaunchProfilePersistenceIssueSchema,
	DesktopTerminalLaunchProfileRendererCommandSchema,
	DesktopTerminalLaunchProfileRequestIdSchema,
	DesktopTerminalLaunchProfileSchema,
	DesktopTerminalLaunchProfileSnapshotSchema,
} from './desktop-terminal-launch-profile-contract';
export type {
	DesktopTerminalLaunchProfile,
	DesktopTerminalLaunchProfileCatalog,
	DesktopTerminalLaunchProfileCommand,
	DesktopTerminalLaunchProfileFailureCode,
	DesktopTerminalLaunchProfileId,
	DesktopTerminalLaunchProfileMainEvent,
	DesktopTerminalLaunchProfilePersistenceIssue,
	DesktopTerminalLaunchProfileRendererCommand,
	DesktopTerminalLaunchProfileRequestId,
	DesktopTerminalLaunchProfileSnapshot,
} from './desktop-terminal-launch-profile-contract';
