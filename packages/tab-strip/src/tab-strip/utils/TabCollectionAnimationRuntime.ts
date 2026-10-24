import {
	tabCollectionEaseOut,
	type AnimatedLayoutFrame,
} from '../layout/tab-collection-animating-layout-manager';
import { type ChildLayout, type ProposedLayout } from '../layout/proposed-layout';
import {
	TabSlotAnimationDelegate,
	type TabSlotAnimationHost,
} from '../model/TabSlotAnimationDelegate';
import { TabSlotViewState } from '../model/TabSlotViewState';
import { type TabGroupId } from '../model/tab-types';

export interface LiveTabSlot {
	id: string;
	group: TabGroupId | null;
	dragging: boolean;
	pinned: boolean | null;
}

interface TabCollectionAnimationSyncResult {
	changed: boolean;
	startProgress: number;
	frame: AnimatedLayoutFrame;
}

interface SlotBoundsAnimation {
	current: ChildLayout;
	cycleStartRawProgress: number;
	rawProgress: number;
	start: ChildLayout;
	target: ChildLayout;
}

const EMPTY_LAYOUT: ProposedLayout = {
	hostSize: { width: 0, height: 0 },
	children: [],
};

function cloneChild(child: Readonly<ChildLayout>): ChildLayout {
	return { id: child.id, visible: child.visible, bounds: { ...child.bounds } };
}

function cloneLayout(layout: Readonly<ProposedLayout>): ProposedLayout {
	return {
		hostSize: { ...layout.hostSize },
		children: layout.children.map(cloneChild),
	};
}

function cloneFrame(frame: Readonly<AnimatedLayoutFrame>): AnimatedLayoutFrame {
	return {
		layout: cloneLayout(frame.layout),
		contentHeight: frame.contentHeight,
	};
}

function sameChild(
	left: Readonly<ChildLayout> | undefined,
	right: Readonly<ChildLayout> | undefined,
): boolean {
	return (
		left !== undefined &&
		right !== undefined &&
		left.id === right.id &&
		left.visible === right.visible &&
		left.bounds.x === right.bounds.x &&
		left.bounds.y === right.bounds.y &&
		left.bounds.width === right.bounds.width &&
		left.bounds.height === right.bounds.height
	);
}

function sameLayout(left: Readonly<ProposedLayout>, right: Readonly<ProposedLayout>): boolean {
	return (
		left.hostSize.width === right.hostSize.width &&
		left.hostSize.height === right.hostSize.height &&
		left.children.length === right.children.length &&
		left.children.every((child, index) => sameChild(child, right.children[index]))
	);
}

function tweenInt(progress: number, start: number, end: number): number {
	return Math.round(start + (end - start) * progress);
}

function tweenChild(
	progress: number,
	start: Readonly<ChildLayout>,
	target: Readonly<ChildLayout>,
): ChildLayout {
	return {
		id: target.id,
		visible: target.visible,
		bounds: {
			x: tweenInt(progress, start.bounds.x, target.bounds.x),
			y: tweenInt(progress, start.bounds.y, target.bounds.y),
			width: tweenInt(progress, start.bounds.width, target.bounds.width),
			height: tweenInt(progress, start.bounds.height, target.bounds.height),
		},
	};
}

function inverseEaseOut(progress: number): number {
	const clamped = Math.min(1, Math.max(0, progress));
	return 1 - Math.sqrt(1 - clamped);
}

/**
 * DOM adapter for Chromium's per-slot BoundsAnimator behavior.
 *
 * React schedules one shared frame loop, but every slot retains its own start,
 * target, and elapsed time. Retargeting one slot therefore does not restart
 * another slot that is already animating to the same bounds.
 */
export class TabCollectionAnimationRuntime {
	private readonly slots = new Map<string, TabSlotViewState>();
	private readonly delegates = new Map<string, TabSlotAnimationDelegate>();
	private readonly animations = new Map<string, SlotBoundsAnimation>();
	private readonly host: TabSlotAnimationHost;
	private target = cloneLayout(EMPTY_LAYOUT);
	private frame: AnimatedLayoutFrame = {
		layout: cloneLayout(EMPTY_LAYOUT),
		contentHeight: 0,
	};
	private initialized = false;

