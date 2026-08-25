import { useMemoizedFn } from '@orchbee/foundation/react';
import * as React from 'react';

import type { PanelSplit } from '../../../../../model/panel-workspace-state';
import { getPanelResizeId } from '../utils/getPanelResizeId';
import { matchesPanelLayoutTopology } from '../utils/matchesPanelLayoutTopology';
import { reconcileAdjacentPanelLayout } from '../utils/reconcileAdjacentPanelLayout';
import { useResizablePanelGroup } from './useResizablePanelGroup';

interface ActiveResizeBase {
	readonly childIds: readonly string[];
	readonly latestLayout: Readonly<Record<string, number>>;
	readonly minimumPanelSizePx: number;
	readonly onResize: (splitId: string, separatorIndex: number, leadingWeight: number) => void;
	readonly separatorIndex: number;
	readonly splitId: string;
}

type ActiveResize =
	| (ActiveResizeBase & {
			readonly kind: 'keyboard';
	  })
	| (ActiveResizeBase & {
			readonly kind: 'pointer';
			readonly pointerId: number;
	  });

interface UseAdjacentPanelResizeResult {
	readonly displayLayout: Readonly<Record<string, number>>;
	readonly finishKeyboardResize: (separatorIndex: number) => void;
	readonly groupElementRef: React.RefObject<HTMLDivElement | null>;
	readonly groupRef: ReturnType<typeof useResizablePanelGroup>['groupRef'];
	readonly maximizationLayoutSettled: boolean;
	readonly onLayoutChange: (candidateLayout: Record<string, number>) => void;
	readonly startKeyboardResize: (separatorIndex: number) => void;
	readonly startPointerResize: (separatorIndex: number, pointerId: number) => void;
}

