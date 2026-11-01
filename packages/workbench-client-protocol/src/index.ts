export {
	MAX_PROJECTED_TERMINAL_IMAGE_PREVIEW_BYTES,
	ProjectedAttachmentBootstrapSchema,
	ProjectedTerminalCommandSchema,
	ProjectedTerminalEventSchema,
} from './projected-terminal-contract';
export type {
	ProjectedAttachmentBootstrap,
	ProjectedTerminalCommand,
	ProjectedTerminalEvent,
	ProjectedTerminalFailure,
} from './projected-terminal-contract';
export {
	TerminalPresentationFailureSchema,
	TerminalPresentationIdSchema,
	terminalPresentationId,
	terminalPresentationOperationId,
	terminalPresentationRemovalId,
} from './terminal-presentation-contract';
export type {
	TerminalPresentationCloseActivity,
	TerminalPresentationCloseOperation,
	TerminalPresentationCloseRequest,
	TerminalPresentationCommand,
	TerminalPresentationCreation,
	TerminalPresentationEvent,
	TerminalPresentationFailure,
	TerminalPresentationId,
	TerminalPresentationLifecycle,
	TerminalPresentationOperationId,
	TerminalPresentationRemovalId,
} from './terminal-presentation-contract';
export {
	TerminalProjectionControlCommandSchema,
	TerminalProjectionControlEventSchema,
	TerminalProjectionRequestIdSchema,
} from './terminal-projection-control-contract';
export type {
	TerminalProjectionControlEvent,
	TerminalProjectionRequestId,
} from './terminal-projection-control-contract';
export {
	WORKBENCH_CLIENT_SESSION_BOOTSTRAP,
	WorkbenchClientSessionBootstrapSchema,
	WorkbenchClientSessionCommandSchema,
	WorkbenchClientSessionEventSchema,
	WorkbenchClientSessionRequestIdSchema,
} from './workbench-client-session-contract';
export type {
	WorkbenchClientSessionBootstrap,
	WorkbenchClientSessionEvent,
	WorkbenchClientSessionRequestId,
	WorkbenchDocumentProposalResult,
	WorkbenchTerminalMetadata,
} from './workbench-client-session-contract';
export {
	WorkbenchSharedDocumentRevisionSchema,
	createWorkbenchSharedDocument,
	parseWorkbenchSharedDocument,
	workbenchSharedDocumentRevision,
} from './workbench-shared-document-contract';
export type {
	WorkbenchSharedDocument,
	WorkbenchSharedDocumentRevision,
	WorkbenchSharedPanelNode,
	WorkbenchSharedPresentation,
	WorkbenchSharedPresentationIntent,
} from './workbench-shared-document-contract';
export {
	areValidWorkbenchSharedLaunchConfigurations,
	assertWorkbenchSharedProjectWorkspaceInvariants,
	hasWorkbenchSharedLaunchConfiguration,
	isValidWorkbenchSharedDirectory,
	workbenchSharedDirectoriesShareLocation,
	workbenchSharedDirectoryPlacementsEqual,
} from './workbench-shared-project-workspace-contract';

export {
	WORKBENCH_PROTOCOL_VERSION,
	WorkbenchRequestIdSchema,
	parseWorkbenchAction,
	parseWorkbenchRequest,
	parseWorkbenchResponse,
} from './workbench-protocol';
export type {
	WorkbenchCommand,
	WorkbenchCommandResult,
	WorkbenchPanelNode,
	WorkbenchPresentation,
	WorkbenchRequest,
	WorkbenchResponse,
	WorkbenchSnapshot,
	WorkbenchTerminalContext,
} from './workbench-protocol';

export {
	BUILT_IN_USER_LAUNCH_PROFILE_ID,
	USER_LAUNCH_PROFILE_LIMITS,
	UserLaunchProfileCatalogSchema,
	UserLaunchProfileIdSchema,
	UserLaunchProfileNameSchema,
	UserLaunchProfileSchema,
} from './user-launch-profile-contract';

export {
	DEFAULT_USER_PREFERENCES,
	MAXIMUM_USER_PREFERENCES_BYTES,
	USER_PREFERENCE_KEYS,
	UserPreferencesPatchSchema,
	UserPreferencesSourceSchema,
	UserPreferencesRequestSchema,
	UserPreferencesResponseSchema,
} from './user-preferences-contract';
export type {
	UserPreferenceKey,
	UserPreferencesPatch,
	UserPreferencesSnapshot,
	UserPreferencesFailure,
	UserPreferencesRequest,
	UserPreferencesResponse,
} from './user-preferences-contract';
