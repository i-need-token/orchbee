import type { FileOverlayTier } from './overlay-mode';

/** A box in the Workbench root's coordinates, in CSS pixels. */
export interface FileOverlayBox {
	readonly height: number;
	readonly left: number;
	readonly top: number;
	readonly width: number;
}

/** What the overlay lies over, measured in the Workbench root. */
export interface FileOverlayRegions {
	/** The docked Sidebar's left and right edges, or null while the Sidebar is not docked. */
	readonly sidebar: { readonly left: number; readonly right: number } | null;
	/** The terminal area, from the 36px row down to the window's bottom. */
	readonly terminal: FileOverlayBox;
	/** The Workbench root's width. */
	readonly width: number;
}

/**
 * The part of the window a tier covers: the scrim dims it and the frame sits inside it, inset on
 * every side. Every tier starts below the 36px row and reaches the window's bottom, so the row stays
 * uncovered. Left-expand covers what default does while no Sidebar is docked, until the layout change
 * that withdrew it reaches the overlay's tier.
 */
export function fileOverlayCover(
	regions: FileOverlayRegions,
	tier: FileOverlayTier,
): FileOverlayBox {
	const { terminal } = regions;
	switch (tier) {
		case 'default':
			return terminal;
		case 'left-expand':
			return regions.sidebar === null
				? terminal
				: spanning(terminal, regions.sidebar.left, terminal.left + terminal.width);
		case 'full':
			return spanning(terminal, 0, regions.width);
	}
}

function spanning(terminal: FileOverlayBox, left: number, right: number): FileOverlayBox {
	return { height: terminal.height, left, top: terminal.top, width: Math.max(0, right - left) };
}
