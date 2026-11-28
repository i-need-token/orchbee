import type { FileOverlayBox, FileOverlayRegions } from './fileOverlayCover';

/** Picture-in-picture's share of the terminal area's width and of its height. */
const PIP_SHARE = 0.4;
/** The smallest picture-in-picture, unless the window below the 36px row is smaller still. */
const PIP_MINIMUM_WIDTH = 200;
const PIP_MINIMUM_HEIGHT = 120;
/** How far inside the terminal area's bottom-right corner picture-in-picture first appears. */
const PIP_INSET = 24;

/** The corners of the window below the 36px row, one of which picture-in-picture keeps to. */
export const FILE_OVERLAY_PIP_CORNERS = [
	'bottom-left',
	'bottom-right',
	'top-left',
	'top-right',
] as const;

/**
 * Where a drag left picture-in-picture: the corner of the window below the 36px row nearest to it,
 * and how far its edges lie from that corner's two edges, in whole pixels. It keeps to that corner
 * as the window changes.
 */
export interface FileOverlayPipPosition {
	readonly corner: (typeof FILE_OVERLAY_PIP_CORNERS)[number];
	/** From the corner's side of the window to picture-in-picture's nearer side. */
	readonly x: number;
	/** From the 36px row's bottom, or the window's bottom, to picture-in-picture's nearer edge. */
	readonly y: number;
}

function nearestCorner(fromLeft: boolean, fromTop: boolean): FileOverlayPipPosition['corner'] {
	if (fromTop) return fromLeft ? 'top-left' : 'top-right';
	return fromLeft ? 'bottom-left' : 'bottom-right';
}

/** The window below the 36px row: where picture-in-picture may go. */
function pipArea({ terminal, width }: FileOverlayRegions): FileOverlayBox {
	return { height: terminal.height, left: 0, top: terminal.top, width };
}

/** Moves `box`, no larger than `area`, the least distance that puts it inside `area`. */
function within(area: FileOverlayBox, box: FileOverlayBox): FileOverlayBox {
	return {
		...box,
		left: Math.max(area.left, Math.min(box.left, area.left + area.width - box.width)),
		top: Math.max(area.top, Math.min(box.top, area.top + area.height - box.height)),
	};
}

/**
 * Picture-in-picture's box: 40% of the terminal area's width and of its height, at least 200 × 120
 * px, where its position puts it, or before a drag has moved it at the terminal area's bottom-right
 * corner, 24 px inside. It stays inside the window below the 36px row, and only a window smaller
 * than its size makes it smaller.
 */
export function fileOverlayPipBox(
	regions: FileOverlayRegions,
	position: FileOverlayPipPosition | null,
): FileOverlayBox {
	const area = pipArea(regions);
	const { terminal } = regions;
	const width = Math.min(area.width, Math.max(PIP_MINIMUM_WIDTH, terminal.width * PIP_SHARE));
	const height = Math.min(area.height, Math.max(PIP_MINIMUM_HEIGHT, terminal.height * PIP_SHARE));
	if (position === null) {
		return within(area, {
			height,
			left: terminal.left + terminal.width - PIP_INSET - width,
			top: terminal.top + terminal.height - PIP_INSET - height,
			width,
		});
	}
	const fromLeft = position.corner === 'bottom-left' || position.corner === 'top-left';
	const fromTop = position.corner === 'top-left' || position.corner === 'top-right';
	return within(area, {
		height,
		left: fromLeft ? area.left + position.x : area.left + area.width - position.x - width,
		top: fromTop ? area.top + position.y : area.top + area.height - position.y - height,
		width,
	});
}

/** Picture-in-picture moved from `origin` by the pointer's travel, kept inside the window below the 36px row. */
export function moveFileOverlayPip(
	regions: FileOverlayRegions,
	origin: FileOverlayBox,
	deltaX: number,
	deltaY: number,
): FileOverlayBox {
	return within(pipArea(regions), {
		...origin,
		left: origin.left + deltaX,
		top: origin.top + deltaY,
	});
}

/** The position that puts picture-in-picture where `box` is, kept from the corner nearest its center. */
export function fileOverlayPipPosition(
	regions: FileOverlayRegions,
	box: FileOverlayBox,
): FileOverlayPipPosition {
	const area = pipArea(regions);
	const fromLeft = box.left + box.width / 2 <= area.left + area.width / 2;
	const fromTop = box.top + box.height / 2 <= area.top + area.height / 2;
	const x = fromLeft ? box.left - area.left : area.left + area.width - box.left - box.width;
	const y = fromTop ? box.top - area.top : area.top + area.height - box.top - box.height;
	return {
		corner: nearestCorner(fromLeft, fromTop),
		x: Math.max(0, Math.round(x)),
		y: Math.max(0, Math.round(y)),
	};
}
