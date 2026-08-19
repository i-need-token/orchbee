import {
	PROTOCOL_LIMITS,
	TerminalGeometrySchema,
	type TerminalGeometry,
} from '@orchbee/terminal-protocol';

import { loadTerminalFonts, measureTerminalFont } from './font-metrics';
import type { TerminalPresentationConfiguration } from './presentation-configuration';
import { calculateSurfaceGrid } from './surface-size';
import type { SurfacePadding } from './types';

const MAX_PTY_PIXEL_DIMENSION = 0xffff;

export interface MountedTerminalGeometryOptions {
	readonly devicePixelRatio?: number;
}

/**
 * Measure the mounted viewport with the same font and grid rules used by the
 * first projected-terminal resize. An unmounted or padding-consumed viewport
 * has no geometry and must be observed again by the caller.
 */
export async function measureMountedTerminalGeometry(
	root: HTMLElement,
	configuration: TerminalPresentationConfiguration,
	options: MountedTerminalGeometryOptions = {},
): Promise<TerminalGeometry | undefined> {
	try {
		await loadTerminalFonts(root.ownerDocument, configuration);
	} catch {
		// ProjectedTerminal also continues with the currently available face
		// after a font-loading failure, so startup measurement follows it.
	}
	if (root.clientWidth <= 0 || root.clientHeight <= 0) return undefined;
	const view = root.ownerDocument.defaultView;
	const devicePixelRatio = options.devicePixelRatio ?? view?.devicePixelRatio ?? 1;
	const metrics = measureTerminalFont(root.ownerDocument, configuration, devicePixelRatio);
	const grid = calculateSurfaceGrid(
		{ height: root.clientHeight, width: root.clientWidth },
		{ height: metrics.cellHeight, width: metrics.cellWidth },
		readSurfacePadding(root),
	);
	if (grid === undefined) return undefined;
	const cellPixelHeight = clamp(
		Math.round(metrics.cellHeight * devicePixelRatio),
		1,
		PROTOCOL_LIMITS.maxCellPixelDimension,
	);
	const cellPixelWidth = clamp(
		Math.round(metrics.cellWidth * devicePixelRatio),
		1,
		PROTOCOL_LIMITS.maxCellPixelDimension,
	);
	const columns = clamp(
		grid.columns,
		1,
		Math.min(PROTOCOL_LIMITS.maxFrameColumns, Math.floor(MAX_PTY_PIXEL_DIMENSION / cellPixelWidth)),
	);
	const rows = clamp(
		grid.rows,
		1,
		Math.min(
			PROTOCOL_LIMITS.maxFrameRows,
			Math.floor(PROTOCOL_LIMITS.maxFrameCells / columns),
			Math.floor(MAX_PTY_PIXEL_DIMENSION / cellPixelHeight),
		),
	);
	return TerminalGeometrySchema.parse({ cellPixelHeight, cellPixelWidth, columns, rows });
}

function readSurfacePadding(root: HTMLElement): SurfacePadding {
	const view = root.ownerDocument.defaultView;
	if (view === null) return { bottom: 0, left: 0, right: 0, top: 0 };
	const style = view.getComputedStyle(root);
	return {
		bottom: cssPixels(style.paddingBottom),
		left: cssPixels(style.paddingLeft),
		right: cssPixels(style.paddingRight),
		top: cssPixels(style.paddingTop),
	};
}

function cssPixels(value: string): number {
	const parsed = Number.parseFloat(value);
	return Number.isFinite(parsed) ? Math.max(0, parsed) : 0;
}

function clamp(value: number, minimum: number, maximum: number): number {
	return Math.max(minimum, Math.min(maximum, value));
}
