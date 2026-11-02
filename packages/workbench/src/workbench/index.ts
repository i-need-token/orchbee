export { Workbench } from './Workbench';

export { WorkbenchProjectWorkspaceRuntime } from './model/WorkbenchProjectWorkspaceRuntime';
export { matchesWorkbenchProjectCreation } from './model/matchesWorkbenchProjectCreation';

export { createWorkbenchPreviewTerminalPresentationComposition } from './model/createWorkbenchPreviewTerminalPresentationComposition';
export {
	resolveTerminalMetadataObservation,
	resolveTerminalNaturalMetadataObservation,
} from './model/resolve-terminal-metadata-observation';
export { WorkbenchTerminalBellEffectCoordinator } from './model/WorkbenchTerminalBellEffectCoordinator';
export type { WorkbenchTerminalBellEffect } from './model/WorkbenchTerminalBellEffectCoordinator';
export type {
	WorkbenchProjectFolderAcquisition,
	WorkbenchProjectFolderAcquisitionFailureCode,
	WorkbenchProjectFolderAcquisitionResult,
	WorkbenchProjectCreationReceipt,
	WorkbenchNewProjectFolderCapability,
	WorkbenchNewProjectFolderParent,
	WorkbenchNewProjectFolderParentResult,
	WorkbenchNewProjectFolderResult,
} from './types/workbench-project-folder-acquisition';
export type {
	WorkbenchProjectWorktreeCapability,
	WorkbenchProjectWorktreeAdoptionResult,
	WorkbenchProjectWorktreeCreationResult,
	WorkbenchProjectWorktreeDiscoveryResult,
	WorkbenchProjectWorktreeFailureCode,
} from './types/workbench-project-worktree';
export type { WorkbenchRemoteTerminalCreationCapability } from './types/workbench-remote-terminal-creation';
export type {
	WorkbenchAuthority,
	WorkbenchAuthorityChange,
	WorkbenchAuthorityClosure,
	WorkbenchAuthorityContext,
	WorkbenchAuthorityCreation,
} from './types/workbench-authority';
export type { WorkbenchEnvironmentProjectConnectionCapability } from './types/workbench-environment-project-connection';
export { WORKBENCH_SHORTCUTS, defaultWorkbenchShortcutBindings } from './utils/workbench-shortcuts';
export type {
	WorkbenchTerminalSessionStopSnapshot,
	WorkbenchTerminalPresentationEventResult,
} from './model/WorkbenchProjectWorkspaceRuntime';
export type { WorkbenchPublication } from './model/workbench-publication';

export type {
	WorkbenchTerminalPresentationCommand,
	WorkbenchTerminalPresentationComposition,
} from './model/createWorkbenchPreviewTerminalPresentationComposition';
export type { WorkbenchPanelPosition, WorkbenchTabPosition } from './actions/workbench-action-ids';
export { TERMINAL_ACTION_IDS } from './actions/terminal-action-contracts';
export type {
	WorkbenchTerminalActionCatalog,
	WorkbenchTerminalLinkActionInput,
} from './actions/terminal-action-contracts';
export type { WorkbenchTerminalActionShortcutLabels } from './types/workbench-terminal-action-shortcut-labels';
export type {
	WorkbenchHostActionCallback,
	WorkbenchNavigationPort,
	WorkbenchNavigationTarget,
	WorkbenchTerminalFontSizeChangeCallback,
} from './types/workbench-host-actions';
export type { WorkbenchShortcutPlatform } from './types/workbench-shortcut-platform';
export type {
	WorkbenchAgentEventObservation,
	WorkbenchAgentNotificationObserver,
} from './types/workbench-agent-notification';
export type { WorkbenchShortcutBindings, WorkbenchShortcutId } from './utils/workbench-shortcuts';
export type {
	WorkbenchTerminalActionContextProvider,
	WorkbenchTerminalBroadcastInput,
	WorkbenchTerminalBroadcastInputReadiness,
	WorkbenchTerminalBroadcastInputResult,
	WorkbenchTerminalBroadcastInputSink,
	WorkbenchTerminalPresentation,
	WorkbenchTerminalSurface,
	WorkbenchTerminalSurfaceProps,
} from './types/workbench-terminal-surface';
