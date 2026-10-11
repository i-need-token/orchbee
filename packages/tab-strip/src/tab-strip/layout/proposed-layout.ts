import type { Rect } from '../utils/rectRight';

export interface Size {
	width: number;
	height: number;
}

export interface SizeBounds {
	width: number | null;
	height: number | null;
}

export interface ChildLayout {
	id: string;
	visible: boolean;
	bounds: Rect;
}

export interface ProposedLayout {
	hostSize: Size;
	children: ChildLayout[];
}