	public constructor(host: Partial<TabSlotAnimationHost> = {}) {
		this.host = {
			onTabSlotAnimationProgressed: host.onTabSlotAnimationProgressed ?? (() => {}),
			...(host.layoutTabSlotImmediately === undefined
				? {}
				: { layoutTabSlotImmediately: host.layoutTabSlotImmediately }),
		};
	}

	public get currentFrame(): AnimatedLayoutFrame {
		return cloneFrame(this.frame);
	}

	public get isAnimating(): boolean {
		return this.animations.size > 0;
	}

	public getSlot(id: string): TabSlotViewState | undefined {
		return this.slots.get(id);
	}

	public sync(
		target: Readonly<ProposedLayout>,
		liveSlots: readonly Readonly<LiveTabSlot>[],
	): TabCollectionAnimationSyncResult {
		const pinChanges = new Map<string, boolean | null>();
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
			this.animations.delete(id);
			this.slots.delete(id);
		}

		for (const live of liveSlots) {
			let slot = this.slots.get(live.id);
			if (!slot) {
				slot = new TabSlotViewState();
				slot.setPinned(live.pinned, false);
				this.slots.set(live.id, slot);
			} else if (slot.pinned !== live.pinned) {
				pinChanges.set(live.id, live.pinned);
			}
			slot.setGroup(live.group);
			slot.setDragging(live.dragging);
		}

		const changed = !sameLayout(target, this.target);
		if (!this.initialized) {
			// A ResizeObserver-backed React host commonly produces one zero-width
			// layout before its first measured frame. Do not animate that bootstrap.
			this.initialized = target.hostSize.width > 0;
			this.snapToTarget(target);
			this.applyPinChanges(pinChanges, false);
			return { changed: false, startProgress: 1, frame: this.currentFrame };
		}

		if (this.target.hostSize.width !== 0 && this.target.hostSize.width !== target.hostSize.width) {
			this.cancelAllAnimations();
			this.snapToTarget(target);
			this.applyPinChanges(pinChanges, false);
			return { changed, startProgress: 1, frame: this.currentFrame };
		}

		if (!changed) {
			const targetById = new Map(target.children.map((child) => [child.id, child]));
			for (const live of liveSlots) {
				if (!live.dragging) continue;
				const targetChild = targetById.get(live.id);
				if (!targetChild) continue;
				this.cancelAnimation(live.id);
				this.animations.delete(live.id);
			}
			this.applyPinChanges(pinChanges, true);
			this.rebuildFrame();
			return { changed: false, startProgress: 0, frame: this.currentFrame };
		}

