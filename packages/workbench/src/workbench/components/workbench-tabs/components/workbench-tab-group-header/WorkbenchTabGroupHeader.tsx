import { Button } from '@orchbee/ui/components';
import * as React from 'react';

import type { WorkbenchTabGroup, WorkbenchTabGroupColor } from '../../../../types/workbench-tab';
import type { WorkbenchColorChoice } from '../workbench-color-palette/WorkbenchColorPalette';
import { WorkbenchTabGroupContextMenu } from '../workbench-tab-group-context-menu/WorkbenchTabGroupContextMenu';
import styles from './WorkbenchTabGroupHeader.module.css';

interface WorkbenchTabGroupDragProperties {
	readonly 'data-dragging': boolean | undefined;
	readonly onClickCapture: React.MouseEventHandler<HTMLDivElement>;
	readonly onLostPointerCapture: React.PointerEventHandler<HTMLDivElement>;
	readonly onPointerCancel: React.PointerEventHandler<HTMLDivElement>;
	readonly onPointerDown: React.PointerEventHandler<HTMLDivElement>;
	readonly onPointerMove: React.PointerEventHandler<HTMLDivElement>;
	readonly onPointerUp: React.PointerEventHandler<HTMLDivElement>;
	readonly reorderable: boolean;
}

interface WorkbenchTabGroupHeaderProps {
	readonly closeLabel: string;
	readonly collapseLabel: string;
	readonly colorChoices: readonly WorkbenchColorChoice[];
	readonly colorLabel: string;
	readonly deleteLabel: string;
	readonly dragProperties: WorkbenchTabGroupDragProperties;
	readonly group: WorkbenchTabGroup;
	readonly newTabLabel: string;
	readonly onClose: (() => void) | undefined;
	readonly onCollapseToggle: (() => void) | undefined;
	readonly onColorChange: ((color: WorkbenchTabGroupColor) => void) | undefined;
	readonly onDelete: (() => void) | undefined;
	readonly onNewTab: (() => void) | undefined;
	readonly onRename: (() => void) | undefined;
	readonly onUngroup: (() => void) | undefined;
	readonly renameLabel: string;
	readonly ungroupLabel: string;
}

export function WorkbenchTabGroupHeader({
	closeLabel,
	collapseLabel,
	colorChoices,
	colorLabel,
	deleteLabel,
	dragProperties,
	group,
	newTabLabel,
	onClose,
	onCollapseToggle,
	onColorChange,
	onDelete,
	onNewTab,
	onRename,
	onUngroup,
	renameLabel,
	ungroupLabel,
}: WorkbenchTabGroupHeaderProps): React.ReactNode {
	return (
		<div
			className={styles['groupItem']}
			data-color={group.color}
			data-group-dragging={dragProperties['data-dragging']}
			data-workbench-tab-group-expanded={group.isCollapsed === true ? undefined : true}
			data-workbench-tab-group-reorderable={dragProperties.reorderable ? true : undefined}
			data-workbench-tab-motion-key={`group:${group.id}`}
			data-workbench-tab-visual-item='group'
			onClickCapture={dragProperties.onClickCapture}
			onLostPointerCapture={dragProperties.onLostPointerCapture}
			onPointerCancel={dragProperties.onPointerCancel}
			onPointerDown={dragProperties.onPointerDown}
			onPointerMove={dragProperties.onPointerMove}
			onPointerUp={dragProperties.onPointerUp}
		>
			<WorkbenchTabGroupContextMenu
				closeLabel={closeLabel}
				color={group.color}
				colorChoices={colorChoices}
				colorLabel={colorLabel}
				deleteLabel={deleteLabel}
				newTabLabel={newTabLabel}
				onClose={onClose}
				onColorChange={onColorChange}
				onDelete={onDelete}
				onNewTab={onNewTab}
				onRename={onRename}
				onUngroup={onUngroup}
				renameLabel={renameLabel}
				trigger={
					<Button
						aria-expanded={group.isCollapsed !== true}
						aria-label={collapseLabel}
						className={styles['trigger']}
						data-color={group.color}
						data-workbench-tab-group-id={group.id}
						size='xs'
						type='button'
						variant='ghost'
						onClick={onCollapseToggle}
					>
						<span className={styles['groupLabel']}>{group.label}</span>
					</Button>
				}
				ungroupLabel={ungroupLabel}
			/>
		</div>
	);
}
