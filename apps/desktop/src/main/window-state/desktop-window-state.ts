import { z } from 'zod';

const DEFAULT_WINDOW_HEIGHT = 720;
const DEFAULT_WINDOW_WIDTH = 1_024;
const MAX_WINDOW_COORDINATE = 1_000_000;
const MAX_WINDOW_DIMENSION = 32_768;
const MIN_WINDOW_HEIGHT = 240;
const MIN_WINDOW_WIDTH = 320;

const desktopWindowBoundsSchema = z.strictObject({
	height: z.number().int().min(MIN_WINDOW_HEIGHT).max(MAX_WINDOW_DIMENSION),
	width: z.number().int().min(MIN_WINDOW_WIDTH).max(MAX_WINDOW_DIMENSION),
	x: z.number().int().min(-MAX_WINDOW_COORDINATE).max(MAX_WINDOW_COORDINATE),
	y: z.number().int().min(-MAX_WINDOW_COORDINATE).max(MAX_WINDOW_COORDINATE),
});

const desktopWindowStateSchema = z.strictObject({
	bounds: desktopWindowBoundsSchema,
	maximized: z.boolean(),
	version: z.literal(1),
});

export interface DesktopWindowBounds {
	readonly height: number;
	readonly width: number;
	readonly x: number;
	readonly y: number;
}

export interface DesktopWindowState {
	readonly bounds: DesktopWindowBounds;
	readonly maximized: boolean;
	readonly version: 1;
}

export type DesktopWindowStateLoadResult =
	| { readonly kind: 'available'; readonly state: DesktopWindowState }
	| { readonly kind: 'missing' }
	| { readonly kind: 'unavailable'; readonly reason: 'invalid' | 'read-failed' }
	/**
	 * The record's version is one this build cannot use: saved by a newer version, or an earlier
	 * one that could not be migrated, backed up, or replaced. It is never rewritten.
	 */
	| {
			readonly kind: 'unusable';
			readonly problem: DesktopWindowStateRecordProblem;
			readonly version: number;
	  };

export type DesktopWindowStateRecordProblem = 'newer-format' | 'not-updated';

/** The record version this build writes. */
export const DESKTOP_WINDOW_STATE_VERSION = 1;

/**
 * One step from version `from` to `from + 1` of the window state record: a pure function of the
 * parsed record, `extensions` included, that answers `null` when it cannot convert it. The result
 * carries `version` `from + 1`.
 */
export interface DesktopWindowStateMigrationStep {
	readonly apply: (
		record: Readonly<Record<string, unknown>>,
	) => Readonly<Record<string, unknown>> | null;
	readonly from: number;
}

/**
 * The steps that bring an earlier record to {@link DESKTOP_WINDOW_STATE_VERSION}. No format has
 * changed since the first public release, so there are none yet; the first change adds its step
 * here, and the store's opening backs the record up and persists it.
 */
export const DESKTOP_WINDOW_STATE_STEPS: readonly DesktopWindowStateMigrationStep[] = [];

export interface DesktopWindowStateStore {
	flush(): Promise<void>;
	load(): Promise<DesktopWindowStateLoadResult>;
	save(state: DesktopWindowState): Promise<boolean>;
}

export interface DesktopWindowDisplayTopology {
	readonly primaryWorkArea: DesktopWindowBounds;
	readonly workAreas: readonly DesktopWindowBounds[];
}

export interface DesktopWindowLaunchState {
	readonly bounds: DesktopWindowBounds | { readonly height: number; readonly width: number };
	readonly maximized: boolean;
}

export function createDesktopWindowState(
	bounds: DesktopWindowBounds,
	maximized: boolean,
): DesktopWindowState | null {
	return parseDesktopWindowState({ bounds, maximized, version: 1 });
}

export function parseDesktopWindowState(value: unknown): DesktopWindowState | null {
	const parsed = desktopWindowStateSchema.safeParse(value);
	return parsed.success ? parsed.data : null;
}

export function resolveDesktopWindowLaunchState(
	state: DesktopWindowState | null,
	topology: DesktopWindowDisplayTopology,
): DesktopWindowLaunchState {
	if (state === null) return defaultLaunchState();
	const primaryWorkArea = validWorkArea(topology.primaryWorkArea);
	if (primaryWorkArea === null) return { ...defaultLaunchState(), maximized: state.maximized };
	const workAreas = topology.workAreas
		.map((workArea) => validWorkArea(workArea))
		.filter((workArea): workArea is DesktopWindowBounds => workArea !== null);
	const target = selectTargetWorkArea(state.bounds, workAreas, primaryWorkArea);
	const width = Math.min(state.bounds.width, target.width);
	const height = Math.min(state.bounds.height, target.height);
	return {
		bounds: {
			height,
			width,
			x: clamp(state.bounds.x, target.x, target.x + target.width - width),
			y: clamp(state.bounds.y, target.y, target.y + target.height - height),
		},
		maximized: state.maximized,
	};
}

function defaultLaunchState(): DesktopWindowLaunchState {
	return {
		bounds: { height: DEFAULT_WINDOW_HEIGHT, width: DEFAULT_WINDOW_WIDTH },
		maximized: false,
	};
}

function selectTargetWorkArea(
	bounds: DesktopWindowBounds,
	workAreas: readonly DesktopWindowBounds[],
	primaryWorkArea: DesktopWindowBounds,
): DesktopWindowBounds {
	let target = primaryWorkArea;
	let targetOverlap = intersectionArea(bounds, primaryWorkArea);
	for (const workArea of workAreas) {
		const overlap = intersectionArea(bounds, workArea);
		if (overlap > targetOverlap) {
			target = workArea;
			targetOverlap = overlap;
		}
	}
	return targetOverlap === 0 ? primaryWorkArea : target;
}

function intersectionArea(left: DesktopWindowBounds, right: DesktopWindowBounds): number {
	const width = Math.max(
		0,
		Math.min(left.x + left.width, right.x + right.width) - Math.max(left.x, right.x),
	);
	const height = Math.max(
		0,
		Math.min(left.y + left.height, right.y + right.height) - Math.max(left.y, right.y),
	);
	return width * height;
}

function validWorkArea(workArea: DesktopWindowBounds): DesktopWindowBounds | null {
	return [workArea.x, workArea.y, workArea.width, workArea.height].every(Number.isInteger) &&
		workArea.width > 0 &&
		workArea.height > 0
		? workArea
		: null;
}

function clamp(value: number, minimum: number, maximum: number): number {
	return Math.min(Math.max(value, minimum), maximum);
}
