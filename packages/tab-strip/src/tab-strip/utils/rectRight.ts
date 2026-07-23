/** A small gfx::Rect equivalent used by the headless layout port. */
export interface Rect {
	x: number;
	y: number;
	width: number;
	height: number;
}

export function rectRight(rect: Rect): number {
	return rect.x + rect.width;
}
