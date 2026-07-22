import type { Rect } from '../geometry';

export const DROP_ARROW_SIZE = 20;

export const DropArrowDirection = {
	Down: 'down',
	Up: 'up',
	Left: 'left',
	Right: 'right',
} as const;

export type DropArrowDirection = (typeof DropArrowDirection)[keyof typeof DropArrowDirection];

export const DropRelativeToIndex = {
	InsertBeforeIndex: 'insert-before-index',
	ReplaceIndex: 'replace-index',
} as const;

export type DropRelativeToIndex = (typeof DropRelativeToIndex)[keyof typeof DropRelativeToIndex];

export const DropGroupInclusion = {
	IncludeInGroup: 'include-in-group',
	DontIncludeInGroup: 'dont-include-in-group',
} as const;

export type DropGroupInclusion = (typeof DropGroupInclusion)[keyof typeof DropGroupInclusion];

export interface TabDropIndex {
	index: number;
	relativeToIndex: DropRelativeToIndex;
	groupInclusion: DropGroupInclusion;
}

export interface DropArrowPlacement {
	bounds: Rect;
	direction: DropArrowDirection;
}

export type DropArrowBoundsCallback = (index: Readonly<TabDropIndex>) => DropArrowPlacement;

/** Headless state port of DropArrow::SetIndex/UpdateBounds. */
export class DropArrowState {
	private currentIndex: TabDropIndex;
	private placementValue: DropArrowPlacement;
	private readonly boundsCallback: DropArrowBoundsCallback;

	public constructor(index: TabDropIndex, boundsCallback: DropArrowBoundsCallback) {
		this.boundsCallback = boundsCallback;
		this.currentIndex = { ...index };
		this.placementValue = this.boundsCallback(this.currentIndex);
	}

	public get index(): TabDropIndex {
		return { ...this.currentIndex };
	}

	public get placement(): DropArrowPlacement {
		return { bounds: { ...this.placementValue.bounds }, direction: this.placementValue.direction };
	}

	public setIndex(index: TabDropIndex): DropArrowPlacement {
		this.currentIndex = { ...index };
		this.placementValue = this.boundsCallback(this.currentIndex);
		return this.placement;
	}
}
