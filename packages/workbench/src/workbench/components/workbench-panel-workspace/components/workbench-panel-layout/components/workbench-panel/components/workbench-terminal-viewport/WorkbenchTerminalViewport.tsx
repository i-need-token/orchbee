import { ContextMenu, ContextMenuTrigger } from '@orchbee/ui/components';
import * as React from 'react';

import { WorkbenchActionContextMenuContent } from '../../../../../../../workbench-action-context-menu-content/WorkbenchActionContextMenuContent';
import type { WorkbenchActionMenuEntry } from '../../../../../../../../types/workbench-action-menu';
import {
	getTerminalDisplayTitle,
	type TerminalPresentation,
	type PanelSplitDirection,
} from '../../../../../../model/panel-workspace-state';
import type { WorkbenchPanelWorkspaceView } from '../../../../../../types/panel-workspace-view';
import type { WorkbenchTerminalSurface } from '../../../../../../../../types/workbench-terminal-surface';
import { useWorkbenchTerminalContextMenu } from './hooks/useWorkbenchTerminalContextMenu';
import { useWorkbenchTerminalActions } from './hooks/useWorkbenchTerminalActions';
import styles from './WorkbenchTerminalViewport.module.css';

interface WorkbenchTerminalViewportProps {
	readonly panelId: string;
	readonly terminal: TerminalPresentation;
	readonly terminalRole: 'child' | 'primary';
	readonly terminalSurface?: WorkbenchTerminalSurface | undefined;
	readonly view: WorkbenchPanelWorkspaceView;
}

export function WorkbenchTerminalViewport({
	panelId,
	terminal,
	terminalRole,
	terminalSurface: TerminalSurface,
	view,
}: WorkbenchTerminalViewportProps): React.ReactNode {
	const contextMenu = useWorkbenchTerminalContextMenu({
		panelId,
		splitPanel: view.splitPanel,
	});
	const terminalActions = useWorkbenchTerminalActions({
		parentScope: view.actionScope,
		presentationId: terminal.presentationId,
		runtime: view.actionRuntime,
	});
	const split = (direction: PanelSplitDirection, event: React.MouseEvent<HTMLElement>): void => {
		contextMenu.split(direction, event.currentTarget.ownerDocument);
	};
	const structuralEntries: readonly WorkbenchActionMenuEntry[] = [
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
	const entries: readonly WorkbenchActionMenuEntry[] =
		terminalActions.entries.length === 0
			? structuralEntries
			: [
					...terminalActions.entries,
					{ id: 'terminal-product-actions', kind: 'separator' },
					...structuralEntries,
				];

	return (
		<ContextMenu>
			<ContextMenuTrigger
				aria-label={getTerminalDisplayTitle(terminal)}
				className={styles['viewport']}
				data-panel-id={panelId}
				data-slot='workbench-terminal-viewport'
				data-terminal-presentation-id={terminal.presentationId}
				data-terminal-role={terminalRole}
				onContextMenu={(event) => {
					if (terminalActions.captureContext(event)) {
						contextMenu.captureFinalFocus(event.currentTarget.ownerDocument);
						return;
					}
					event.preventDefault();
					event.stopPropagation();
				}}
				tabIndex={0}
			>
				{TerminalSurface === undefined ? null : (
					<TerminalSurface
						actionRuntime={view.actionRuntime}
						actionScope={terminalActions.scope}
						admitBellEffect={view.admitTerminalBellEffect}
						bindActionContextProvider={terminalActions.bindActionContextProvider}
						presentation={terminal}
					/>
				)}
			</ContextMenuTrigger>
			<WorkbenchActionContextMenuContent entries={entries} finalFocus={contextMenu.finalFocus} />
		</ContextMenu>
	);
}
