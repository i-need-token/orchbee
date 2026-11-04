export {
	ProjectedAttachmentBootstrapSchema,
	ProjectedTerminalEventSchema,
} from './projected-terminal-contract';
export type {
	ProjectedAttachmentBootstrap,
	ProjectedTerminalCommand,
	ProjectedTerminalEvent,
	ProjectedTerminalFailure,
} from './projected-terminal-contract';
export {
	TerminalPresentationIdSchema,
	terminalPresentationId,
	terminalPresentationOperationId,
} from './terminal-presentation-contract';
export type {
	TerminalPresentationCloseActivity,
	TerminalPresentationId,
	TerminalPresentationLifecycle,
	TerminalPresentationRemovalId,
} from './terminal-presentation-contract';
export {
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
export { assertWorkbenchSharedProjectWorkspaceInvariants } from './assertWorkbenchSharedProjectWorkspaceInvariants';

export {
	AGENT_INTEGRATION_PROVIDERS,
	WORKBENCH_MAXIMUM_LABEL_LENGTH,
	WORKBENCH_PROTOCOL_VERSION,
	WorkbenchLabelSchema,
	WorkbenchLaunchSelectionSchema,
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
	UserLaunchProfileNameSchema,
	UserLaunchProfileSchema,
} from './user-launch-profile-contract';
/** @internal Fixture parser for Web tests and Stories; production parses it only inside the catalog schemas. */
export { UserLaunchProfileIdSchema } from './user-launch-profile-contract';
export type {
	UserLaunchProfile,
	UserLaunchProfileCatalog,
	UserLaunchProfileId,
} from './user-launch-profile-contract';

export {
	DEFAULT_USER_PREFERENCES,
	USER_PREFERENCE_KEYS,
	UserPreferencesPatchSchema,
	UserPreferencesSourceSchema,
} from './user-preferences-contract';
export type {
	UserPreferenceKey,
	UserPreferencesPatch,
	UserPreferencesSnapshot,
	UserPreferencesFailure,
	UserPreferencesResponse,
} from './user-preferences-contract';
