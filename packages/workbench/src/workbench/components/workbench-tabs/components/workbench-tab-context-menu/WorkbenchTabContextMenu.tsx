import {
	ContextMenu,
	ContextMenuCheckboxItem,
	ContextMenuContent,
	ContextMenuItem,
	ContextMenuSeparator,
	ContextMenuShortcut,
	ContextMenuSub,
	ContextMenuSubContent,
	ContextMenuSubTrigger,
	ContextMenuTrigger,
} from '@orchbee/ui/components';
import * as React from 'react';

import type { WorkbenchTabColor, WorkbenchTabGroup } from '../../../../types/workbench-tab';
import {
	WorkbenchColorPalette,
	type WorkbenchColorChoice,
} from '../workbench-color-palette/WorkbenchColorPalette';
import { activateWorkbenchContextMenuOutsideTarget } from '../../utils/activateWorkbenchContextMenuOutsideTarget';
import styles from './WorkbenchTabContextMenu.module.css';

interface WorkbenchTabContextMenuProps {
	readonly addToGroupLabel: string;
	readonly addToNewGroupLabel: string;
	readonly closeAllLabel: string;
	readonly closeLabel: string;
	readonly closeOthersLabel: string;
	readonly closeRightLabel: string;
	readonly closeShortcutLabel: string;
	readonly color: WorkbenchTabColor;
	readonly colorChoices: readonly WorkbenchColorChoice[];
	readonly currentGroupId: string | undefined;
	readonly groupChoices: readonly WorkbenchTabGroup[];
	readonly isPinned: boolean;
	readonly onClose?: (() => void) | undefined;
	readonly onCloseAll?: (() => void) | undefined;
	readonly onCloseOthers?: (() => void) | undefined;
	readonly onCloseRight?: (() => void) | undefined;
	readonly onColorChange?: ((color: WorkbenchTabColor) => void) | undefined;
	readonly onCreateGroup?: (() => void) | undefined;
	readonly onGroupAssign?: ((groupId: string) => void) | undefined;
	readonly onGroupRemove?: (() => void) | undefined;
	readonly onPinToggle?: (() => void) | undefined;
	readonly onRename?: (() => void) | undefined;
	readonly pinLabel: string;
	readonly renameLabel: string;
	readonly removeFromGroupLabel: string;
	readonly tabColorLabel: string;
	readonly trigger: React.ReactElement;
}

export function WorkbenchTabContextMenu({
	addToGroupLabel,
	addToNewGroupLabel,
	closeAllLabel,
	closeLabel,
	closeOthersLabel,
	closeRightLabel,
	closeShortcutLabel,
	color,
	colorChoices,
	currentGroupId,
	groupChoices,
	isPinned,
	onClose,
	onCloseAll,
	onCloseOthers,
	onCloseRight,
	onColorChange,
	onCreateGroup,
	onGroupAssign,
	onGroupRemove,
	onPinToggle,
	onRename,
	pinLabel,
	renameLabel,
	removeFromGroupLabel,
	tabColorLabel,
	trigger,
}: WorkbenchTabContextMenuProps): React.ReactNode {
	const availableGroupChoices = groupChoices.filter((group) => group.id !== currentGroupId);
	const canManageGroup =
		!isPinned &&
		(onCreateGroup !== undefined || onGroupAssign !== undefined || onGroupRemove !== undefined);
	if (
		onClose === undefined &&
		onCloseAll === undefined &&
		onCloseOthers === undefined &&
		onCloseRight === undefined &&
		onColorChange === undefined &&
		!canManageGroup &&
		onPinToggle === undefined &&
		onRename === undefined
	) {
		return trigger;
	}

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
				<ContextMenuItem closeOnClick disabled={onClose === undefined} onClick={onClose}>
					{closeLabel}
					<ContextMenuShortcut>{closeShortcutLabel}</ContextMenuShortcut>
				</ContextMenuItem>
				<ContextMenuItem
					closeOnClick
					disabled={onCloseOthers === undefined}
					onClick={onCloseOthers}
				>
					{closeOthersLabel}
				</ContextMenuItem>
				<ContextMenuItem closeOnClick disabled={onCloseRight === undefined} onClick={onCloseRight}>
					{closeRightLabel}
				</ContextMenuItem>
				<ContextMenuItem closeOnClick disabled={onCloseAll === undefined} onClick={onCloseAll}>
					{closeAllLabel}
				</ContextMenuItem>
				<ContextMenuSeparator />
				<ContextMenuCheckboxItem
					checked={isPinned}
					closeOnClick
					disabled={onPinToggle === undefined}
					onClick={onPinToggle}
				>
					{pinLabel}
				</ContextMenuCheckboxItem>
				<ContextMenuItem closeOnClick disabled={onRename === undefined} onClick={onRename}>
					{renameLabel}
				</ContextMenuItem>
				{canManageGroup && (
					<>
						<ContextMenuSeparator />
						<ContextMenuItem
							closeOnClick
							disabled={onCreateGroup === undefined}
							onClick={onCreateGroup}
						>
							{addToNewGroupLabel}
						</ContextMenuItem>
						{availableGroupChoices.length > 0 && (
							<ContextMenuSub>
								<ContextMenuSubTrigger disabled={onGroupAssign === undefined}>
									{addToGroupLabel}
								</ContextMenuSubTrigger>
								<ContextMenuSubContent>
									{availableGroupChoices.map((group) => (
										<ContextMenuItem
											key={group.id}
											closeOnClick
											onClick={() => {
												onGroupAssign?.(group.id);
											}}
										>
											<span className={styles['groupDot']} data-color={group.color} />
											{group.label}
										</ContextMenuItem>
									))}
								</ContextMenuSubContent>
							</ContextMenuSub>
						)}
						{currentGroupId !== undefined && (
							<ContextMenuItem
								closeOnClick
								disabled={onGroupRemove === undefined}
								onClick={onGroupRemove}
							>
								{removeFromGroupLabel}
							</ContextMenuItem>
						)}
					</>
				)}
				<ContextMenuSeparator />
				<WorkbenchColorPalette
					color={color}
					colorChoices={colorChoices}
					label={tabColorLabel}
					onColorChange={onColorChange}
				/>
			</ContextMenuContent>
		</ContextMenu>
	);
}
