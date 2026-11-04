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
export { TerminalPresentationIdSchema } from './terminal-presentation-contract';
/** @internal Builds presentation IDs for tests in this package and `apps/web`; production boundaries parse them with `TerminalPresentationIdSchema`. */
export { terminalPresentationId } from './terminal-presentation-contract';
export type {
	TerminalPresentationCloseActivity,
	TerminalPresentationId,
	TerminalPresentationLifecycle,
} from './terminal-presentation-contract';

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
