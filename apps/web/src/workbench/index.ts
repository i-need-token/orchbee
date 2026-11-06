export { Workbench } from './Workbench';
export { useWorkbenchConfiguredCommandKeys } from './hooks/useWorkbenchConfiguredCommandKeys';

export { WorkbenchProjectWorkspaceRuntime } from './model/WorkbenchProjectWorkspaceRuntime';
export { matchesWorkbenchProjectCreation } from './model/matchesWorkbenchProjectCreation';
export { resolveActiveTerminalPresentationId } from './utils/resolveActiveTerminalPresentationId';
export {
	readWorkbenchClientSelection,
	readWorkbenchShownPresentationId,
	showWorkbenchClientTarget,
	showWorkbenchPresentation,
} from './model/workbench-client-selection';
export type { WorkbenchClientTarget } from './model/workbench-client-selection';
export type {
	WorkbenchAgentBoard,
	WorkbenchAgentBoardJump,
	WorkbenchAgentBoardJumpFailure,
	WorkbenchAgentBoardRow,
	WorkbenchAgentBoardSnapshot,
} from './types/workbench-agent-board';
export type {
	WorkbenchConfiguredCommandOutcome,
	WorkbenchConfiguredCommands,
} from './types/workbench-configured-commands';

export { resolveTerminalMetadataObservation } from './model/resolveTerminalMetadataObservation';
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
	WorkbenchFocusedPanel,
	WorkbenchPanelTerminal,
} from './types/workbench-focused-panel';
export type {
	WorkbenchWorktreeCommand,
	WorkbenchWorktreeDiscovery,
	WorkbenchWorktreeDiscoveryResult,
	WorkbenchWorktreeFailureCause,
	WorkbenchWorktreeOutcome,
	WorkbenchWorktreeRemoval,
	WorkbenchWorktreeStoppedRemoval,
	WorkbenchWorktrees,
} from './types/workbench-worktrees';
export type {
	WorkbenchAuthority,
	WorkbenchAuthorityChange,
	WorkbenchAuthorityClosure,
	WorkbenchAuthorityContext,
	WorkbenchAuthorityCreation,
	WorkbenchAuthorityPanelTerminalCreation,
	WorkbenchAuthorityPanelTerminalReceipt,
	WorkbenchAuthoritySplitRatioChange,
	WorkbenchAuthorityTabCreation,
	WorkbenchAuthorityTabDuplication,
	WorkbenchAuthorityTabDuplicationReport,
	WorkbenchAuthorityTabReceipt,
	WorkbenchTabCloseStop,
	WorkbenchTabDuplicationStop,
} from './types/workbench-authority';
export { WORKBENCH_SHORTCUTS, defaultWorkbenchShortcutBindings } from './utils/workbench-shortcuts';
export type {
	WorkbenchPublication,
	WorkbenchPublishedTab,
	WorkbenchPublishedTerminalContext,
} from './model/workbench-publication';
export { createDefaultWorkbenchClientView, createWorkbenchClientView } from './document';
export type { WorkbenchClientView } from './document';

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
export type { WorkbenchShortcutClaim } from './types/workbench-shortcut-claims';
export type { WorkbenchShortcutPlatform } from './types/workbench-shortcut-platform';
export type {
	WorkbenchAgentNotification,
	WorkbenchAgentNotificationObserver,
} from './types/workbench-agent-notification';
export type {
	WorkbenchAgentNames,
	WorkbenchAgentNameSource,
	WorkbenchAgentObservation,
	WorkbenchAgentSnapshot,
	WorkbenchAgentSource,
} from './types/workbench-agent-observation';
export type {
	WorkbenchWorkspaceGit,
	WorkbenchWorkspaceLocation,
	WorkbenchWorkspaceLocationSource,
} from './types/workbench-workspace-location';
export type { WorkbenchShortcutBindings, WorkbenchShortcutId } from './utils/workbench-shortcuts';
export type {
	WorkbenchTerminalActionContextProvider,
	WorkbenchTerminalPresentation,
	WorkbenchTerminalSurface,
	WorkbenchTerminalSurfaceProps,
} from './types/workbench-terminal-surface';

export { hasWorkbenchExclusiveFocusSurface } from './utils/workbench-focus-dom';
