import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from '@orchbee/ui/components';
import * as React from 'react';

import {
	findPanel,
	type PanelLayoutNode,
	type PanelSplit,
} from '../../../../model/panel-workspace-state';
import type { WorkbenchPanelWorkspaceView } from '../../../../types/panel-workspace-view';
import {
	DISABLED_SEPARATOR_KEYS,
	MINIMUM_PANEL_SIZE_PX,
	PANEL_RESIZE_TARGET_SIZE_PX,
	RESIZE_ARROW_KEYS,
} from './constants/panel-resize';
import { useAdjacentPanelResize } from './hooks/useAdjacentPanelResize';
import { getPanelSeparatorIndexAtPoint } from './utils/getPanelSeparatorIndexAtPoint';
import { getPanelResizeId } from './utils/getPanelResizeId';
import { getPanelSeparatorId } from './utils/getPanelSeparatorId';
import styles from './WorkbenchPanelSplit.module.css';

interface WorkbenchPanelSplitProps {
	readonly renderNode: (node: PanelLayoutNode, layoutSettled: boolean) => React.ReactNode;
	readonly split: PanelSplit;
	readonly view: WorkbenchPanelWorkspaceView;
}

export function WorkbenchPanelSplit({
	renderNode,
	split,
	view,
}: WorkbenchPanelSplitProps): React.ReactNode {
	const maximizedPanelId = view.maximizedPanelId;
	const maximizedChildId =
		maximizedPanelId === null
			? null
			: (split.children.find((child) => findPanel(child.node, maximizedPanelId) !== null)?.id ??
				null);
	const resize = useAdjacentPanelResize(
		split,
		view.resizeAdjacentPanels,
		MINIMUM_PANEL_SIZE_PX,
		maximizedChildId,
	);

	return (
		<ResizablePanelGroup
			className={styles['group']}
			defaultLayout={resize.displayLayout}
			disableCursor
			elementRef={resize.groupElementRef}
			groupRef={resize.groupRef}
			id={split.id}
			onLayoutChange={resize.onLayoutChange}
			onPointerDownCapture={(event) => {
				if (event.button !== 0 || maximizedChildId !== null) {
					return;
				}
				const separators = [...event.currentTarget.children].filter(
					(child): child is HTMLElement =>
						child instanceof HTMLElement && child.dataset['slot'] === 'resizable-handle',
				);
				const hasCoarsePointer =
					event.currentTarget.ownerDocument.defaultView?.matchMedia('(pointer: coarse)').matches ===
					true;
				const separatorIndex = getPanelSeparatorIndexAtPoint(
					split.orientation,
					{ x: event.clientX, y: event.clientY },
					separators.map((separator) => separator.getBoundingClientRect()),
					hasCoarsePointer ? PANEL_RESIZE_TARGET_SIZE_PX.coarse : PANEL_RESIZE_TARGET_SIZE_PX.fine,
				);
				if (separatorIndex !== null) {
					resize.startPointerResize(separatorIndex, event.pointerId);
				}
			}}
			orientation={split.orientation}
			resizeTargetMinimumSize={PANEL_RESIZE_TARGET_SIZE_PX}
		>
			{split.children.map((child, index) => {
				const trailingChild = split.children[index + 1];
				const isHidden = maximizedChildId !== null && child.id !== maximizedChildId;

				return (
					<React.Fragment key={child.id}>
						<ResizablePanel hidden={isHidden} id={getPanelResizeId(split.id, child.id)}>
							{renderNode(child.node, resize.maximizationLayoutSettled)}
						</ResizablePanel>
						{trailingChild !== undefined && (
							<ResizableHandle
								className={styles['separator']}
								disableDoubleClick
								hidden={maximizedChildId !== null}
								id={getPanelSeparatorId(split.id, child.id, trailingChild.id)}
								onKeyDownCapture={(event) => {
									if (DISABLED_SEPARATOR_KEYS.has(event.key)) {
										event.preventDefault();
										return;
									}
									if (RESIZE_ARROW_KEYS[split.orientation].has(event.key)) {
										resize.startKeyboardResize(index);
									}
								}}
								onKeyUpCapture={(event) => {
									if (RESIZE_ARROW_KEYS[split.orientation].has(event.key)) {
										resize.finishKeyboardResize(index);
									}
								}}
							/>
						)}
					</React.Fragment>
				);
			})}
		</ResizablePanelGroup>
	);
}
