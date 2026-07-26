import type { Rect } from '../utils/rectRight';
import type { ChildLayout, ProposedLayout, Size } from './proposed-layout';

export const TabCollectionAnimationAxis = {
	Vertical: 'vertical',
	Horizontal: 'horizontal',
} as const;

export type TabCollectionAnimationAxis =
	(typeof TabCollectionAnimationAxis)[keyof typeof TabCollectionAnimationAxis];

export const TabCollectionAnimationDirection = {
	StartToEnd: 'start-to-end',
	EndToStart: 'end-to-start',
} as const;

export type TabCollectionAnimationDirection =
	(typeof TabCollectionAnimationDirection)[keyof typeof TabCollectionAnimationDirection];

export interface SourceLayoutInfo {
	animationAxis?: TabCollectionAnimationAxis;
	animationDirection?: TabCollectionAnimationDirection;
}

export interface TabCollectionAnimatingLayoutDelegate {
	isDragging?(): boolean;
	isViewDragging?(id: string): boolean;
	shouldSnapToTarget?(id: string): boolean;
	onAnimationEnded?(): void;
}

export interface AnimatedLayoutFrame {
	layout: ProposedLayout;
	contentHeight: number;
}

interface ViewAnimationMetadata {
	previousCollectionBounds?: Rect;
	sourceLayoutInfo?: SourceLayoutInfo;
}

const EMPTY_LAYOUT: ProposedLayout = {
	hostSize: { width: 0, height: 0 },
	children: [],
};

function cloneRect(rect: Readonly<Rect>): Rect {
	return { ...rect };
}

function cloneChild(child: Readonly<ChildLayout>): ChildLayout {
	return { id: child.id, visible: child.visible, bounds: cloneRect(child.bounds) };
}

function cloneLayout(layout: Readonly<ProposedLayout>): ProposedLayout {
	return {
		hostSize: { ...layout.hostSize },
		children: layout.children.map(cloneChild),
	};
}

function layoutMap(layout: Readonly<ProposedLayout>): Map<string, ChildLayout> {
	return new Map(layout.children.map((child) => [child.id, cloneChild(child)]));
}

function layoutsEqual(left: Readonly<ProposedLayout>, right: Readonly<ProposedLayout>): boolean {
	if (
		left.hostSize.width !== right.hostSize.width ||
		left.hostSize.height !== right.hostSize.height
	)
		return false;
	if (left.children.length !== right.children.length) return false;
	return left.children.every((child, index) => {
		const candidate = right.children[index];
		return (
			candidate !== undefined &&
			child.id === candidate.id &&
			child.visible === candidate.visible &&
			child.bounds.x === candidate.bounds.x &&
			child.bounds.y === candidate.bounds.y &&
			child.bounds.width === candidate.bounds.width &&
			child.bounds.height === candidate.bounds.height
		);
	});
}

function tweenInt(value: number, start: number, end: number): number {
	return Math.round(start + (end - start) * value);
}

function tweenRect(value: number, start: Readonly<Rect>, end: Readonly<Rect>): Rect {
	return {
		x: tweenInt(value, start.x, end.x),
		y: tweenInt(value, start.y, end.y),
		width: tweenInt(value, start.width, end.width),
		height: tweenInt(value, start.height, end.height),
	};
}

function isEmptyRect(rect: Readonly<Rect>): boolean {
	return rect.width <= 0 || rect.height <= 0;
}

function bottom(rect: Readonly<Rect>): number {
	return rect.y + rect.height;
}

function clamp01(value: number): number {
	return Math.min(1, Math.max(0, value));
}

/**
 * Headless port of common/TabCollectionAnimatingLayoutManager.
 *
 * DOM/React callers provide child IDs and target ProposedLayouts in place of
 * Views pointers. The class retains Chromium's retargeting, reparenting,
 * drag-snap, and animated-host-height rules. Tab creation and closure snap
 * directly to their terminal layouts and are not animation responsibilities.
 */
export class TabCollectionAnimatingLayoutManager {
	private readonly delegate: TabCollectionAnimatingLayoutDelegate;
	private readonly animationAxis: TabCollectionAnimationAxis;
	private readonly animateHostSize: boolean;
	private childIds: string[] = [];
	private metadata = new Map<string, ViewAnimationMetadata>();
	private startingLayout = cloneLayout(EMPTY_LAYOUT);
	private target = cloneLayout(EMPTY_LAYOUT);
	private currentFrameValue: AnimatedLayoutFrame = {
		layout: cloneLayout(EMPTY_LAYOUT),
		contentHeight: 0,
	};
	private startMap = new Map<string, ChildLayout>();
	private targetMap = new Map<string, ChildLayout>();
	private startingOffset = 0;
	private currentOffset = 1;
	private animating = false;

