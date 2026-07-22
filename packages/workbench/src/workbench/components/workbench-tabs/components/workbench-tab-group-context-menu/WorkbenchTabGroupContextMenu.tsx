import {
	ContextMenu,
	ContextMenuContent,
	ContextMenuItem,
	ContextMenuSeparator,
	ContextMenuTrigger,
} from '@orchbee/ui/components';
import * as React from 'react';

import type { WorkbenchTabGroupColor } from '../../../../types/workbench-tab';
import {
	WorkbenchColorPalette,
	type WorkbenchColorChoice,
} from '../workbench-color-palette/WorkbenchColorPalette';
import { activateWorkbenchContextMenuOutsideTarget } from '../../utils/activateWorkbenchContextMenuOutsideTarget';
import styles from './WorkbenchTabGroupContextMenu.module.css';

interface WorkbenchTabGroupContextMenuProps {
	readonly closeLabel: string;
	readonly color: WorkbenchTabGroupColor;
	readonly colorChoices: readonly WorkbenchColorChoice[];
	readonly colorLabel: string;
	readonly deleteLabel: string;
	readonly newTabLabel: string;
	readonly onClose: (() => void) | undefined;
	readonly onColorChange: ((color: WorkbenchTabGroupColor) => void) | undefined;
	readonly onDelete: (() => void) | undefined;
	readonly onNewTab: (() => void) | undefined;
	readonly onRename: (() => void) | undefined;
	readonly onUngroup: (() => void) | undefined;
	readonly renameLabel: string;
	readonly trigger: React.ReactElement;
	readonly ungroupLabel: string;
}

export function WorkbenchTabGroupContextMenu({
	closeLabel,
	color,
	colorChoices,
	colorLabel,
	deleteLabel,
	newTabLabel,
	onClose,
	onColorChange,
	onDelete,
	onNewTab,
	onRename,
	onUngroup,
	renameLabel,
	trigger,
	ungroupLabel,
}: WorkbenchTabGroupContextMenuProps): React.ReactNode {
	return (
		<ContextMenu
			onOpenChange={(open, eventDetails) => {
				if (!open && eventDetails.reason === 'outside-press') {
					activateWorkbenchContextMenuOutsideTarget(eventDetails.event);
				}
			}}
		>
			<ContextMenuTrigger render={trigger} />
			<ContextMenuContent className={styles['content']}>
				<ContextMenuItem closeOnClick disabled={onNewTab === undefined} onClick={onNewTab}>
					{newTabLabel}
				</ContextMenuItem>
				<ContextMenuItem closeOnClick disabled={onRename === undefined} onClick={onRename}>
					{renameLabel}
				</ContextMenuItem>
				<ContextMenuSeparator />
				<WorkbenchColorPalette
					color={color}
					colorChoices={colorChoices}
					label={colorLabel}
					onColorChange={
						onColorChange === undefined
							? undefined
							: (nextColor) => {
									if (nextColor !== 'default') onColorChange(nextColor);
								}
					}
				/>
				<ContextMenuSeparator />
				<ContextMenuItem closeOnClick disabled={onClose === undefined} onClick={onClose}>
					{closeLabel}
				</ContextMenuItem>
				<ContextMenuItem closeOnClick disabled={onUngroup === undefined} onClick={onUngroup}>
					{ungroupLabel}
				</ContextMenuItem>
				<ContextMenuItem
					closeOnClick
					disabled={onDelete === undefined}
					variant='destructive'
					onClick={onDelete}
				>
					{deleteLabel}
				</ContextMenuItem>
			</ContextMenuContent>
		</ContextMenu>
	);
}
