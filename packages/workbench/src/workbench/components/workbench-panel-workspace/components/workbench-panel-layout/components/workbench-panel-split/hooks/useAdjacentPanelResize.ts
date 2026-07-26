import type { ResizablePanelGroupHandle } from '@orchbee/ui/components';
import * as React from 'react';

import type { PanelSplit } from '../../../../../model/panel-workspace-state';
import { getPanelResizeId } from '../utils/getPanelResizeId';
import { matchesPanelLayoutTopology } from '../utils/matchesPanelLayoutTopology';
import { projectAdjacentPanelLayout } from '../utils/projectAdjacentPanelLayout';
import { readPanelGroupLayout } from '../utils/readPanelGroupLayout';
import { writePanelGroupLayout } from '../utils/writePanelGroupLayout';

interface ActiveResize {
	readonly availablePanelSize: number;
	readonly childIds: readonly string[];
	readonly initialLayout: Readonly<Record<string, number>>;
	readonly latestLeadingWeight: number | null;
	readonly manualPointerStart: number | null;
	readonly minimumChildWeight: number;
	readonly onResize: (splitId: string, separatorIndex: number, leadingWeight: number) => void;
	readonly separatorIndex: number;
	readonly splitId: string;
}

interface PointerResizeStart {
	readonly position: number;
}

interface UseAdjacentPanelResizeResult {
	readonly finishResize: () => void;
	readonly groupElementRef: React.RefObject<HTMLDivElement | null>;
	readonly groupRef: React.RefObject<ResizablePanelGroupHandle | null>;
	readonly modelLayout: Readonly<Record<string, number>>;
	readonly onLayoutChange: (candidateLayout: Record<string, number>) => void;
	readonly startResize: (separatorIndex: number, pointer?: PointerResizeStart) => void;
}

