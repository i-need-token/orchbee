import { tabCollectionEaseInOut } from '../../../../../layout/tab-collection-animating-layout-manager';
import type { TabData, TabGroupData, TabModelSnapshot } from '../../../../../model/tab-model';

export type VerticalPinAnimationDirection = 'pinning' | 'unpinning';

export interface VerticalPinAnimation {
	tabId: string;
	direction: VerticalPinAnimationDirection;
	sourceTab: Readonly<TabData>;
	sourceGroup: Readonly<TabGroupData> | null;
	sourceIndex: number;
	progress: number;
	sourceMainAxisScale: number;
	sourceOpacity: number;
	destinationMainAxisScale: number;
	destinationOpacity: number;
}

interface TimedVerticalPinAnimation extends VerticalPinAnimation {
	startedAt: number;
}

function cloneAnimation(animation: Readonly<TimedVerticalPinAnimation>): VerticalPinAnimation {
	return {
		tabId: animation.tabId,
		direction: animation.direction,
		sourceTab: { ...animation.sourceTab },
		sourceGroup: animation.sourceGroup === null ? null : { ...animation.sourceGroup },
		sourceIndex: animation.sourceIndex,
		progress: animation.progress,
		sourceMainAxisScale: animation.sourceMainAxisScale,
		sourceOpacity: animation.sourceOpacity,
		destinationMainAxisScale: animation.destinationMainAxisScale,
		destinationOpacity: animation.destinationOpacity,
	};
}

/**
 * Retains the source-side tab while a pin-state move creates its destination.
 *
 * Chromium models this boundary crossing as two concurrent collection
 * animations, so one logical tab temporarily has a source and destination
 * presentation.
 */
export class VerticalPinAnimationRuntime {
	private previousTabs: readonly Readonly<TabData>[] | null = null;
	private previousGroups: readonly Readonly<TabGroupData>[] = [];
	private readonly animations = new Map<string, TimedVerticalPinAnimation>();

	public get currentAnimations(): readonly VerticalPinAnimation[] {
		return [...this.animations.values()].map(cloneAnimation);
	}

	public get isAnimating(): boolean {
		return this.animations.size > 0;
	}

	public sync(snapshot: Readonly<TabModelSnapshot>, now: number): boolean {
		if (this.previousTabs === null) {
			this.capture(snapshot);
			return false;
		}

		const currentById = new Map(snapshot.tabs.map((tab) => [tab.id, tab]));
		let changed = false;
		for (const [sourceIndex, previousTab] of this.previousTabs.entries()) {
			const currentTab = currentById.get(previousTab.id);
			if (!currentTab || currentTab.pinned === previousTab.pinned) continue;
			const sourceGroup =
				previousTab.group === null
					? null
					: (this.previousGroups.find((group) => group.id === previousTab.group) ?? null);
			this.animations.set(previousTab.id, {
				tabId: previousTab.id,
				direction: currentTab.pinned ? 'pinning' : 'unpinning',
				sourceTab: { ...previousTab },
				sourceGroup: sourceGroup === null ? null : { ...sourceGroup },
				sourceIndex,
				progress: 0,
				sourceMainAxisScale: 1,
				sourceOpacity: 1,
				destinationMainAxisScale: 0,
				destinationOpacity: 0,
				startedAt: now,
			});
			changed = true;
		}

		for (const id of this.animations.keys()) {
			if (currentById.has(id)) continue;
			this.animations.delete(id);
		}
		this.capture(snapshot);
		return changed;
	}

	public advance(now: number, durationMs: number): readonly VerticalPinAnimation[] {
		for (const [id, animation] of this.animations) {
			const rawProgress =
				durationMs <= 0 ? 1 : Math.min(1, Math.max(0, (now - animation.startedAt) / durationMs));
			if (rawProgress === 1) {
				this.animations.delete(id);
				continue;
			}
			const progress = tabCollectionEaseInOut(rawProgress);
			animation.progress = progress;
			animation.sourceMainAxisScale = 1 - progress;
			animation.sourceOpacity = 1 - progress;
			animation.destinationMainAxisScale = progress;
			animation.destinationOpacity = progress;
		}
		return this.currentAnimations;
	}

	public complete(): void {
		this.animations.clear();
	}

	private capture(snapshot: Readonly<TabModelSnapshot>): void {
		this.previousTabs = snapshot.tabs.map((tab) => ({ ...tab }));
		this.previousGroups = snapshot.groups.map((group) => ({ ...group }));
	}
}