	public constructor(
		options: {
			delegate?: TabCollectionAnimatingLayoutDelegate;
			animationAxis?: TabCollectionAnimationAxis;
			animateHostSize?: boolean;
		} = {},
	) {
		this.delegate = options.delegate ?? {};
		this.animationAxis = options.animationAxis ?? TabCollectionAnimationAxis.Vertical;
		this.animateHostSize = options.animateHostSize ?? false;
	}

	public get isAnimating(): boolean {
		return this.animating;
	}

	/** Current absolute SlideAnimation value, used when a live host retargets. */
	public get currentProgress(): number {
		return this.currentOffset;
	}

	public get targetLayout(): ProposedLayout {
		return cloneLayout(this.target);
	}

	public get currentFrame(): AnimatedLayoutFrame {
		return {
			layout: cloneLayout(this.currentFrameValue.layout),
			contentHeight: this.currentFrameValue.contentHeight,
		};
	}

	public setChildren(ids: readonly string[]): void {
		if (new Set(ids).size !== ids.length)
			throw new Error('Animating layout child IDs must be unique.');
		this.childIds = [...ids];
	}

	public onViewAdded(id: string): void {
		if (!this.childIds.includes(id)) this.childIds.push(id);
	}

	public onViewRemoved(id: string): void {
		this.childIds = this.childIds.filter((candidate) => candidate !== id);
		this.metadata.delete(id);
	}

	public setSourceLayoutInfo(id: string, sourceLayoutInfo: SourceLayoutInfo): void {
		this.getMetadata(id).sourceLayoutInfo = { ...sourceLayoutInfo };
	}

	/** Previous bounds are local to the new host (screen conversion is a DOM concern). */
	public animateAndReparentView(id: string, previousBounds: Readonly<Rect>): void {
		this.onViewAdded(id);
		if (!this.delegate.isViewDragging?.(id) && !isEmptyRect(previousBounds)) {
			this.getMetadata(id).previousCollectionBounds = cloneRect(previousBounds);
		}
	}

	/**
	 * Recomputes the target and starts or retargets the 0..1 animation timeline.
	 * Returns false when the layout is unchanged.
	 */
	public recalculateTarget(newTarget: Readonly<ProposedLayout>): boolean {
		if (layoutsEqual(newTarget, this.target)) return false;

		// A Views target layout follows current child order. Mirror that ordering
		// while retaining pending-delete/reparented children absent from target.
		const targetIds = newTarget.children.map((child) => child.id);
		const targetIdSet = new Set(targetIds);
		this.childIds = [...targetIds, ...this.childIds.filter((id) => !targetIdSet.has(id))];

		if (
			this.currentFrameValue.layout.hostSize.width !== 0 &&
			this.currentFrameValue.layout.hostSize.width !== newTarget.hostSize.width
		) {
			this.currentFrameValue = {
				layout: cloneLayout(newTarget),
				contentHeight: newTarget.hostSize.height,
			};
			this.setStartingLayout(newTarget);
			this.setTargetLayout(newTarget);
			this.startingOffset = 0;
			this.currentOffset = 1;
			this.animating = false;
			return true;
		}

		this.setTargetLayout(newTarget);

		if (this.animating && this.currentOffset === this.startingOffset) {
			this.currentFrameValue = this.interpolateLayout(0);
			return true;
		}

		if (this.currentOffset > 0.8) {
			this.setStartingLayout(this.currentFrameValue.layout);
			this.startingOffset = 0;
			this.currentOffset = 0;
		} else {
			this.setStartingLayout(this.currentFrameValue.layout);
			this.startingOffset = this.currentOffset;
		}
		this.animating = true;
		this.currentFrameValue = this.interpolateLayout(0);
		return true;
	}

	/** Applies an absolute animation value, matching SlideAnimation::GetCurrentValue(). */
	public setProgress(value: number): AnimatedLayoutFrame {
		if (!this.animating) return this.currentFrame;
		this.currentOffset = clamp01(value);
		const denominator = 1 - this.startingOffset;
		const percent =
			denominator === 0 ? 1 : clamp01((this.currentOffset - this.startingOffset) / denominator);
		this.currentFrameValue = this.interpolateLayout(percent);
		return this.currentFrame;
	}