export function useAdjacentPanelResize(
	split: PanelSplit,
	onResize: (splitId: string, separatorIndex: number, leadingWeight: number) => void,
	minimumPanelSizePx: number,
): UseAdjacentPanelResizeResult {
	const groupElementRef = React.useRef<HTMLDivElement | null>(null);
	const groupRef = React.useRef<ResizablePanelGroupHandle | null>(null);
	const activeResizeRef = React.useRef<ActiveResize | null>(null);
	const correctingLayoutRef = React.useRef(false);
	const [layoutSyncRevision, requestLayoutSync] = React.useReducer(
		(revision: number) => revision + 1,
		0,
	);
	const childIds = React.useMemo(
		() => split.children.map((child) => getPanelResizeId(split.id, child.id)),
		[split],
	);
	const modelLayout = React.useMemo(
		() =>
			Object.fromEntries(
				split.children.map((child) => [getPanelResizeId(split.id, child.id), child.weight]),
			),
		[split],
	);
	const moveResize = React.useCallback((pointerPosition: number) => {
		const activeResize = activeResizeRef.current;
		if (
			activeResize === null ||
			activeResize.manualPointerStart === null ||
			activeResize.availablePanelSize <= 0
		) {
			return;
		}
		const leadingId = activeResize.childIds[activeResize.separatorIndex];
		const trailingId = activeResize.childIds[activeResize.separatorIndex + 1];
		const initialLeading =
			leadingId === undefined ? undefined : activeResize.initialLayout[leadingId];
		const initialTrailing =
			trailingId === undefined ? undefined : activeResize.initialLayout[trailingId];
		if (
			leadingId === undefined ||
			trailingId === undefined ||
			initialLeading === undefined ||
			initialTrailing === undefined
		) {
			return;
		}
		const delta =
			((pointerPosition - activeResize.manualPointerStart) / activeResize.availablePanelSize) * 100;
		const projection = projectAdjacentPanelLayout(
			activeResize.childIds,
			activeResize.separatorIndex,
			activeResize.initialLayout,
			{
				...activeResize.initialLayout,
				[leadingId]: initialLeading + delta,
				[trailingId]: initialTrailing - delta,
			},
			activeResize.minimumChildWeight,
		);
		if (projection === null || projection.leadingWeight === activeResize.latestLeadingWeight) {
			return;
		}
		activeResizeRef.current = {
			...activeResize,
			latestLeadingWeight: projection.leadingWeight,
		};
		correctingLayoutRef.current = true;
		try {
			writePanelGroupLayout(groupRef.current, projection.layout);
		} finally {
			correctingLayoutRef.current = false;
		}
		activeResize.onResize(
			activeResize.splitId,
			activeResize.separatorIndex,
			projection.leadingWeight,
		);
	}, []);
	const finishResize = React.useCallback(() => {
		if (activeResizeRef.current === null) {
			return;
		}
		activeResizeRef.current = null;
		requestLayoutSync();
	}, []);

	React.useEffect(() => {
		const ownerDocument = groupElementRef.current?.ownerDocument;
		if (ownerDocument === undefined) {
			return;
		}
		const handlePointerMove = (event: PointerEvent) => {
			const activeResize = activeResizeRef.current;
			if (activeResize === null || activeResize.manualPointerStart === null) {
				return;
			}
			moveResize(split.orientation === 'horizontal' ? event.clientX : event.clientY);
			event.preventDefault();
			event.stopPropagation();
		};
		ownerDocument.addEventListener('lostpointercapture', finishResize, true);
		ownerDocument.addEventListener('pointercancel', finishResize, true);
		ownerDocument.addEventListener('pointermove', handlePointerMove, true);
		ownerDocument.addEventListener('pointerup', finishResize, true);
		return () => {
			ownerDocument.removeEventListener('lostpointercapture', finishResize, true);
			ownerDocument.removeEventListener('pointercancel', finishResize, true);
			ownerDocument.removeEventListener('pointermove', handlePointerMove, true);
			ownerDocument.removeEventListener('pointerup', finishResize, true);
		};
	}, [finishResize, moveResize, split.orientation]);

	React.useLayoutEffect(() => {
		if (activeResizeRef.current !== null) {
			return;
		}
		const imperativeLayout = readPanelGroupLayout(groupRef.current);
		if (
			imperativeLayout !== null &&
			matchesPanelLayoutTopology(imperativeLayout, childIds) &&
			childIds.some(
				(childId) =>
					typeof imperativeLayout[childId] !== 'number' ||
					Math.abs(imperativeLayout[childId] - (modelLayout[childId] ?? 0)) > 0.000_001,
			)
		) {
			writePanelGroupLayout(groupRef.current, modelLayout);
		}
	}, [childIds, layoutSyncRevision, modelLayout]);

	return {
		finishResize,
		groupElementRef,
		groupRef,
		modelLayout,
		onLayoutChange: (candidateLayout) => {
			const activeResize = activeResizeRef.current;
			if (activeResize === null || correctingLayoutRef.current) {
				return;
			}
			const projection = projectAdjacentPanelLayout(
				childIds,
				activeResize.separatorIndex,
				activeResize.initialLayout,
				candidateLayout,
				activeResize.minimumChildWeight,
			);
			if (projection === null) {
				return;
			}
			const candidateNeedsCorrection = childIds.some(
				(childId) =>
					Math.abs((candidateLayout[childId] ?? 0) - (projection.layout[childId] ?? 0)) > 0.000_001,
			);
			if (candidateNeedsCorrection) {
				correctingLayoutRef.current = true;
				writePanelGroupLayout(groupRef.current, projection.layout);
				correctingLayoutRef.current = false;
			}
			onResize(split.id, activeResize.separatorIndex, projection.leadingWeight);
		},
		startResize: (separatorIndex, pointer) => {
			const groupElement = groupElementRef.current;
			const imperativeLayout = readPanelGroupLayout(groupRef.current);
			const initialLayout =
				imperativeLayout !== null && matchesPanelLayoutTopology(imperativeLayout, childIds)
					? imperativeLayout
					: modelLayout;
			const availablePanelSize =
				groupElement === null
					? 0
					: [...groupElement.children].reduce((total, child) => {
							if (!(child instanceof HTMLElement) || child.dataset['slot'] !== 'resizable-panel') {
								return total;
							}
							const bounds = child.getBoundingClientRect();
							return total + (split.orientation === 'horizontal' ? bounds.width : bounds.height);
						}, 0);
			activeResizeRef.current = {
				availablePanelSize,
				childIds,
				initialLayout,
				latestLeadingWeight: null,
				manualPointerStart: pointer?.position ?? null,
				minimumChildWeight:
					availablePanelSize > 0 ? (minimumPanelSizePx / availablePanelSize) * 100 : 0,
				onResize,
				separatorIndex,
				splitId: split.id,
			};
		},
	};
}
