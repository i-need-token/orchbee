import type { Rect } from '../geometry';
import type { SplitTabId, TabGroupId } from './tab-types';

/** Browser-independent state carried by Chromium's TabSlotView base class. */
export class TabSlotViewState {
	private groupId: TabGroupId | null = null;
	private splitId: SplitTabId | null = null;
	private detachedValue = false;
	private draggingValue = false;
	private animatingValue = false;

	public get group(): TabGroupId | null {
		return this.groupId;
	}
	public get split(): SplitTabId | null {
		return this.splitId;
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

	public setGroup(group: TabGroupId | null): void {
		this.groupId = group;
	}
	public setSplit(split: SplitTabId | null): void {
		this.splitId = split;
	}
	public setDetached(): void {
		this.detachedValue = true;
	}
	public setDragging(dragging: boolean): void {
		this.draggingValue = dragging;
	}
	public setAnimating(animating: boolean): void {
		this.animatingValue = animating;
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