		const previousTargetById = new Map(this.target.children.map((child) => [child.id, child]));
		const currentById = new Map(this.frame.layout.children.map((child) => [child.id, child]));
		const liveSlotById = new Map(liveSlots.map((slot) => [slot.id, slot]));
		this.target = cloneLayout(target);
		const targetIds = new Set(target.children.map((child) => child.id));
		for (const id of this.animations.keys()) {
			if (targetIds.has(id)) continue;
			this.animations.delete(id);
		}
		for (const targetChild of target.children) {
			const slot = this.slots.get(targetChild.id);
			if (!slot) continue;
			const current =
				currentById.get(targetChild.id) ??
				this.getAddedGroupHeaderStart(targetChild, liveSlots, liveSlotById, currentById);
			const existing = this.animations.get(targetChild.id);
			if (slot.dragging || !current) {
				this.cancelAnimation(targetChild.id);
				this.animations.delete(targetChild.id);
				continue;
			}
			if (existing && sameChild(previousTargetById.get(targetChild.id), targetChild)) {
				existing.cycleStartRawProgress = existing.rawProgress;
				continue;
			}
			if (sameChild(current, targetChild)) {
				this.cancelAnimation(targetChild.id);
				this.animations.delete(targetChild.id);
				continue;
			}
			this.cancelAnimation(targetChild.id);
			this.delegates.set(targetChild.id, new TabSlotAnimationDelegate(this.host, slot));
			this.animations.set(targetChild.id, {
				current: cloneChild(current),
				cycleStartRawProgress: 0,
				rawProgress: 0,
				start: cloneChild(current),
				target: cloneChild(targetChild),
			});
		}
		this.applyPinChanges(pinChanges, true);
		this.rebuildFrame();
		return {
			changed,
			startProgress: 0,
			frame: this.currentFrame,
		};
	}

	public setProgress(value: number): AnimatedLayoutFrame {
		const cycleRawProgress = inverseEaseOut(value);
		const completed: string[] = [];
		for (const [id, animation] of this.animations) {
			animation.rawProgress = Math.min(1, animation.cycleStartRawProgress + cycleRawProgress);
			animation.current = tweenChild(
				tabCollectionEaseOut(animation.rawProgress),
				animation.start,
				animation.target,
			);
			this.delegates.get(id)?.animationProgressed();
			if (animation.rawProgress === 1) completed.push(id);
		}
		for (const id of completed) {
			this.animations.delete(id);
			this.finishAnimation(id);
		}
		this.rebuildFrame();
		return this.currentFrame;
	}

	public complete(): AnimatedLayoutFrame {
		const ids = [...this.animations.keys()];
		this.animations.clear();
		for (const id of ids) this.finishAnimation(id);
		this.frame = {
			layout: cloneLayout(this.target),
			contentHeight: this.target.hostSize.height,
		};
		return this.currentFrame;
	}

	public dispose(): void {
		for (const [id, slot] of this.slots) {
			const delegate = this.delegates.get(id);
			if (delegate) delegate.onViewDeleting(slot);
			delegate?.stopObserving();
			slot.setDetached();
		}
		this.animations.clear();
		this.delegates.clear();
		this.slots.clear();
	}

	private cancelAnimation(id: string): void {
		const delegate = this.delegates.get(id);
		if (!delegate) return;
		delegate.animationCanceled();
		delegate.stopObserving();
		this.delegates.delete(id);
	}

	private cancelAllAnimations(): void {
		const ids = [...this.delegates.keys()];
		this.animations.clear();
		for (const id of ids) this.cancelAnimation(id);
	}

	private finishAnimation(id: string): void {
		const delegate = this.delegates.get(id);
		if (!delegate) return;
		delegate.animationEnded();
		delegate.stopObserving();
		this.delegates.delete(id);
	}

	private applyPinChanges(
		changes: ReadonlyMap<string, boolean | null>,
		animateWhenBoundsChange: boolean,
	): void {
		for (const [id, pinned] of changes) {
			this.slots.get(id)?.setPinned(pinned, animateWhenBoundsChange && this.animations.has(id));
		}
	}

	private getAddedGroupHeaderStart(
		targetChild: Readonly<ChildLayout>,
		liveSlots: readonly Readonly<LiveTabSlot>[],
		liveSlotById: ReadonlyMap<string, Readonly<LiveTabSlot>>,
		currentById: ReadonlyMap<string, Readonly<ChildLayout>>,
	): ChildLayout | undefined {
		const addedSlot = liveSlotById.get(targetChild.id);
		if (!addedSlot || addedSlot.pinned !== null || addedSlot.group === null) return undefined;
		const firstMember = liveSlots.find(
			(candidate) => candidate.pinned !== null && candidate.group === addedSlot.group,
		);
		if (!firstMember) return undefined;
		const memberBounds = currentById.get(firstMember.id);
		if (!memberBounds) return undefined;
		return {
			id: targetChild.id,
			visible: true,
			bounds: { ...memberBounds.bounds },
		};
	}

	private rebuildFrame(): void {
		const animationById = this.animations;
		const children = this.target.children.map((child) =>
			cloneChild(animationById.get(child.id)?.current ?? child),
		);
		const contentHeight = children.reduce(
			(height, child) => Math.max(height, child.bounds.y + child.bounds.height),
			this.target.hostSize.height,
		);
		this.frame = {
			layout: {
				hostSize: { ...this.target.hostSize },
				children,
			},
			contentHeight,
		};
	}

	private snapToTarget(target: Readonly<ProposedLayout>): void {
		this.target = cloneLayout(target);
		this.frame = {
			layout: cloneLayout(target),
			contentHeight: target.hostSize.height,
		};
	}
}
