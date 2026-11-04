import { useMemoizedFn } from '@orchbee/foundation/react';
import * as React from 'react';

import type { PanelSplit } from '../../../../../model/panel-workspace-state';
import type { SharedSplitResize } from '../../../../../types/panel-workspace-view';
import { getPanelResizeId } from '../utils/getPanelResizeId';
import { matchesPanelLayoutTopology } from '../utils/matchesPanelLayoutTopology';
import { reconcileAdjacentPanelLayout } from '../utils/reconcileAdjacentPanelLayout';
import { useResizablePanelGroup } from './useResizablePanelGroup';

interface ActiveResizeBase {
	readonly childIds: readonly string[];
	readonly latestLayout: Readonly<Record<string, number>>;
	readonly minimumPanelSizePx: number;
	readonly separatorIndex: number;
	readonly shared: SharedSplitResize;
	readonly splitId: string;
	/** What a shared divider will submit when released. Null while nothing has moved. */
	readonly unsubmittedLeadingWeight: number | null;
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
	minimumPanelSizePx: number,
	maximizedChildId: string | null,
	shared: SharedSplitResize,
): UseAdjacentPanelResizeResult {
	const activeResizeRef = React.useRef<ActiveResize | null>(null);
	const correctingLayoutRef = React.useRef(false);
	/** Commits still unanswered: the released divider stays where it is until the layout says. */
	const settlingCommitsRef = React.useRef(0);
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
	const canSynchronizeLayout = useMemoizedFn(
		() => activeResizeRef.current === null && settlingCommitsRef.current === 0,
	);
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
				minimumChildPercent: activeResize.shared.minimumChildPercent,
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
				unsubmittedLeadingWeight: reconciliation.modelNeedsUpdate
					? reconciliation.leadingWeight
					: activeResize.unsubmittedLeadingWeight,
			};
			if (writeAcceptedLayout || reconciliation.candidateNeedsCorrection) {
				correctingLayoutRef.current = true;
				try {
					writeLayout(reconciliation.layout);
				} finally {
					correctingLayoutRef.current = false;
				}
			}
		},
	);
	const finishResize = useMemoizedFn(
		(predicate: (activeResize: ActiveResize) => boolean, released: boolean) => {
			const activeResize = activeResizeRef.current;
			if (activeResize === null || !predicate(activeResize)) {
				return;
			}
			activeResizeRef.current = null;
			if (!released || activeResize.unsubmittedLeadingWeight === null) {
				requestLayoutSync();
				return;
			}
			settlingCommitsRef.current += 1;
			void activeResize.shared
				.commit(activeResize.splitId, activeResize.unsubmittedLeadingWeight)
				.finally(() => {
					settlingCommitsRef.current -= 1;
					requestLayoutSync();
				});
		},
	);
	const finishKeyboardResize = useMemoizedFn((separatorIndex: number) => {
		finishResize(
			(activeResize) =>
				activeResize.kind === 'keyboard' && activeResize.separatorIndex === separatorIndex,
			true,
		);
	});
	const finishPointerResize = useMemoizedFn((pointerId: number, released: boolean) => {
		finishResize(
			(activeResize) => activeResize.kind === 'pointer' && activeResize.pointerId === pointerId,
			released,
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
				separatorIndex,
				shared,
				splitId: split.id,
				unsubmittedLeadingWeight: null,
			};
		},
	);

	React.useEffect(() => {
		const ownerDocument = getOwnerDocument();
		if (ownerDocument === undefined) {
			return;
		}
		const handlePointerCompletion = (event: PointerEvent) => {
			finishPointerResize(event.pointerId, event.type === 'pointerup');
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
