import {
	Button,
	DropdownMenu,
	DropdownMenuContent,
	DropdownMenuItem,
	DropdownMenuTrigger,
	ProductIcon,
	Tooltip,
	TooltipContent,
	TooltipTrigger,
} from '@orchbee/ui/components';
import * as React from 'react';

import {
	getDisplayedTerminal,
	type PanelLeaf,
} from '../../../../../../model/panel-workspace-state';
import type { WorkbenchPanelWorkspaceView } from '../../../../../../types/panel-workspace-view';
import styles from './WorkbenchPanelHeader.module.css';

interface WorkbenchPanelHeaderProps {
	readonly panel: PanelLeaf;
	readonly view: WorkbenchPanelWorkspaceView;
}

export function WorkbenchPanelHeader({ panel, view }: WorkbenchPanelHeaderProps): React.ReactNode {
	const displayedTerminal = getDisplayedTerminal(panel);
	const isChildTerminalActive = panel.activeChildTerminalId !== null;
	const isMaximized = view.maximizedPanelId === panel.id;

	return (
		<header
			className={styles['root']}
			data-drag-enabled={!isChildTerminalActive || undefined}
			data-slot='workbench-panel-header'
			onPointerDown={
				isChildTerminalActive
					? undefined
					: (event) => {
							const target = event.target;
							if (
								target instanceof Element &&
								target.closest(
									'button, a, input, select, textarea, [role="button"], [role="menuitem"]',
								) !== null
							) {
								return;
							}
							view.startPanelDrag(panel.id, event);
						}
			}
		>
			<div className={styles['left']} data-slot='workbench-panel-header-leading'>
				{isChildTerminalActive && (
					<Tooltip>
						<TooltipTrigger
							render={
								<Button
									aria-label={view.messages.returnToPrimary}
									onClick={() => view.showPrimaryTerminal(panel.id, 'button')}
									size='icon-xs'
									variant='ghost'
								/>
							}
						>
							<ProductIcon name='arrow-left' />
						</TooltipTrigger>
						<TooltipContent side='bottom'>{view.messages.returnToPrimary}</TooltipContent>
					</Tooltip>
				)}
			</div>
			<div className={styles['title']} data-slot='workbench-panel-header-title'>
				{displayedTerminal.title}
			</div>
			<div className={styles['actions']} data-slot='workbench-panel-header-actions'>
				{view.panelCount > 1 && (
					<Tooltip>
						<DropdownMenu>
							<DropdownMenuTrigger
								render={
									<TooltipTrigger
										render={
											<Button
												aria-label={view.messages.more}
												className={styles['mutedAction']}
												size='icon-xs'
												variant='ghost'
											/>
										}
									/>
								}
							>
								<ProductIcon name='more-vertical' />
							</DropdownMenuTrigger>
							<DropdownMenuContent align='end'>
								<DropdownMenuItem onClick={() => view.togglePanelMaximization(panel.id, 'menu')}>
									{isMaximized ? view.messages.minimize : view.messages.maximize}
								</DropdownMenuItem>
							</DropdownMenuContent>
						</DropdownMenu>
						<TooltipContent side='bottom'>{view.messages.more}</TooltipContent>
					</Tooltip>
				)}
				<Tooltip>
					<TooltipTrigger
						render={
							<Button
								aria-label={view.messages.close}
								className={styles['mutedAction']}
								onClick={() => view.requestClose(panel.id, 'button')}
								size='icon-xs'
								variant='ghost'
							/>
						}
					>
						<ProductIcon name='close' />
					</TooltipTrigger>
					<TooltipContent side='bottom'>{view.messages.close}</TooltipContent>
				</Tooltip>
			</div>
		</header>
	);
}