	public resetViewsToTargetLayout(ids: readonly string[]): AnimatedLayoutFrame {
		const idsToSnap = new Set(ids);
		this.startingLayout.children = this.startingLayout.children.map((child) => {
			if (!idsToSnap.has(child.id)) return child;
			const target = this.targetMap.get(child.id);
			return target ? { ...child, bounds: cloneRect(target.bounds) } : child;
		});
		this.setStartingLayout(this.startingLayout);
		this.setTargetLayout(this.target);
		this.currentFrameValue = this.interpolateLayout(0);
		return this.currentFrame;
	}

	public getPreferredSize(targetPreferredSize: Readonly<Size>): Size {
		if (this.animateHostSize && this.animating && !this.delegate.isDragging?.()) {
			return { width: targetPreferredSize.width, height: this.currentFrameValue.contentHeight };
		}
		return { ...targetPreferredSize };
	}

	public complete(): void {
		if (this.animating) this.setProgress(1);
		this.animating = false;
		this.currentOffset = 1;
		this.startingOffset = 0;

		this.currentFrameValue = {
			layout: cloneLayout(this.target),
			contentHeight: this.target.hostSize.height,
		};
		this.setStartingLayout(this.target);
		this.metadata.clear();
		this.delegate.onAnimationEnded?.();
	}

	private setStartingLayout(layout: Readonly<ProposedLayout>): void {
		const childSet = new Set(this.childIds);
		this.startingLayout = cloneLayout(layout);
		this.startMap = new Map(
			this.startingLayout.children
				.filter((child) => childSet.has(child.id))
				.map((child) => [child.id, cloneChild(child)]),
		);
	}

	private setTargetLayout(layout: Readonly<ProposedLayout>): void {
		this.target = cloneLayout(layout);
		this.targetMap = layoutMap(this.target);
	}

	private interpolateLayout(value: number): AnimatedLayoutFrame {
		const result: ProposedLayout = {
			hostSize: { ...this.target.hostSize },
			children: [],
		};
		let contentHeight = tweenInt(
			value,
			this.startingLayout.hostSize.height,
			this.target.hostSize.height,
		);

		for (const id of this.childIds) {
			const target = this.targetMap.get(id);
			const start = this.startMap.get(id);
			const metadata = this.metadata.get(id);

			if (target) {
				const child = cloneChild(target);
				if (!this.delegate.isViewDragging?.(id) && start) {
					child.bounds = tweenRect(value, start.bounds, target.bounds);
					child.visible = target.visible;
				} else if (
					!this.delegate.isViewDragging?.(id) &&
					!this.delegate.shouldSnapToTarget?.(id) &&
					metadata?.previousCollectionBounds
				) {
					child.bounds = tweenRect(value, metadata.previousCollectionBounds, target.bounds);
				}
				if (!isEmptyRect(child.bounds))
					contentHeight = Math.max(contentHeight, bottom(child.bounds));
				result.children.push(child);
				continue;
			}

			if (metadata?.previousCollectionBounds && !start) {
				const startBounds = cloneRect(metadata.previousCollectionBounds);
				const targetBounds = cloneRect(startBounds);
				const source = metadata.sourceLayoutInfo;
				const axis = source?.animationAxis ?? this.animationAxis;
				const direction = source?.animationDirection ?? TabCollectionAnimationDirection.EndToStart;
				if (axis === TabCollectionAnimationAxis.Vertical) {
					if (direction === TabCollectionAnimationDirection.StartToEnd) {
						targetBounds.y = bottom(startBounds);
					}
					targetBounds.height = 0;
				} else {
					if (direction === TabCollectionAnimationDirection.StartToEnd) {
						targetBounds.x = startBounds.x + startBounds.width;
					}
					targetBounds.width = 0;
				}
				const child: ChildLayout = {
					id,
					visible: true,
					bounds: tweenRect(value, startBounds, targetBounds),
				};
				if (!isEmptyRect(child.bounds))
					contentHeight = Math.max(contentHeight, bottom(child.bounds));
				result.children.push(child);
			}
		}

		return { layout: result, contentHeight };
	}

	private getMetadata(id: string): ViewAnimationMetadata {
		let metadata = this.metadata.get(id);
		if (!metadata) {
			metadata = {};
			this.metadata.set(id, metadata);
		}
		return metadata;
	}
}

/** DOM equivalent of the EASE_OUT tween used by Chromium's BoundsAnimator. */
export function tabCollectionEaseOut(value: number): number {
	const clamped = clamp01(value);
	return 1 - (1 - clamped) ** 2;
}

/** DOM equivalent of the EASE_IN_OUT tween used by collection layout animations. */
export function tabCollectionEaseInOut(value: number): number {
	const clamped = clamp01(value);
	return clamped < 0.5 ? 2 * clamped * clamped : 1 - (-2 * clamped + 2) ** 2 / 2;
}
