export { Workbench } from './Workbench';
export type { WorkbenchProps } from './Workbench';
export type {
	WorkbenchTabScopeSnapshot,
	WorkbenchTabScopeSource,
	WorkbenchTabScopeTerminalContext,
} from './types/workbench-tab-scope';
export type {
	WorkbenchTabWindowControl,
	WorkbenchTabWindowDestination,
	WorkbenchTabWindowMoveResult,
	WorkbenchTabWindowSnapshot,
	WorkbenchTabWindowSummary,
} from './types/workbench-tab-window-control';
export { WorkbenchProjectWorkspaceRuntime } from './model/WorkbenchProjectWorkspaceRuntime';
export type { CurrentDirectoryTerminalTarget } from './model/resolve-current-directory-terminal-target';
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
} from './types/workbench-project-folder-acquisition';
export type {
	WorkbenchProjectWorktreeCapability,
	WorkbenchProjectWorktreeAdoptionResult,
	WorkbenchProjectWorktreeCreationResult,
	WorkbenchProjectWorktreeDiscovery,
	WorkbenchProjectWorktreeDiscoveryResult,
	WorkbenchProjectExistingWorktree,
	WorkbenchProjectWorktreeFailureCode,
	WorkbenchProjectWorktreeInventory,
	WorkbenchProjectWorktreeRepository,
} from './types/workbench-project-worktree';
export type {
	WorkbenchRemoteTerminalCreationCapability,
	WorkbenchRemoteTerminalCreationFailureReason,
	WorkbenchRemoteTerminalCreationResult,
} from './types/workbench-remote-terminal-creation';
export type {
	WorkbenchEnvironmentProjectConnectionCapability,
	WorkbenchEnvironmentProjectConnectionState,
	WorkbenchEnvironmentProjectConnectionStatus,
} from './types/workbench-environment-project-connection';
export { WORKBENCH_SHORTCUTS, defaultWorkbenchShortcutBindings } from './utils/workbench-shortcuts';
export type {
	WorkbenchActiveTerminalCreationRoute,
	WorkbenchActiveTerminalCreationRouteInput,
	WorkbenchEnvironmentTerminalCreateInput,
	WorkbenchPublishedTerminalIdentity,
	WorkbenchProjectWorkspaceRuntimeOptions,
	WorkbenchRuntimeTerminalCreateInput,
	WorkbenchRuntimeTerminalCreateAtCwdInput,
	WorkbenchRuntimeTerminalCreateResult,
	WorkbenchSharedDocumentApplyResult,
	WorkbenchStandaloneTerminalPresentation,
	WorkbenchTerminalSessionStopFailure,
	WorkbenchTerminalSessionStopRequestResult,
	WorkbenchTerminalSessionRestartStageResult,
	WorkbenchTerminalSessionStopSnapshot,
	WorkbenchTerminalPresentationEventResult,
} from './model/WorkbenchProjectWorkspaceRuntime';
export type {
	RawTerminalMetadataObservation,
	RawTerminalNaturalMetadataObservation,
	WorkbenchTerminalCurrentDirectoryObservation,
	WorkbenchTerminalMetadataObservation,
} from './model/resolve-terminal-metadata-observation';
export type {
	WorkbenchTerminalPresentationCommand,
	WorkbenchTerminalPresentationComposition,
} from './model/createWorkbenchPreviewTerminalPresentationComposition';
export type {
	WorkbenchActionCatalog,
	WorkbenchProjectCreateInput,
	WorkbenchProjectDirectoryInput,
	WorkbenchProjectFolderAddInput,
	WorkbenchProjectInput,
	WorkbenchProjectRenameInput,
	WorkbenchPanelPosition,
	WorkbenchPanelPositionInput,
	WorkbenchSearchOpenInput,
	WorkbenchTabPosition,
	WorkbenchTabPositionInput,
	WorkbenchTerminalCreateInput,
	WorkbenchWorkspaceCreateInput,
	WorkbenchWorkspaceInput,
	WorkbenchWorkspaceUpdateInput,
} from './actions/workbench-action-ids';
export { TERMINAL_ACTION_IDS } from './actions/terminal-action-contracts';
export type {
	WorkbenchTerminalActionCatalog,
	WorkbenchTerminalActionId,
	WorkbenchTerminalLinkActionInput,
} from './actions/terminal-action-contracts';
export type { WorkbenchTerminalActionShortcutLabels } from './types/workbench-terminal-action-shortcut-labels';
export type {
	WorkbenchHostActionCallback,
	WorkbenchHostActionResult,
	WorkbenchNavigationPort,
	WorkbenchNavigationTarget,
	WorkbenchTerminalCreateCallback,
	WorkbenchTerminalCreateRequest,
	WorkbenchTerminalFontSizeChange,
	WorkbenchTerminalFontSizeChangeCallback,
} from './types/workbench-host-actions';
export type { WorkbenchShortcutPlatform } from './types/workbench-shortcut-platform';
export type {
	WorkbenchShortcutBinding,
	WorkbenchShortcutBindings,
	WorkbenchShortcutId,
} from './utils/workbench-shortcuts';
export type {
	WorkbenchTerminalActionContextCapture,
	WorkbenchTerminalActionContextCaptureRequest,
	WorkbenchTerminalActionContextProvider,
	WorkbenchTerminalBroadcastInput,
	WorkbenchTerminalBroadcastInputReadiness,
	WorkbenchTerminalBroadcastInputResult,
	WorkbenchTerminalBroadcastInputSink,
	WorkbenchTerminalBroadcastKeyModifiers,
	WorkbenchTerminalPresentation,
	WorkbenchTerminalPresentationMode,
	WorkbenchTerminalPresentationReadiness,
	WorkbenchTerminalSurface,
	WorkbenchTerminalSurfaceProps,
} from './types/workbench-terminal-surface';
export type { WorkbenchSearchFilter } from './types/workbench-search';
export type {
	WorkbenchSearchProvider,
	WorkbenchSearchProviderArgumentOption,
	WorkbenchSearchProviderArgumentPicker,
	WorkbenchSearchProviderContext,
	WorkbenchSearchProviderResult,
	WorkbenchSearchProviderResultInvocation,
	WorkbenchSearchProviderSection,
} from './types/workbench-search-provider';