export function useAdjacentPanelResize(
	split: PanelSplit,
	onResize: (splitId: string, separatorIndex: number, leadingWeight: number) => void,
	minimumPanelSizePx: number,
	maximizedChildId: string | null = null,
): UseAdjacentPanelResizeResult {
	const activeResizeRef = React.useRef<ActiveResize | null>(null);
	const correctingLayoutRef = React.useRef(false);
	const [synchronizedMaximizedChildId, setSynchronizedMaximizedChildId] = React.useState<
		string | null | undefined
	>(maximizedChildId === null ? null : undefined);
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
	const displayLayout = React.useMemo(
		() =>
			maximizedChildId === null
				? modelLayout
				: Object.fromEntries(
						split.children.map((child) => [
							getPanelResizeId(split.id, child.id),
							child.id === maximizedChildId ? 100 : 0,
						]),
					),
		[modelLayout, maximizedChildId, split],
	);
	const canSynchronizeLayout = useMemoizedFn(() => activeResizeRef.current === null);
	const markModelLayoutSynchronized = useMemoizedFn(() => {
		setSynchronizedMaximizedChildId((current) =>
			current === maximizedChildId ? current : maximizedChildId,
		);
	});
	const {
		getOwnerDocument,
		groupElementRef,
		groupRef,
		measureAvailablePanelSize,
		readLayout,
		requestLayoutSync,
		writeLayout,
	} = useResizablePanelGroup({
		canSynchronizeLayout,
		childIds,
		modelLayout: displayLayout,
		onModelLayoutSynchronized: markModelLayoutSynchronized,
		orientation: split.orientation,
	});
	React.useLayoutEffect(() => {
		if (maximizedChildId === null || activeResizeRef.current === null) {
			return;
		}
		activeResizeRef.current = null;
		requestLayoutSync();
	}, [maximizedChildId, requestLayoutSync]);
	const applyCandidateLayout = useMemoizedFn(
		(candidateLayout: Readonly<Record<string, number>>, writeAcceptedLayout = false) => {
			const activeResize = activeResizeRef.current;
			if (activeResize === null || correctingLayoutRef.current || maximizedChildId !== null) {
				return;
			}
			const reconciliation = reconcileAdjacentPanelLayout({
				availablePanelSizePx: measureAvailablePanelSize(),
				candidateLayout,
				childIds: activeResize.childIds,
				minimumPanelSizePx: activeResize.minimumPanelSizePx,
				modelLayout: activeResize.latestLayout,
			});
			if (
				reconciliation.kind !== 'accepted' ||
				reconciliation.separatorIndex !== activeResize.separatorIndex
			) {
				return;
			}
			activeResizeRef.current = {
				...activeResize,
				latestLayout: reconciliation.layout,
			};
			if (writeAcceptedLayout || reconciliation.candidateNeedsCorrection) {
				correctingLayoutRef.current = true;
				try {
					writeLayout(reconciliation.layout);
				} finally {
					correctingLayoutRef.current = false;
				}
			}
			if (reconciliation.modelNeedsUpdate) {
				activeResize.onResize(
					activeResize.splitId,
					reconciliation.separatorIndex,
					reconciliation.leadingWeight,
				);
			}
		},
	);
	const finishResize = useMemoizedFn((predicate: (activeResize: ActiveResize) => boolean) => {
		const activeResize = activeResizeRef.current;
		if (activeResize === null || !predicate(activeResize)) {
			return;
		}
		activeResizeRef.current = null;
		requestLayoutSync();
	});
	const finishKeyboardResize = useMemoizedFn((separatorIndex: number) => {
		finishResize(
			(activeResize) =>
				activeResize.kind === 'keyboard' && activeResize.separatorIndex === separatorIndex,
		);
	});
	const finishPointerResize = useMemoizedFn((pointerId: number) => {
		finishResize(
			(activeResize) => activeResize.kind === 'pointer' && activeResize.pointerId === pointerId,
		);
	});
	const startResize = useMemoizedFn(
		(
			separatorIndex: number,
			source:
				| { readonly kind: 'keyboard' }
				| { readonly kind: 'pointer'; readonly pointerId: number },
		) => {
			if (activeResizeRef.current !== null || maximizedChildId !== null) {
				return;
			}
			const imperativeLayout = readLayout();
			const initialLayout =
				imperativeLayout !== null && matchesPanelLayoutTopology(imperativeLayout, childIds)
					? imperativeLayout
					: modelLayout;
			activeResizeRef.current = {
				...source,
				childIds,
				latestLayout: initialLayout,
				minimumPanelSizePx,
				onResize,
				separatorIndex,
				splitId: split.id,
			};
		},
	);

	React.useEffect(() => {
		const ownerDocument = getOwnerDocument();
		if (ownerDocument === undefined) {
			return;
		}
		const handlePointerCompletion = (event: PointerEvent) => {
			finishPointerResize(event.pointerId);
		};
		ownerDocument.addEventListener('lostpointercapture', handlePointerCompletion, true);
		ownerDocument.addEventListener('pointercancel', handlePointerCompletion, true);
		ownerDocument.addEventListener('pointerup', handlePointerCompletion, true);
		return () => {
			ownerDocument.removeEventListener('lostpointercapture', handlePointerCompletion, true);
			ownerDocument.removeEventListener('pointercancel', handlePointerCompletion, true);
			ownerDocument.removeEventListener('pointerup', handlePointerCompletion, true);
		};
	}, [finishPointerResize, getOwnerDocument]);

	return {
		displayLayout,
		finishKeyboardResize,
		groupElementRef,
		groupRef,
		maximizationLayoutSettled: synchronizedMaximizedChildId === maximizedChildId,
		onLayoutChange: applyCandidateLayout,
		startKeyboardResize: (separatorIndex) => {
			startResize(separatorIndex, { kind: 'keyboard' });
		},
		startPointerResize: (separatorIndex, pointerId) => {
			startResize(separatorIndex, { kind: 'pointer', pointerId });
		},
	};
}
