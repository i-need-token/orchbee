export {
	DESKTOP_AGENT_INTEGRATION_IDS,
	DesktopAgentIntegrationIdSchema,
	DesktopAgentIntegrationManagementMainEventSchema,
	DesktopAgentIntegrationManagementRendererCommandSchema,
	DesktopAgentIntegrationManagementRequestIdSchema,
	DesktopAgentIntegrationManagementSnapshotSchema,
} from './desktop-agent-integration-management-contract';
export type {
	DesktopAgentIntegrationId,
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
export type { DesktopAgentObservationRequestId } from './desktop-agent-observation-contract';

export {
	DesktopClipboardMainEventSchema,
	DesktopClipboardRendererCommandSchema,
	DesktopClipboardRequestIdSchema,
	DesktopClipboardTextSchema,
} from './desktop-clipboard-contract';
export type {
	DesktopClipboardReadResult,
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
export {
	DESKTOP_SERVER_CONNECTION_IPC_CHANNEL,
	DESKTOP_SERVER_CONNECTION_WINDOW_MESSAGE,
	DesktopServerConnectionWindowMessageSchema,
	type DesktopServerConnectionBootstrap,
} from './desktop-server-connection-contract';

export {
	DesktopTerminalDefaultSettingsMainEventSchema,
	DesktopTerminalDefaultSettingsRendererCommandSchema,
	DesktopTerminalDefaultSettingsRequestIdSchema,
	DesktopTerminalDefaultSettingsSnapshotSchema,
} from './desktop-terminal-default-settings-contract';
export type {
	DesktopTerminalDefaultSettingsFailureCode,
	DesktopTerminalDefaultSettingsMainEvent,
	DesktopTerminalDefaultSettingsRendererCommand,
	DesktopTerminalDefaultSettingsRequestId,
	DesktopTerminalDefaultSettingsSnapshot,
} from './desktop-terminal-default-settings-contract';
export { DESKTOP_FILE_PATH_BRIDGE_KEY } from './desktop-file-path-bridge-contract';
export {
	DesktopNewProjectFolderNameSchema,
	DesktopProjectFolderAcquisitionMainEventSchema,
	DesktopProjectFolderAcquisitionRendererCommandSchema,
	DesktopProjectFolderAcquisitionRequestIdSchema,
	DesktopProjectFolderPathSchema,
} from './desktop-project-folder-acquisition-contract';
export {
	DesktopProjectWorktreeMainEventSchema,
	DesktopProjectWorktreeRendererCommandSchema,
	DesktopProjectWorktreeRequestIdSchema,
} from './desktop-project-worktree-contract';
export type {
	DesktopProjectWorktreeAdoptionResult,
	DesktopProjectWorktreeCreationResult,
	DesktopProjectWorktreeDiscovery,
	DesktopProjectWorktreeFailureCode,
	DesktopProjectWorktreeRequestId,
} from './desktop-project-worktree-contract';
export type {
	DesktopProjectFolderAcquisitionFailureCode,
	DesktopProjectFolderAcquisitionRendererCommand,
	DesktopProjectFolderAcquisitionRequestId,
	DesktopProjectFolderAcquisitionResult,
} from './desktop-project-folder-acquisition-contract';

export {
	DESKTOP_BUILT_IN_TERMINAL_LAUNCH_PROFILE_ID,
	DESKTOP_TERMINAL_LAUNCH_PROFILE_LIMITS,
	DesktopTerminalLaunchProfileCatalogSchema,
	DesktopTerminalLaunchProfileIdSchema,
	DesktopTerminalLaunchProfileMainEventSchema,
	DesktopTerminalLaunchProfileNameSchema,
	DesktopTerminalLaunchProfileRendererCommandSchema,
	DesktopTerminalLaunchProfileRequestIdSchema,
	DesktopTerminalLaunchProfileSchema,
	DesktopTerminalLaunchProfileSnapshotSchema,
} from './desktop-terminal-launch-profile-contract';
export type {
	DesktopTerminalLaunchProfile,
	DesktopTerminalLaunchProfileCatalog,
	DesktopTerminalLaunchProfileFailureCode,
	DesktopTerminalLaunchProfileId,
	DesktopTerminalLaunchProfileMainEvent,
	DesktopTerminalLaunchProfileRendererCommand,
	DesktopTerminalLaunchProfileRequestId,
	DesktopTerminalLaunchProfileSnapshot,
} from './desktop-terminal-launch-profile-contract';
export {
	DesktopUserPreferencesCommandSchema,
	DesktopUserPreferencesEventSchema,
} from './desktop-user-preferences-contract';
export type { DesktopUserPreferencesEvent } from './desktop-user-preferences-contract';
