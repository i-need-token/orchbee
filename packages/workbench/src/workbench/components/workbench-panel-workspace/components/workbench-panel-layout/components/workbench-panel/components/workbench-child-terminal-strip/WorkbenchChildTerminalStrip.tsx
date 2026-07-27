import {
	Button,
	ProductIcon,
	Tooltip,
	TooltipContent,
	TooltipTrigger,
} from '@orchbee/ui/components';
import * as React from 'react';

import type { PanelLeaf } from '../../../../../../model/panel-workspace-state';
import type { WorkbenchPanelWorkspaceView } from '../../../../../../types/panel-workspace-view';
import styles from './WorkbenchChildTerminalStrip.module.css';

interface WorkbenchChildTerminalStripProps {
	readonly panel: PanelLeaf;
	readonly view: WorkbenchPanelWorkspaceView;
}

export function WorkbenchChildTerminalStrip({
	panel,
	view,
}: WorkbenchChildTerminalStripProps): React.ReactNode {
	return (
		<div
			aria-label={view.messages.childTerminalsLabel}
			className={styles['root']}
			data-slot='workbench-child-terminal-strip'
			role='toolbar'
		>
			{panel.childTerminals.map((terminal) => (
				<Tooltip key={terminal.id}>
					<TooltipTrigger
						render={
							<Button
								aria-label={terminal.title}
								aria-pressed={panel.activeChildTerminalId === terminal.id}
								data-active={panel.activeChildTerminalId === terminal.id || undefined}
								onClick={() => view.activateChildTerminal(panel.id, terminal.id, 'button')}
								size='icon-sm'
								variant='ghost'
							/>
						}
					>
						<ProductIcon name='terminal' />
					</TooltipTrigger>
					<TooltipContent side='top'>{terminal.title}</TooltipContent>
				</Tooltip>
			))}
		</div>
	);
}
