import { useMemoizedFn } from '@orchbee/foundation/react';
import type { ResizablePanelGroupHandle } from '@orchbee/ui/components';
import * as React from 'react';

import { matchesPanelLayoutTopology } from '../utils/matchesPanelLayoutTopology';
import { readPanelGroupLayout } from '../utils/readPanelGroupLayout';
import { writePanelGroupLayout } from '../utils/writePanelGroupLayout';

interface UseResizablePanelGroupInput {
	readonly canSynchronizeLayout: () => boolean;
	readonly childIds: readonly string[];
	readonly modelLayout: Readonly<Record<string, number>>;
	readonly onModelLayoutSynchronized?: (() => void) | undefined;
	readonly orientation: 'horizontal' | 'vertical';
}

interface UseResizablePanelGroupResult {
	readonly getOwnerDocument: () => Document | undefined;
	readonly groupElementRef: React.RefObject<HTMLDivElement | null>;
	readonly groupRef: React.RefObject<ResizablePanelGroupHandle | null>;
	readonly measureAvailablePanelSize: () => number;
	readonly readLayout: () => Readonly<Record<string, number>> | null;
	readonly requestLayoutSync: () => void;
	readonly writeLayout: (layout: Readonly<Record<string, number>>) => boolean;
}

export function useResizablePanelGroup({
	canSynchronizeLayout,
	childIds,
	modelLayout,
	onModelLayoutSynchronized,
	orientation,
}: UseResizablePanelGroupInput): UseResizablePanelGroupResult {
	const groupElementRef = React.useRef<HTMLDivElement | null>(null);
	const groupRef = React.useRef<ResizablePanelGroupHandle | null>(null);
	const [layoutSyncRevision, requestLayoutSync] = React.useReducer(
		(revision: number) => revision + 1,
		0,
	);
	const readLayout = useMemoizedFn(() => readPanelGroupLayout(groupRef.current));
	const writeLayout = useMemoizedFn((layout: Readonly<Record<string, number>>) =>
		writePanelGroupLayout(groupRef.current, layout),
	);
	const getOwnerDocument = useMemoizedFn(() => groupElementRef.current?.ownerDocument);
	const measureAvailablePanelSize = useMemoizedFn(() => {
		const groupElement = groupElementRef.current;
		if (groupElement === null) {
			return 0;
		}

		return [...groupElement.children].reduce((total, child) => {
			if (!(child instanceof HTMLElement) || child.dataset['slot'] !== 'resizable-panel') {
				return total;
			}
			const bounds = child.getBoundingClientRect();
			return total + (orientation === 'horizontal' ? bounds.width : bounds.height);
		}, 0);
	});

	React.useLayoutEffect(() => {
		if (!canSynchronizeLayout()) {
			return;
		}
		const primitiveLayout = readLayout();
		if (primitiveLayout === null || !matchesPanelLayoutTopology(primitiveLayout, childIds)) {
			return;
		}
		const needsSynchronization = childIds.some(
			(childId) =>
				typeof primitiveLayout[childId] !== 'number' ||
				Math.abs(primitiveLayout[childId] - (modelLayout[childId] ?? 0)) > 0.000_001,
		);
		if (needsSynchronization && !writeLayout(modelLayout)) {
			return;
		}
		onModelLayoutSynchronized?.();
	}, [
		canSynchronizeLayout,
		childIds,
		layoutSyncRevision,
		modelLayout,
		onModelLayoutSynchronized,
		readLayout,
		writeLayout,
	]);

	return {
		getOwnerDocument,
		groupElementRef,
		groupRef,
		measureAvailablePanelSize,
		readLayout,
		requestLayoutSync,
		writeLayout,
	};
}
