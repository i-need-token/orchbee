import type { TabData, TabGroupData, TabModelSnapshot } from '../../../../../model/tab-model';
import type { VerticalPinAnimation } from '../model/VerticalPinAnimationRuntime';

export type VerticalPinAnimationPhase = 'stable' | 'source' | 'destination';

export interface VerticalPinAnimationEntry {
	key: string;
	tab: Readonly<TabData>;
	order: number;
	phase: VerticalPinAnimationPhase;
	progress: number;
	mainAxisScale: number;
	opacity: number;
	retainedGroup: Readonly<TabGroupData> | null;
}

export interface VerticalPinAnimationEntries {
	pinned: readonly VerticalPinAnimationEntry[];
	unpinned: readonly VerticalPinAnimationEntry[];
}

function byOrder(left: VerticalPinAnimationEntry, right: VerticalPinAnimationEntry): number {
	return left.order - right.order;
}

export function buildVerticalPinAnimationEntries({
	snapshot,
	animations,
}: Readonly<{
	snapshot: TabModelSnapshot;
	animations: readonly VerticalPinAnimation[];
}>): VerticalPinAnimationEntries {
	const animationById = new Map(animations.map((animation) => [animation.tabId, animation]));
	const pinned: VerticalPinAnimationEntry[] = [];
	const unpinned: VerticalPinAnimationEntry[] = [];

	for (const [order, tab] of snapshot.tabs.entries()) {
		const animation = animationById.get(tab.id);
		const destination =
			animation &&
			((animation.direction === 'pinning' && tab.pinned) ||
				(animation.direction === 'unpinning' && !tab.pinned))
				? animation
				: null;
		const entry: VerticalPinAnimationEntry = {
			key: `tab:${tab.id}`,
			tab,
			order,
			phase: destination ? 'destination' : 'stable',
			progress: destination?.progress ?? 1,
			mainAxisScale: destination?.destinationMainAxisScale ?? 1,
			opacity: destination?.destinationOpacity ?? 1,
			retainedGroup: null,
		};
		if (tab.pinned) {
			pinned.push(entry);
		} else {
			unpinned.push(entry);
		}
	}

	for (const animation of animations) {
		const entry: VerticalPinAnimationEntry = {
			key: `pin-source:${animation.tabId}`,
			tab: animation.sourceTab,
			order: animation.sourceIndex,
			phase: 'source',
			progress: animation.progress,
			mainAxisScale: animation.sourceMainAxisScale,
			opacity: animation.sourceOpacity,
			retainedGroup: animation.sourceGroup,
		};
		if (animation.direction === 'unpinning') {
			pinned.push(entry);
		} else {
			unpinned.push(entry);
		}
	}

	pinned.sort(byOrder);
	unpinned.sort(byOrder);
	return { pinned, unpinned };
}
