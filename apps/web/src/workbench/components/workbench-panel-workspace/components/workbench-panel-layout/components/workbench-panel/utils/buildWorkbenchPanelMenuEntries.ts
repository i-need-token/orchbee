import type { ActionInvocationSource } from '@orchbee/actions';

import { WORKBENCH_ACTION_IDS } from '../../../../../../../actions/workbench-action-ids';
import type {
	WorkbenchActionMenuCommand,
	WorkbenchActionMenuEntry,
} from '../../../../../../../types/workbench-action-menu';
import type { WorkbenchPanelWorkspaceView } from '../../../../../types/panel-workspace-view';

interface WorkbenchPanelMenuOptions {
	readonly panelId: string;
	readonly source: Extract<ActionInvocationSource, 'context-menu' | 'menu'>;
	readonly split: (direction: 'below' | 'right') => void;
	/** Present only while another Client holds the panel's size. */
	readonly takeControl?: WorkbenchActionMenuCommand | undefined;
	readonly view: WorkbenchPanelWorkspaceView;
}

/**
 * A panel's menu, composed as the TUI's pane menu with no GUI-only section: Split right and Split
 * below, Maximize only beside another panel and Take control only while another Client holds the
 * panel's size, then the red Close. The other split directions, resizing and equalizing stay in
 * Search and their shortcuts. Opening the menu focuses its panel, so a row's active-panel shortcut
 * does what the row does.
 */
export function buildWorkbenchPanelMenuEntries({
	panelId,
	source,
	split,
	takeControl,
	view,
}: WorkbenchPanelMenuOptions): readonly WorkbenchActionMenuEntry[] {
	const isMaximized = view.maximizedPanelId === panelId;
	return [
		{
			icon: 'edit',
			id: 'rename-panel',
			kind: 'command',
			label: view.messages.rename,
			onSelect: () => view.renamePanel(panelId, source),
		},
		{
			icon: 'panels',
			id: 'split-panel-right',
			kind: 'command',
			label: view.messages.splitRight,
			onSelect: () => split('right'),
			shortcut: view.shortcutLabelForAction(WORKBENCH_ACTION_IDS.splitActivePanelRight),
		},
		{
			icon: 'panels',
			id: 'split-panel-below',
			kind: 'command',
			label: view.messages.splitBelow,
			onSelect: () => split('below'),
			shortcut: view.shortcutLabelForAction(WORKBENCH_ACTION_IDS.splitActivePanelBelow),
		},
		...(view.panelCount < 2
			? []
			: [
					{
						icon: isMaximized ? 'screen-normal' : 'screen-full',
						id: 'toggle-panel-maximization',
						kind: 'command',
						label: isMaximized ? view.messages.restore : view.messages.maximize,
						onSelect: () => view.togglePanelMaximization(panelId, source),
						shortcut: view.shortcutLabelForAction(
							WORKBENCH_ACTION_IDS.toggleActivePanelMaximization,
						),
					} satisfies WorkbenchActionMenuCommand,
				]),
		...(takeControl === undefined ? [] : [takeControl]),
		{ id: 'panel-close-actions', kind: 'separator' },
		{
			destructive: true,
			icon: 'close',
			id: 'close-active-terminal',
			kind: 'command',
			label: view.messages.close,
			onSelect: () => view.closeActiveTerminal(panelId, source),
			shortcut: view.shortcutLabelForAction(WORKBENCH_ACTION_IDS.closeActiveTerminal),
		},
	];
}
