import {
	ContextMenu,
	ContextMenuContent,
	ContextMenuItem,
	ContextMenuSeparator,
	ContextMenuTrigger,
} from '@orchbee/ui/components';
import * as React from 'react';

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
			<ContextMenuContent finalFocus={contextMenu.finalFocus}>
				<ContextMenuItem onClick={() => view.createChildTerminal(panelId, 'context-menu')}>
					{view.messages.newChildTerminal}
				</ContextMenuItem>
				<ContextMenuSeparator />
				<ContextMenuItem onClick={(event) => split('right', event)}>
					{view.messages.splitRight}
				</ContextMenuItem>
				<ContextMenuItem onClick={(event) => split('left', event)}>
					{view.messages.splitLeft}
				</ContextMenuItem>
				<ContextMenuItem onClick={(event) => split('below', event)}>
					{view.messages.splitBelow}
				</ContextMenuItem>
				<ContextMenuItem onClick={(event) => split('above', event)}>
					{view.messages.splitAbove}
				</ContextMenuItem>
			</ContextMenuContent>
		</ContextMenu>
	);
}
