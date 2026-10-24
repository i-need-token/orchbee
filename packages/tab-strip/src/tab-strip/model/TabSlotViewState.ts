import type { Rect } from '../utils/rectRight';
import type { TabGroupId } from './tab-types';

type TabPinTransition = 'pinning' | 'unpinning';

/** Browser-independent state carried by Chromium's TabSlotView base class. */
export class TabSlotViewState {
	private groupId: TabGroupId | null = null;
	private detachedValue = false;
	private draggingValue = false;
	private animatingValue = false;
	private pinnedValue: boolean | null = null;
	private pinTransitionValue: TabPinTransition | null = null;

	public get group(): TabGroupId | null {
		return this.groupId;
	}
	public get detached(): boolean {
		return this.detachedValue;
	}
	public get dragging(): boolean {
		return this.draggingValue;
	}
	public get animating(): boolean {
		return this.animatingValue;
	}
	public get pinned(): boolean | null {
		return this.pinnedValue;
	}
	public get pinTransition(): TabPinTransition | null {
		return this.pinTransitionValue;
	}

	public setGroup(group: TabGroupId | null): void {
		this.groupId = group;
	}
	public setDetached(): void {
		this.detachedValue = true;
	}
	public setDragging(dragging: boolean): void {
		this.draggingValue = dragging;
	}
	public setPinned(pinned: boolean | null, animate: boolean): void {
		const previous = this.pinnedValue;
		this.pinnedValue = pinned;
		if (animate && previous !== null && pinned !== null && previous !== pinned) {
			this.pinTransitionValue = pinned ? 'pinning' : 'unpinning';
		} else {
			this.pinTransitionValue = null;
		}
	}
	public setAnimating(animating: boolean): void {
		this.animatingValue = animating;
		if (!animating) this.pinTransitionValue = null;
	}

	/** TabSlotView::GetAnchorBoundsInScreen() insets vertically by 2dp. */
	public getAnchorBoundsInScreen(bounds: Readonly<Rect>): Rect {
		return {
			x: bounds.x,
			y: bounds.y + 2,
			width: bounds.width,
			height: Math.max(0, bounds.height - 4),
		};
	}
}
