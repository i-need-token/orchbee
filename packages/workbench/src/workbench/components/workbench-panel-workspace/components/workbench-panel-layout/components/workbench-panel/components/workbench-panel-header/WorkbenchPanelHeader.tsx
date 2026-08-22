import {
	Button,
	ContextMenu,
	ContextMenuTrigger,
	DropdownMenu,
	DropdownMenuTrigger,
	ProductIcon,
	Tooltip,
	TooltipContent,
	TooltipTrigger,
} from '@orchbee/ui/components';
import * as React from 'react';

import { WorkbenchActionContextMenuContent } from '../../../../../../../workbench-action-context-menu-content/WorkbenchActionContextMenuContent';
import { WorkbenchActionDropdownMenuContent } from '../../../../../../../workbench-action-dropdown-menu-content/WorkbenchActionDropdownMenuContent';
import { WORKBENCH_ACTION_IDS } from '../../../../../../../../actions/workbench-action-ids';
import type { WorkbenchActionMenuEntry } from '../../../../../../../../types/workbench-action-menu';
import {
	getDisplayedTerminal,
	getTerminalDisplayTitle,
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
	const isChildTerminalActive = panel.activeChildPresentationId !== null;
	const isMaximized = view.maximizedPanelId === panel.id;
	const closeShortcut = view.shortcutLabelForAction(WORKBENCH_ACTION_IDS.closeActiveTerminal);
	const panelMenuEntries: readonly WorkbenchActionMenuEntry[] = [
		{
			icon: isMaximized ? 'screen-normal' : 'screen-full',
			id: 'toggle-panel-maximization',
			kind: 'command',
			label: isMaximized ? view.messages.minimize : view.messages.maximize,
			onSelect: () => view.togglePanelMaximization(panel.id, 'menu'),
			shortcut: view.shortcutLabelForAction(WORKBENCH_ACTION_IDS.toggleActivePanelMaximization),
		},
	];

	const header = (
		<header
			className={styles['root']}
			data-drag-enabled={!isChildTerminalActive || undefined}
			data-slot='workbench-panel-header'
			onPointerDown={
				isChildTerminalActive
					? undefined
					: (event) => {
							if (event.button !== 0) return;
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
				{getTerminalDisplayTitle(displayedTerminal)}
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
								<ProductIcon name='ellipsis' />
							</DropdownMenuTrigger>
							<WorkbenchActionDropdownMenuContent entries={panelMenuEntries} />
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
								onClick={() => view.closeActiveTerminal(panel.id, 'button')}
								size='icon-xs'
								variant='ghost'
							/>
						}
					>
						<ProductIcon name='close' />
					</TooltipTrigger>
					<TooltipContent side='bottom'>
						{closeShortcut === undefined
							? view.messages.close
							: `${view.messages.close} (${closeShortcut})`}
					</TooltipContent>
				</Tooltip>
			</div>
		</header>
	);

	if (view.panelCount <= 1) return header;

	return (
		<ContextMenu>
			<ContextMenuTrigger render={<div className={styles['contextTrigger']} />}>
				{header}
			</ContextMenuTrigger>
			<WorkbenchActionContextMenuContent entries={panelMenuEntries} />
		</ContextMenu>
	);
}
