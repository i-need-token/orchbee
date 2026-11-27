/** VS Code's `workbench.list.mouseWheelScrollSensitivity` and `fastScrollSensitivity` defaults. */
const MOUSE_WHEEL_SCROLL_SENSITIVITY = 1;
const FAST_SCROLL_SENSITIVITY = 5;

/** Pixels per wheel unit; a Chromium wheel notch is one unit. */
const PIXELS_PER_UNIT = 50;

/** One legacy `wheelDelta` notch. */
const LEGACY_NOTCH = 120;

/** Pixel-mode deltas per wheel unit. */
const PIXELS_PER_DELTA_UNIT = 40;

/** Firefox outside macOS reports three lines per notch. */
const FIREFOX_LINES_PER_NOTCH = 3;

/** `WheelEvent.DOM_DELTA_LINE`: deltas count lines. */
const LINE_MODE = 1;

/** What the browser and system change about wheel deltas, read once from the user agent. */
interface WheelEnvironment {
	/** On macOS the system already turns Shift and a vertical wheel into a horizontal one. */
	readonly apple: boolean;
	readonly firefox: boolean;
	/** Chromium before 123 scales its legacy deltas by the device pixel ratio. */
	readonly legacyDeltaScaled: boolean;
}

export function wheelEnvironment(userAgent: string): WheelEnvironment {
	const chrome = /Chrome\/(\d+)/u.exec(userAgent);
	return {
		apple: userAgent.includes('Macintosh'),
		firefox: userAgent.includes('Firefox'),
		legacyDeltaScaled: chrome !== null && Number(chrome[1]) <= 122,
	};
}

/**
 * How far a wheel event scrolls a list, in whole pixels, positive downward: VS Code's list scrolling
 * without smoothing. Only the predominant axis counts, and a list never scrolls sideways, so a
 * horizontal wheel, or Shift with a vertical one outside macOS, scrolls nothing. Alt scrolls five
 * times as far, and a delta is rounded away from zero, so the slowest wheel still moves.
 */
export function wheelScrollDelta(
	event: WheelEvent,
	environment: WheelEnvironment,
	devicePixelRatio: number,
): number {
	const legacyNotch = environment.legacyDeltaScaled
		? LEGACY_NOTCH * devicePixelRatio
		: LEGACY_NOTCH;
	// Wheel units, positive upward and leftward, as VS Code counts them.
	let units = 0;
	if ('wheelDeltaY' in event && typeof event.wheelDeltaY === 'number') {
		units = event.wheelDeltaY / legacyNotch;
	} else {
		units = deltaUnits(event.deltaY, event.deltaMode, environment);
	}
	let sideways = 0;
	if ('wheelDeltaX' in event && typeof event.wheelDeltaX === 'number') {
		sideways = event.wheelDeltaX / legacyNotch;
	} else {
		sideways = deltaUnits(event.deltaX, event.deltaMode, environment);
	}
	if (
		units === 0 &&
		sideways === 0 &&
		'wheelDelta' in event &&
		typeof event.wheelDelta === 'number'
	) {
		units = event.wheelDelta / legacyNotch;
	}
	units *= MOUSE_WHEEL_SCROLL_SENSITIVITY;
	sideways *= MOUSE_WHEEL_SCROLL_SENSITIVITY;
	if (Math.abs(units) < Math.abs(sideways)) return 0;
	if (event.shiftKey && !environment.apple) return 0;
	if (event.altKey) units *= FAST_SCROLL_SENSITIVITY;
	const pixels = PIXELS_PER_UNIT * units;
	if (pixels === 0) return 0;
	return -(pixels < 0 ? Math.floor(pixels) : Math.ceil(pixels));
}

function deltaUnits(delta: number, deltaMode: number, environment: WheelEnvironment): number {
	if (deltaMode !== LINE_MODE) return -delta / PIXELS_PER_DELTA_UNIT;
	return environment.firefox && !environment.apple ? -delta / FIREFOX_LINES_PER_NOTCH : -delta;
}
