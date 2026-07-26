import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from '@orchbee/ui/components';
import * as React from 'react';

import type { PanelLayoutNode, PanelSplit } from '../../../../model/panel-workspace-state';
import type { WorkbenchPanelWorkspaceView } from '../../../../types/panel-workspace-view';
import { useAdjacentPanelResize } from './hooks/useAdjacentPanelResize';
import { getPanelSeparatorIndexAtPoint } from './utils/getPanelSeparatorIndexAtPoint';
import { getPanelResizeId } from './utils/getPanelResizeId';
import { getPanelSeparatorId } from './utils/getPanelSeparatorId';
import styles from './WorkbenchPanelSplit.module.css';

interface WorkbenchPanelSplitProps {
	readonly renderNode: (node: PanelLayoutNode) => React.ReactNode;
	readonly split: PanelSplit;
	readonly view: WorkbenchPanelWorkspaceView;
}

const MINIMUM_PANEL_SIZE_PX = 30;
const PANEL_RESIZE_TARGET_SIZE_PX = {
	coarse: 20,
	fine: 10,
} as const;

export function WorkbenchPanelSplit({
	renderNode,
	split,
	view,
}: WorkbenchPanelSplitProps): React.ReactNode {
	const resize = useAdjacentPanelResize(split, view.resizeAdjacentPanels, MINIMUM_PANEL_SIZE_PX);

	return (
		<ResizablePanelGroup
			className={styles['group']}
			defaultLayout={resize.modelLayout}
			elementRef={resize.groupElementRef}
			groupRef={resize.groupRef}
			id={split.id}
			onLayoutChange={resize.onLayoutChange}
			onPointerCancelCapture={resize.finishResize}
			onPointerDownCapture={(event) => {
				if (event.button !== 0) {
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
					resize.startResize(separatorIndex, {
						position: split.orientation === 'horizontal' ? event.clientX : event.clientY,
					});
					separators[separatorIndex]?.setPointerCapture(event.pointerId);
				}
			}}
			onPointerUpCapture={resize.finishResize}
			orientation={split.orientation}
			resizeTargetMinimumSize={PANEL_RESIZE_TARGET_SIZE_PX}
		>
			{split.children.map((child, index) => {
				const trailingChild = split.children[index + 1];

				return (
					<React.Fragment key={child.id}>
						<ResizablePanel id={getPanelResizeId(split.id, child.id)}>
							{renderNode(child.node)}
						</ResizablePanel>
						{trailingChild !== undefined && (
							<ResizableHandle
								className={styles['separator']}
								disableDoubleClick
								id={getPanelSeparatorId(split.id, child.id, trailingChild.id)}
								onKeyDownCapture={(event) => {
									if (
										event.key === 'ArrowDown' ||
										event.key === 'ArrowLeft' ||
										event.key === 'ArrowRight' ||
										event.key === 'ArrowUp'
									) {
										resize.startResize(index);
									}
								}}
								onKeyUpCapture={resize.finishResize}
							/>
						)}
					</React.Fragment>
				);
			})}
		</ResizablePanelGroup>
	);
}
