import { ContextMenu, ContextMenuTrigger } from '@orchbee/ui/components';
import * as React from 'react';

import { WorkbenchActionContextMenuContent } from '../../../../../../../workbench-action-context-menu-content/WorkbenchActionContextMenuContent';
import type { WorkbenchActionMenuEntry } from '../../../../../../../../types/workbench-action-menu';
import type {
	PanelTerminal,
	PanelSplitDirection,
} from '../../../../../../model/panel-workspace-state';
import type { WorkbenchPanelWorkspaceView } from '../../../../../../types/panel-workspace-view';
import { useWorkbenchTerminalContextMenu } from './hooks/useWorkbenchTerminalContextMenu';
import styles from './WorkbenchTerminalViewport.module.css';

interface WorkbenchTerminalViewportProps {
	readonly panelId: string;
	readonly terminal: PanelTerminal;
	readonly terminalRole: 'child' | 'primary';
	readonly view: WorkbenchPanelWorkspaceView;
}

export function WorkbenchTerminalViewport({
	panelId,
	terminal,
	terminalRole,
	view,
}: WorkbenchTerminalViewportProps): React.ReactNode {
	const contextMenu = useWorkbenchTerminalContextMenu({
		panelId,
		splitPanel: view.splitPanel,
	});
	const split = (direction: PanelSplitDirection, event: React.MouseEvent<HTMLElement>): void => {
		contextMenu.split(direction, event.currentTarget.ownerDocument);
	};
	const entries: readonly WorkbenchActionMenuEntry[] = [
		{
			icon: 'terminal',
			id: 'new-child-terminal',
			kind: 'command',
			label: view.messages.newChildTerminal,
			onSelect: () => view.createChildTerminal(panelId, 'context-menu'),
		},
		{ id: 'terminal-split-actions', kind: 'separator' },
		{
			icon: 'panels',
			id: 'split-panel-right',
			kind: 'command',
			label: view.messages.splitRight,
			onSelect: (event) => split('right', event),
		},
		{
			icon: 'panels',
			id: 'split-panel-left',
			kind: 'command',
			label: view.messages.splitLeft,
			onSelect: (event) => split('left', event),
		},
		{
			icon: 'panels',
			id: 'split-panel-below',
			kind: 'command',
			label: view.messages.splitBelow,
			onSelect: (event) => split('below', event),
		},
		{
			icon: 'panels',
			id: 'split-panel-above',
			kind: 'command',
			label: view.messages.splitAbove,
			onSelect: (event) => split('above', event),
		},
	];

	return (
		<ContextMenu>
			<ContextMenuTrigger
				aria-label={terminal.title}
				className={styles['viewport']}
				data-panel-id={panelId}
				data-slot='workbench-terminal-viewport'
				data-terminal-id={terminal.id}
				data-terminal-role={terminalRole}
				tabIndex={0}
			/>
			<WorkbenchActionContextMenuContent entries={entries} finalFocus={contextMenu.finalFocus} />
		</ContextMenu>
	);
}
