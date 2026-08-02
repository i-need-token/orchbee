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
			aria-label={view.messages.childPresentationsLabel}
			className={styles['root']}
			data-slot='workbench-child-terminal-strip'
			role='toolbar'
		>
			{panel.childPresentations.map((terminal) => (
				<Tooltip key={terminal.presentationId}>
					<TooltipTrigger
						render={
							<Button
								aria-label={terminal.title}
								aria-pressed={panel.activeChildPresentationId === terminal.presentationId}
								data-active={
									panel.activeChildPresentationId === terminal.presentationId || undefined
								}
								onClick={() =>
									view.activateChildTerminal(panel.id, terminal.presentationId, 'button')
								}
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
