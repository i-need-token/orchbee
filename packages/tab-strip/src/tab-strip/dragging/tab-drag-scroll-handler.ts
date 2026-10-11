import type { Rect } from '../utils/rectRight';

const TAB_DRAG_SCROLL_INCREMENT = 5;
export const TAB_DRAG_SCROLL_INTERVAL_MS = 20;

export type TabDragOverflowGradient = 'vertical' | 'vertical-leading' | 'vertical-trailing';

export interface TabDragScrollable {
	scrollTop: number;
	readonly scrollHeight: number;
	readonly clientHeight: number;
	scrollBy(options: { top: number; left: number }): void;
	setDragOverflowGradient?(gradient: TabDragOverflowGradient): void;
}

export interface TabDragScrollSnapshot {
	scrolling: boolean;
	increment: number;
	gradient: TabDragOverflowGradient;
}

/** Direct timer and edge-condition port of common/TabDragScrollHandler. */
export class TabDragScrollHandler {
	private timer: ReturnType<typeof setInterval> | null = null;
	private scrollable: TabDragScrollable | null = null;
	private readonly onContentsScrolled: () => void;
	private snapshot: TabDragScrollSnapshot = Object.freeze({
		scrolling: false,
		increment: 0,
		gradient: 'vertical',
	});

	public constructor(onContentsScrolled: () => void = () => {}) {
		this.onContentsScrolled = onContentsScrolled;
	}

	public getSnapshot(): TabDragScrollSnapshot {
		return this.snapshot;
	}

	public onDraggedTabPositionUpdated(
		scrollable: TabDragScrollable,
		draggedBounds: Readonly<Rect>,
	): void {
		const visibleBottom = scrollable.scrollTop + scrollable.clientHeight;
		const isDraggingToBottom =
			visibleBottom < scrollable.scrollHeight &&
			draggedBounds.y + draggedBounds.height >= scrollable.clientHeight;
		const isDraggingToTop = scrollable.scrollTop > 0 && draggedBounds.y <= 0;
		if (isDraggingToBottom) this.startOrContinue(scrollable, TAB_DRAG_SCROLL_INCREMENT);
		else if (isDraggingToTop) this.startOrContinue(scrollable, -TAB_DRAG_SCROLL_INCREMENT);
		else this.stopScrolling(scrollable);
	}

	public stopScrolling(scrollable = this.scrollable): void {
		if (this.timer !== null) clearInterval(this.timer);
		this.timer = null;
		scrollable?.setDragOverflowGradient?.('vertical');
		this.scrollable = null;
		this.snapshot = Object.freeze({ scrolling: false, increment: 0, gradient: 'vertical' });
	}

	public destroy(): void {
		this.stopScrolling();
	}

	private startOrContinue(scrollable: TabDragScrollable, increment: number): void {
		this.scrollable = scrollable;
		const gradient = increment > 0 ? 'vertical-leading' : 'vertical-trailing';
		scrollable.setDragOverflowGradient?.(gradient);
		this.snapshot = Object.freeze({ scrolling: true, increment, gradient });
		if (this.timer !== null) return;
		this.timer = setInterval(() => {
			if (!this.scrollable) return;
			this.scrollable.scrollBy({ top: this.snapshot.increment, left: 0 });
			this.onContentsScrolled();
		}, TAB_DRAG_SCROLL_INTERVAL_MS);
	}
}

export function createDomTabDragScrollable(element: HTMLElement): TabDragScrollable {
	return {
		get scrollTop() {
			return element.scrollTop;
		},
		set scrollTop(value: number) {
			element.scrollTop = value;
		},
		get scrollHeight() {
			return element.scrollHeight;
		},
		get clientHeight() {
			return element.clientHeight;
		},
		scrollBy: ({ top, left }) => element.scrollBy({ top, left }),
		setDragOverflowGradient: (gradient) => {
			if (gradient === 'vertical') delete element.dataset['dragScrollGradient'];
			else element.dataset['dragScrollGradient'] = gradient;
		},
	};
}
