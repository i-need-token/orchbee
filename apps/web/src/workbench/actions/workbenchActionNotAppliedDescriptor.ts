import { workbenchMessages } from '@orchbee/i18n';

import { PANEL_ACTION_IDS } from '../components/workbench-panel-workspace/actions/panel-action-contracts';
import { WORKBENCH_ACTION_IDS } from './workbench-action-ids';

type WorkbenchNoticeDescriptor = (typeof workbenchMessages)[keyof typeof workbenchMessages];

/**
 * The TUI's sentence for an Action that was refused or failed, keyed by Action ID, so a control, menu,
 * shortcut, or Search invocation says the same words. An Action without an entry keeps the generic
 * sentence.
 */
const WORKBENCH_ACTION_NOT_APPLIED: Readonly<Partial<Record<string, WorkbenchNoticeDescriptor>>> = {
	[WORKBENCH_ACTION_IDS.chooseAgentConfirmation]: workbenchMessages.agentConfirmationNotChanged,
	[WORKBENCH_ACTION_IDS.closeActiveTab]: workbenchMessages.tabCloseSingleNotApplied,
	[WORKBENCH_ACTION_IDS.closePanelTarget]: workbenchMessages.panelCloseNotApplied,
	[WORKBENCH_ACTION_IDS.createTabAfterActive]: workbenchMessages.tabCreateNotCreated,
	[WORKBENCH_ACTION_IDS.createTerminal]: workbenchMessages.tabCreateNotCreated,
	[WORKBENCH_ACTION_IDS.createTerminalInCurrentDirectory]: workbenchMessages.tabCreateNotCreated,
	[WORKBENCH_ACTION_IDS.createWorkspace]: workbenchMessages.workspaceCreateNotApplied,
	[WORKBENCH_ACTION_IDS.duplicateActiveTabs]: workbenchMessages.tabDuplicationNotCreated,
	[WORKBENCH_ACTION_IDS.moveActivePanelDown]: workbenchMessages.panelsMoveNotApplied,
	[WORKBENCH_ACTION_IDS.moveActivePanelLeft]: workbenchMessages.panelsMoveNotApplied,
	[WORKBENCH_ACTION_IDS.moveActivePanelRight]: workbenchMessages.panelsMoveNotApplied,
	[WORKBENCH_ACTION_IDS.moveActivePanelUp]: workbenchMessages.panelsMoveNotApplied,
	[WORKBENCH_ACTION_IDS.moveActiveTabBackward]: workbenchMessages.tabsMoveNotApplied,
	[WORKBENCH_ACTION_IDS.moveActiveTabForward]: workbenchMessages.tabsMoveNotApplied,
	[WORKBENCH_ACTION_IDS.renameProject]: workbenchMessages.projectRenameNotApplied,
	[WORKBENCH_ACTION_IDS.resizeActivePanelDown]: workbenchMessages.panelResizeFailed,
	[WORKBENCH_ACTION_IDS.resizeActivePanelLeft]: workbenchMessages.panelResizeFailed,
	[WORKBENCH_ACTION_IDS.resizeActivePanelRight]: workbenchMessages.panelResizeFailed,
	[WORKBENCH_ACTION_IDS.resizeActivePanelUp]: workbenchMessages.panelResizeFailed,
	[WORKBENCH_ACTION_IDS.selectNextTab]: workbenchMessages.tabsSwitchNotApplied,
	[WORKBENCH_ACTION_IDS.selectPreviousTab]: workbenchMessages.tabsSwitchNotApplied,
	[WORKBENCH_ACTION_IDS.selectTabByPosition]: workbenchMessages.tabsSwitchNotApplied,
	[WORKBENCH_ACTION_IDS.setTabTitle]: workbenchMessages.tabsRenameNotApplied,
	[WORKBENCH_ACTION_IDS.splitActivePanelAbove]: workbenchMessages.panelsSplitNotApplied,
	[WORKBENCH_ACTION_IDS.splitActivePanelBelow]: workbenchMessages.panelsSplitNotApplied,
	[WORKBENCH_ACTION_IDS.splitActivePanelLeft]: workbenchMessages.panelsSplitNotApplied,
	[WORKBENCH_ACTION_IDS.splitActivePanelRight]: workbenchMessages.panelsSplitNotApplied,
	[WORKBENCH_ACTION_IDS.updateWorkspace]: workbenchMessages.workspaceRenameNotApplied,
	[PANEL_ACTION_IDS.close]: workbenchMessages.panelCloseNotApplied,
	[PANEL_ACTION_IDS.move]: workbenchMessages.panelsMoveNotApplied,
	[PANEL_ACTION_IDS.resize]: workbenchMessages.panelResizeFailed,
	[PANEL_ACTION_IDS.split]: workbenchMessages.panelsSplitNotApplied,
};

/** The TUI's sentence for this Action, or undefined when the TUI names none for it. */
export function workbenchActionNotAppliedDescriptor(
	actionId: string,
): WorkbenchNoticeDescriptor | undefined {
	return WORKBENCH_ACTION_NOT_APPLIED[actionId];
}
