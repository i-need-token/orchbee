import * as React from 'react';

import type { WorkbenchTab } from '../../../types/workbench-tab';
import { resolveTabDropEdge } from '../utils/resolveTabDropEdge';
import { resolveTabDropPosition } from '../utils/resolveTabDropPosition';

interface WorkbenchTabDragProperties {
	readonly 'data-dragging': boolean | undefined;
	readonly draggable: boolean;
	readonly onDragEnd: React.DragEventHandler<HTMLDivElement>;
	readonly onDragOver: React.DragEventHandler<HTMLDivElement>;
	readonly onDragStart: React.DragEventHandler<HTMLDivElement>;
	readonly onDrop: React.DragEventHandler<HTMLDivElement>;
}

interface WorkbenchTabDrag {
	readonly dropBoundaryIndex: number | undefined;
	readonly getTabDragProperties: (tabId: string) => WorkbenchTabDragProperties;
	readonly hoverSuppressed: boolean;
	readonly onPointerMove: React.PointerEventHandler<HTMLDivElement>;
}

export function useWorkbenchTabDrag(
	tabs: readonly WorkbenchTab[],
	onTabMove: ((tabId: string, targetIndex: number) => void) | undefined,
	dragPreviewClassName: string,
): WorkbenchTabDrag {
	const [draggedTabId, setDraggedTabId] = React.useState<string>();
	const [dropBoundaryIndex, setDropBoundaryIndex] = React.useState<number>();
	const [hoverSuppressed, setHoverSuppressed] = React.useState(false);
	const finishDrag = () => {
		setDraggedTabId(undefined);
		setDropBoundaryIndex(undefined);
		setHoverSuppressed(true);
	};

	const resolveEventDropPosition = (event: React.DragEvent<HTMLDivElement>, tabId: string) => {
		const bounds = event.currentTarget.getBoundingClientRect();
		return resolveTabDropPosition(
			tabs.map((tab) => tab.id),
			draggedTabId,
			tabId,
			resolveTabDropEdge(event.clientX, bounds.left, bounds.width),
		);
	};

	const getTabDragProperties = (tabId: string): WorkbenchTabDragProperties => {
		return {
			'data-dragging': draggedTabId === tabId ? true : undefined,
			draggable: onTabMove !== undefined,
			onDragEnd: finishDrag,
			onDragOver: (event) => {
				if (draggedTabId === undefined) return;

				event.preventDefault();
				event.dataTransfer.dropEffect = 'move';
				setDropBoundaryIndex(resolveEventDropPosition(event, tabId)?.boundaryIndex);
			},
			onDragStart: (event) => {
				if (onTabMove === undefined) {
					event.preventDefault();
					return;
				}

				const eventTarget = event.target;
				if (
					eventTarget instanceof Element &&
					eventTarget.closest('[data-workbench-tab-action]') !== null
				) {
					event.preventDefault();
					return;
				}

				event.dataTransfer.effectAllowed = 'move';
				event.dataTransfer.setData('text/plain', tabId);
				const bounds = event.currentTarget.getBoundingClientRect();
				const dragPreview = event.currentTarget.ownerDocument.createElement('div');
				dragPreview.className = dragPreviewClassName;
				dragPreview.textContent = tabs.find((tab) => tab.id === tabId)?.label ?? '';
				dragPreview.style.width = `${bounds.width}px`;
				dragPreview.style.height = `${bounds.height}px`;
				event.currentTarget.ownerDocument.body.append(dragPreview);
				event.dataTransfer.setDragImage(
					dragPreview,
					Math.max(0, Math.min(event.clientX - bounds.left, bounds.width)),
					bounds.height / 2,
				);
				const browserWindow = event.currentTarget.ownerDocument.defaultView;
				if (browserWindow === null) dragPreview.remove();
				else browserWindow.requestAnimationFrame(() => dragPreview.remove());
				setDraggedTabId(tabId);
				setDropBoundaryIndex(undefined);
			},
			onDrop: (event) => {
				if (draggedTabId === undefined) return;

				event.preventDefault();
				const dropPosition = resolveEventDropPosition(event, tabId);
				if (dropPosition !== undefined) onTabMove?.(draggedTabId, dropPosition.targetIndex);
				finishDrag();
			},
		};
	};

	return {
		dropBoundaryIndex,
		getTabDragProperties,
		hoverSuppressed,
		onPointerMove: () => {
			if (hoverSuppressed) setHoverSuppressed(false);
		},
	};
}
