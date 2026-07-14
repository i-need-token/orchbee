import type { PaddingBalance, SurfacePadding } from './types';

export interface SurfaceSize {
	readonly width: number;
	readonly height: number;
}

export interface SurfaceGridSize {
	readonly columns: number;
	readonly rows: number;
}

/** Calculate the terminal grid that remains after explicit padding. */
export function calculateSurfaceGrid(
	screen: SurfaceSize,
	cell: SurfaceSize,
	explicit: SurfacePadding,
): SurfaceGridSize {
	const availableWidth = Math.max(0, screen.width - explicit.left - explicit.right);
	const availableHeight = Math.max(0, screen.height - explicit.top - explicit.bottom);
	return {
		columns: Math.max(1, Math.floor(availableWidth / Math.max(1, cell.width))),
		rows: Math.max(1, Math.floor(availableHeight / Math.max(1, cell.height))),
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
	return balanceGridPadding(screen, cell, explicit, grid, mode);
}

/** Balance padding around an already selected grid, including FitAddon scrollbar space. */
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
