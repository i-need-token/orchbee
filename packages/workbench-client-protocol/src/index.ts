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
	TerminalPresentationId,
	TerminalPresentationLifecycle,
} from './terminal-presentation-contract';

export {
	AGENT_INTEGRATION_PROVIDERS,
	WORKBENCH_MAXIMUM_LABEL_LENGTH,
	WORKBENCH_PROTOCOL_VERSION,
	WorkbenchAgentStartProviderSchema,
	WorkbenchLabelSchema,
	WorkbenchLaunchSelectionSchema,
	WorkbenchRequestIdSchema,
	WorkbenchStopIdSchema,
	WorktreeReferenceSchema,
	parseWorkbenchAction,
	parseWorkbenchRequest,
	parseWorkbenchResponse,
} from './workbench-protocol';
export type {
	WorkbenchAgentCatalog,
	WorkbenchAgentStartProvider,
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

/** @internal Fixture parser for Web tests and Stories; production parses snapshots inside the settings result schema. */
export { SettingsSnapshotSchema } from './workbench-protocol';
export type {
	SettingChange,
	SettingEntries,
	SettingsRefusal,
	SettingsRefusalReason,
	SettingsSnapshot,
} from './workbench-protocol';
export { readSettingValue, settingDefault } from './setting-keys';
/** @internal Every scalar key, for the Web in-memory settings fixture. */
export { SETTING_KEYS } from './setting-keys';
export type { SettingKey, SettingValues, SettingsPlatform } from './setting-keys';
/** @internal The file state, for Web fixtures that publish one. */
export type { SettingsFile } from './workbench-protocol';
