export {
	DESKTOP_AGENT_INTEGRATION_IDS,
	DesktopAgentIntegrationIdSchema,
} from './desktop-agent-integration-management-contract';
export type {
	DesktopAgentIntegrationId,
	DesktopAgentIntegrationManagementFailureCode,
	DesktopAgentIntegrationManagementMainEvent,
	DesktopAgentIntegrationManagementSnapshot,
	DesktopAgentIntegrationSnapshotItem,
} from './desktop-agent-integration-management-contract';
export {
	DESKTOP_SERVER_CONNECTION_IPC_CHANNEL,
	DESKTOP_SERVER_CONNECTION_PORT_ROLES,
	DESKTOP_SERVER_CONNECTION_REQUEST_IPC_CHANNEL,
	DESKTOP_SERVER_CONNECTION_WINDOW_MESSAGE,
	DESKTOP_SERVER_CONNECTION_WINDOW_REQUEST,
	DesktopServerConnectionDocumentSchema,
	DesktopServerConnectionReplySchema,
	DesktopServerConnectionWindowMessageSchema,
	DesktopServerConnectionWindowRequestSchema,
	type DesktopServerConnectionBootstrap,
	type DesktopServerConnectionPortRole,
	type DesktopServerConnectionReply,
} from './desktop-server-connection-contract';

/** @internal Fixture parser for the Web terminal settings Stories; no production boundary parses it. */
export { DesktopTerminalDefaultSettingsSnapshotSchema } from './desktop-terminal-default-settings-contract';
export type {
	DesktopTerminalDefaultSettingsFailureCode,
	DesktopTerminalDefaultSettingsMainEvent,
	DesktopTerminalDefaultSettingsSnapshot,
} from './desktop-terminal-default-settings-contract';
export {
	DesktopNewProjectFolderNameSchema,
	DesktopProjectFolderAcquisitionMainEventSchema,
	DesktopProjectFolderAcquisitionRendererCommandSchema,
	DesktopProjectFolderAcquisitionRequestIdSchema,
	DesktopProjectFolderPathSchema,
} from './desktop-project-folder-acquisition-contract';
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
	DesktopTerminalLaunchProfileNameSchema,
	DesktopTerminalLaunchProfileSchema,
} from './desktop-terminal-launch-profile-contract';
/** @internal Fixture parsers for the Web terminal settings Stories and tests; no production boundary parses them. */
export {
	DesktopTerminalLaunchProfileIdSchema,
	DesktopTerminalLaunchProfileSnapshotSchema,
} from './desktop-terminal-launch-profile-contract';
export type {
	DesktopTerminalLaunchProfile,
	DesktopTerminalLaunchProfileCatalog,
	DesktopTerminalLaunchProfileFailureCode,
	DesktopTerminalLaunchProfileId,
	DesktopTerminalLaunchProfileMainEvent,
	DesktopTerminalLaunchProfileSnapshot,
} from './desktop-terminal-launch-profile-contract';
