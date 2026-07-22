import {
	TabCollectionAnimatingLayoutManager,
	TabCollectionAnimationAxis,
	type AnimatedLayoutFrame,
	type ProposedLayout,
} from '../../../layout';
import {
	TabSlotAnimationDelegate,
	TabSlotViewState,
	type TabGroupId,
	type TabSlotAnimationHost,
} from '../../../model';

export interface LiveTabSlot {
	id: string;
	group: TabGroupId | null;
	dragging: boolean;
}

export interface TabCollectionAnimationSyncResult {
	changed: boolean;
	startProgress: number;
	frame: AnimatedLayoutFrame;
}

function sameBounds(
	left: Readonly<{ x: number; y: number; width: number; height: number }> | undefined,
	right: Readonly<{ x: number; y: number; width: number; height: number }> | undefined,
): boolean {
	return (
		left !== undefined &&
		right !== undefined &&
		left.x === right.x &&
		left.y === right.y &&
		left.width === right.width &&
		left.height === right.height
	);
}

/**
 * DOM adapter for Chromium's TabCollectionAnimatingLayoutManager.
 *
 * The manager remains the bounds/retargeting owner. React only schedules its
 * 0..1 timeline and paints the returned frame. TabSlotViewState and
 * TabSlotAnimationDelegate retain the per-view state and lifecycle that Views
 * normally supplies in Chromium.
 */
export class TabCollectionAnimationRuntime {
	private readonly slots = new Map<string, TabSlotViewState>();
	private readonly delegates = new Map<string, TabSlotAnimationDelegate>();
	private readonly host: TabSlotAnimationHost;
	private readonly manager: TabCollectionAnimatingLayoutManager;
	private initialized = false;

	public constructor(host: Partial<TabSlotAnimationHost> = {}) {
		this.host = {
			onTabSlotAnimationProgressed: host.onTabSlotAnimationProgressed ?? (() => {}),
			...(host.layoutTabSlotImmediately === undefined
				? {}
				: { layoutTabSlotImmediately: host.layoutTabSlotImmediately }),
		};
		this.manager = new TabCollectionAnimatingLayoutManager({
			animationAxis: TabCollectionAnimationAxis.Horizontal,
			delegate: {
				isDragging: () => [...this.slots.values()].some((slot) => slot.dragging),
				isViewDragging: (id) => this.slots.get(id)?.dragging ?? false,
				shouldAnimateOpacityForAddAndRemove: () => true,
			},
		});
	}

	public get currentFrame(): AnimatedLayoutFrame {
		return this.manager.currentFrame;
	}

	public get isAnimating(): boolean {
		return this.manager.isAnimating;
	}

	public getSlot(id: string): TabSlotViewState | undefined {
		return this.slots.get(id);
	}

	public sync(
		target: Readonly<ProposedLayout>,
		liveSlots: readonly Readonly<LiveTabSlot>[],
	): TabCollectionAnimationSyncResult {
		const nextIds = new Set(liveSlots.map((slot) => slot.id));
		for (const [id, slot] of this.slots) {
			if (nextIds.has(id)) continue;
			slot.setDetached();
			const delegate = this.delegates.get(id);
			if (delegate) {
				delegate.onViewDeleting(slot);
				delegate.stopObserving();
				this.delegates.delete(id);
			}
			this.manager.onViewRemoved(id);
			this.slots.delete(id);
		}

		for (const live of liveSlots) {
			let slot = this.slots.get(live.id);
			if (!slot) {
				slot = new TabSlotViewState();
				this.slots.set(live.id, slot);
				this.manager.onViewAdded(live.id);
			}
			slot.setGroup(live.group);
			slot.setDragging(live.dragging);
		}
		this.manager.setChildren(liveSlots.map((slot) => slot.id));

		const beforeById = new Map(
			this.manager.currentFrame.layout.children.map((child) => [child.id, child.bounds]),
		);
		const changed = this.manager.recalculateTarget(target);
		if (!this.initialized) {
			// A ResizeObserver-backed React host commonly produces one zero-width
			// layout before its first measured frame. Do not animate that bootstrap.
			this.initialized = target.hostSize.width > 0;
			this.manager.complete();
			return { changed: false, startProgress: 1, frame: this.manager.currentFrame };
		}

		if (changed && this.manager.isAnimating) {
			const targetById = new Map(target.children.map((child) => [child.id, child.bounds]));
			for (const [id, slot] of this.slots) {
				if (sameBounds(beforeById.get(id), targetById.get(id))) continue;
				if (!this.delegates.has(id)) {
					this.delegates.set(id, new TabSlotAnimationDelegate(this.host, slot));
				}
			}
		}
		return {
			changed,
			startProgress: this.manager.currentProgress,
			frame: this.manager.currentFrame,
		};
	}

	public setProgress(value: number): AnimatedLayoutFrame {
		const frame = this.manager.setProgress(value);
		this.delegates.forEach((delegate) => delegate.animationProgressed());
		return frame;
	}

	public complete(): AnimatedLayoutFrame {
		this.manager.complete();
		this.delegates.forEach((delegate) => {
			delegate.animationEnded();
			delegate.stopObserving();
		});
		this.delegates.clear();
		return this.manager.currentFrame;
	}

	public dispose(): void {
		for (const [id, slot] of this.slots) {
			const delegate = this.delegates.get(id);
			if (delegate) delegate.onViewDeleting(slot);
			delegate?.stopObserving();
			slot.setDetached();
		}
		this.delegates.clear();
		this.slots.clear();
	}
}
