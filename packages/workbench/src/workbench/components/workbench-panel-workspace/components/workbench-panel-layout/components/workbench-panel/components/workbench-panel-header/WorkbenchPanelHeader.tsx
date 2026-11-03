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
import {
	getDisplayedTerminal,
	getTerminalDisplayTitle,
	type PanelLeaf,
} from '../../../../../../model/panel-workspace-state';
import type { WorkbenchPanelWorkspaceView } from '../../../../../../types/panel-workspace-view';
import { buildWorkbenchPanelMenuEntries } from '../../utils/buildWorkbenchPanelMenuEntries';
import styles from './WorkbenchPanelHeader.module.css';

interface WorkbenchPanelHeaderProps {
	readonly panel: PanelLeaf;
	readonly view: WorkbenchPanelWorkspaceView;
}

export function WorkbenchPanelHeader({ panel, view }: WorkbenchPanelHeaderProps): React.ReactNode {
	const displayedTerminal = getDisplayedTerminal(panel);
	const isChildTerminalActive = panel.activeChildPresentationId !== null;
	const closeShortcut = view.shortcutLabelForAction(WORKBENCH_ACTION_IDS.closeActiveTerminal);
	const splitTookFocus = React.useRef(false);
	const panelMenuEntries = buildWorkbenchPanelMenuEntries({
		panelId: panel.id,
		source: 'menu',
		split: (direction) => {
			splitTookFocus.current = view.splitPanel(panel.id, direction, 'menu') !== null;
		},
		view,
	});
	// The new panel's terminal takes focus after a split; returning it to this header would undo that.
	const menuFinalFocus = (): boolean => {
		const tookFocus = splitTookFocus.current;
		splitTookFocus.current = false;
		return !tookFocus;
	};

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
						<WorkbenchActionDropdownMenuContent
							entries={panelMenuEntries}
							finalFocus={menuFinalFocus}
						/>
					</DropdownMenu>
					<TooltipContent side='bottom'>{view.messages.more}</TooltipContent>
				</Tooltip>
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

	return (
		<ContextMenu>
			<ContextMenuTrigger render={<div className={styles['contextTrigger']} />}>
				{header}
			</ContextMenuTrigger>
			<WorkbenchActionContextMenuContent entries={panelMenuEntries} finalFocus={menuFinalFocus} />
		</ContextMenu>
	);
}
