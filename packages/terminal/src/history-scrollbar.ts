export const HISTORY_SCROLLBAR_FADE_DELAY_MS = 1_500;

const HISTORY_SCROLLBAR_HIT_WIDTH = 16;
const HISTORY_SCROLLBAR_MINIMUM_THUMB_HEIGHT = 20;
const HISTORY_SCROLLBAR_THUMB_RIGHT_INSET = 2;
const HISTORY_SCROLLBAR_THUMB_WIDTH = 8;
const HISTORY_SCROLLBAR_VERTICAL_TRACK_PADDING = 4;
const RATIO_SCALE = 1_000_000n;

export interface HistoryScrollbarInput {
	readonly opacity: number;
	readonly totalRows: bigint;
	readonly viewportHeight: number;
	readonly viewportWidth: number;
	readonly visibleRows: number;
	readonly visibleStartRow: bigint;
}

export interface HistoryScrollbarProjection {
	readonly hitArea: {
		readonly height: number;
		readonly width: number;
		readonly x: number;
		readonly y: number;
	};
	readonly maximumStartRow: bigint;
	readonly thumb: {
		readonly height: number;
		readonly opacity: number;
		readonly width: number;
		readonly x: number;
		readonly y: number;
	};
	readonly track: {
		readonly height: number;
		readonly travel: number;
		readonly y: number;
	};
	readonly visibleStartRow: bigint;
}

export interface HistoryScrollbarDrag {
	readonly pointerY: number;
	readonly startRatio: number;
}

export interface HistoryScrollbarDragStart {
	readonly drag: HistoryScrollbarDrag;
	readonly targetStartRow: bigint;
}

/** Project one bounded history viewport into shared interaction and renderer geometry. */
export function projectHistoryScrollbar(
	input: HistoryScrollbarInput,
): HistoryScrollbarProjection | null {
	const width = finiteNonnegative(input.viewportWidth);
	const height = finiteNonnegative(input.viewportHeight);
	const finiteVisibleRows = finiteNonnegative(input.visibleRows);
	const visibleRows = finiteVisibleRows === null ? 0 : Math.floor(finiteVisibleRows);
	if (
		width === null ||
		height === null ||
		finiteVisibleRows === null ||
		width === 0 ||
		height <= HISTORY_SCROLLBAR_VERTICAL_TRACK_PADDING * 2 ||
		visibleRows === 0 ||
		input.totalRows <= BigInt(visibleRows)
	) {
		return null;
	}

	const trackHeight = height - HISTORY_SCROLLBAR_VERTICAL_TRACK_PADDING * 2;
	const maximumStartRow = input.totalRows - BigInt(visibleRows);
	const visibleStartRow = clampBigInt(input.visibleStartRow, 0n, maximumStartRow);
	const proportionalHeight =
		(Number((BigInt(visibleRows) * RATIO_SCALE) / input.totalRows) / Number(RATIO_SCALE)) *
		trackHeight;
	const thumbHeight = Math.min(
		trackHeight,
		Math.max(HISTORY_SCROLLBAR_MINIMUM_THUMB_HEIGHT, proportionalHeight),
	);
	const travel = Math.max(0, trackHeight - thumbHeight);
	const startRatio = rowRatio(visibleStartRow, maximumStartRow);
	const opacity = Number.isFinite(input.opacity) ? clampNumber(input.opacity, 0, 1) : 0;
	return {
		hitArea: {
			height,
			width: Math.min(HISTORY_SCROLLBAR_HIT_WIDTH, width),
			x: Math.max(0, width - HISTORY_SCROLLBAR_HIT_WIDTH),
			y: 0,
		},
		maximumStartRow,
		thumb: {
			height: thumbHeight,
			opacity: (visibleStartRow < maximumStartRow ? 0.5 : 0.3) * opacity,
			width: Math.min(HISTORY_SCROLLBAR_THUMB_WIDTH, width),
			x: Math.max(0, width - HISTORY_SCROLLBAR_THUMB_WIDTH - HISTORY_SCROLLBAR_THUMB_RIGHT_INSET),
			y: HISTORY_SCROLLBAR_VERTICAL_TRACK_PADDING + travel * startRatio,
		},
		track: {
			height: trackHeight,
			travel,
			y: HISTORY_SCROLLBAR_VERTICAL_TRACK_PADDING,
		},
		visibleStartRow,
	};
}

/** Test one CSS-pixel point against the mature right-edge interaction gutter. */
export function historyScrollbarContainsPoint(
	projection: HistoryScrollbarProjection,
	x: number,
	y: number,
): boolean {
	return (
		Number.isFinite(x) &&
		Number.isFinite(y) &&
		x >= projection.hitArea.x &&
		x <= projection.hitArea.x + projection.hitArea.width &&
		y >= projection.hitArea.y &&
		y <= projection.hitArea.y + projection.hitArea.height
	);
}

/** Resolve a thumb press or centered track jump and begin proportional dragging. */
export function beginHistoryScrollbarDrag(
	projection: HistoryScrollbarProjection,
	pointerY: number,
): HistoryScrollbarDragStart {
	const boundedPointerY = Number.isFinite(pointerY)
		? clampNumber(pointerY, projection.hitArea.y, projection.hitArea.y + projection.hitArea.height)
		: projection.thumb.y + projection.thumb.height / 2;
	const thumbEnd = projection.thumb.y + projection.thumb.height;
	let startRatio = rowRatio(projection.visibleStartRow, projection.maximumStartRow);
	if (boundedPointerY < projection.thumb.y || boundedPointerY > thumbEnd) {
		startRatio =
			projection.track.travel === 0
				? startRatio
				: clampNumber(
						(boundedPointerY - projection.track.y - projection.thumb.height / 2) /
							projection.track.travel,
						0,
						1,
					);
	}
	return {
		drag: { pointerY: boundedPointerY, startRatio },
		targetStartRow: rowAtRatio(projection.maximumStartRow, startRatio),
	};
}

/** Resolve one drag sample to a clamped absolute visible-start row. */
export function resolveHistoryScrollbarDrag(
	projection: HistoryScrollbarProjection,
	drag: HistoryScrollbarDrag,
	pointerY: number,
): bigint {
	if (!Number.isFinite(pointerY) || projection.track.travel === 0) {
		return rowAtRatio(projection.maximumStartRow, drag.startRatio);
	}
	const ratio = clampNumber(
		drag.startRatio + (pointerY - drag.pointerY) / projection.track.travel,
		0,
		1,
	);
	return rowAtRatio(projection.maximumStartRow, ratio);
}

function rowAtRatio(maximumStartRow: bigint, ratio: number): bigint {
	const scaled = BigInt(Math.round(clampNumber(ratio, 0, 1) * Number(RATIO_SCALE)));
	return (maximumStartRow * scaled + RATIO_SCALE / 2n) / RATIO_SCALE;
}

function rowRatio(row: bigint, maximumStartRow: bigint): number {
	if (maximumStartRow <= 0n) return 0;
	return Number((row * RATIO_SCALE) / maximumStartRow) / Number(RATIO_SCALE);
}

function finiteNonnegative(value: number): number | null {
	return Number.isFinite(value) && value >= 0 ? value : null;
}

function clampNumber(value: number, minimum: number, maximum: number): number {
	return Math.min(maximum, Math.max(minimum, value));
}

function clampBigInt(value: bigint, minimum: bigint, maximum: bigint): bigint {
	if (value < minimum) return minimum;
	return value > maximum ? maximum : value;
}
