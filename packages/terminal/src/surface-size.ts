import type { PaddingBalance, SurfacePadding } from './types';

export interface SurfaceSize {
	readonly width: number;
	readonly height: number;
}

export interface SurfaceGridSize {
	readonly columns: number;
	readonly rows: number;
}

const MINIMUM_COLUMNS = 2;
const MINIMUM_ROWS = 1;

/** Calculate the terminal grid that remains after explicit padding. */
export function calculateSurfaceGrid(
	screen: SurfaceSize,
	cell: SurfaceSize,
	explicit: SurfacePadding,
): SurfaceGridSize | undefined {
	const availableWidth = screen.width - explicit.left - explicit.right;
	const availableHeight = screen.height - explicit.top - explicit.bottom;
	if (availableWidth <= 0 || availableHeight <= 0 || cell.width <= 0 || cell.height <= 0) {
		return undefined;
	}
	return {
		columns: Math.max(MINIMUM_COLUMNS, Math.floor(availableWidth / cell.width)),
		rows: Math.max(MINIMUM_ROWS, Math.floor(availableHeight / cell.height)),
	};
}

/** Match Ghostty's balanced surface padding after explicit padding determines the grid. */
export function balanceSurfacePadding(
	screen: SurfaceSize,
	cell: SurfaceSize,
	explicit: SurfacePadding,
	mode: Exclude<PaddingBalance, false>,
): SurfacePadding {
	const grid = calculateSurfaceGrid(screen, cell, explicit);
	if (grid === undefined) return { ...explicit };
	return balanceGridPadding(screen, cell, explicit, grid, mode);
}

/** Balance padding around an already selected grid and any reserved scrollbar space. */
export function balanceGridPadding(
	screen: SurfaceSize,
	cell: SurfaceSize,
	explicit: SurfacePadding,
	grid: SurfaceGridSize,
	mode: Exclude<PaddingBalance, false>,
): SurfacePadding {
	const horizontal = Math.max(0, Math.floor((screen.width - grid.columns * cell.width) / 2));
	const vertical = Math.max(0, Math.floor((screen.height - grid.rows * cell.height) / 2));
	const padding: SurfacePadding = {
		top: vertical,
		bottom: vertical,
		left: horizontal,
		right: horizontal,
	};

	if (mode === 'equal') return padding;

	const maximumTop = Math.floor((explicit.left + explicit.right + cell.width) / 2);
	const verticalShift = Math.max(0, padding.top - maximumTop);
	return {
		...padding,
		top: padding.top - verticalShift,
		bottom: padding.bottom + verticalShift,
	};
}

/** Return CSS margins needed in addition to padding already applied by the surface element. */
export function additionalSurfaceMargins(
	balanced: SurfacePadding,
	explicit: SurfacePadding,
): SurfacePadding {
	return {
		top: Math.max(0, balanced.top - explicit.top),
		bottom: Math.max(0, balanced.bottom - explicit.bottom),
		left: Math.max(0, balanced.left - explicit.left),
		right: Math.max(0, balanced.right - explicit.right),
	};
}
