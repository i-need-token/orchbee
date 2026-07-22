export const TAB_STRIP_NUDGE_EXPANSION_IN_MS = 500;
export const TAB_STRIP_NUDGE_EXPANSION_OUT_MS = 250;
export const TAB_STRIP_NUDGE_OPACITY_IN_MS = 300;
export const TAB_STRIP_NUDGE_OPACITY_OUT_MS = 100;
export const TAB_STRIP_NUDGE_OPACITY_DELAY_MS = 100;
export const TAB_STRIP_NUDGE_EXPANSION_EASING = 'cubic-bezier(0.2, 0, 0, 1)';

export const TabStripNudgeAnimationSessionType = {
	Show: 'show',
	Hide: 'hide',
} as const;

export type TabStripNudgeAnimationSessionType =
	(typeof TabStripNudgeAnimationSessionType)[keyof typeof TabStripNudgeAnimationSessionType];

export interface TabStripNudgeAnimationPlan {
	type: TabStripNudgeAnimationSessionType;
	expansionDurationMs: number;
	expansionEasing: typeof TAB_STRIP_NUDGE_EXPANSION_EASING;
	opacityDurationMs: number;
	opacityDelayMs: number;
	animateOpacity: boolean;
	totalDurationMs: number;
}

export interface TabStripNudgeAnimationSample {
	expansion: number;
	opacity: number;
	done: boolean;
}

const clamp01 = (value: number) => Math.min(1, Math.max(0, value));

/** Plan translated from TabStripNudgeAnimationSession::Show/Hide. */
export function createTabStripNudgeAnimationPlan(
	type: TabStripNudgeAnimationSessionType,
	options: { animateOpacity?: boolean; richAnimations?: boolean } = {},
): TabStripNudgeAnimationPlan {
	const animateOpacity = options.animateOpacity ?? true;
	const rich = options.richAnimations ?? true;
	let expansionDurationMs = 0;
	if (rich) {
		expansionDurationMs =
			type === TabStripNudgeAnimationSessionType.Show
				? TAB_STRIP_NUDGE_EXPANSION_IN_MS
				: TAB_STRIP_NUDGE_EXPANSION_OUT_MS;
	}
	let opacityDurationMs = 0;
	if (rich && animateOpacity) {
		opacityDurationMs =
			type === TabStripNudgeAnimationSessionType.Show
				? TAB_STRIP_NUDGE_OPACITY_IN_MS
				: TAB_STRIP_NUDGE_OPACITY_OUT_MS;
	}
	const opacityDelayMs =
		rich && animateOpacity && type === TabStripNudgeAnimationSessionType.Show
			? TAB_STRIP_NUDGE_OPACITY_DELAY_MS
			: 0;
	return {
		type,
		expansionDurationMs,
		expansionEasing: TAB_STRIP_NUDGE_EXPANSION_EASING,
		opacityDurationMs,
		opacityDelayMs,
		animateOpacity,
		totalDurationMs: Math.max(expansionDurationMs, opacityDelayMs + opacityDurationMs),
	};
}

// gfx::Tween::ACCEL_20_DECEL_100 is CubicBezier(0.2, 0, 0, 1).
export function calculateAccel20Decel100(state: number): number {
	const x = clamp01(state);
	if (x === 0 || x === 1) return x;
	const sample = (t: number, p1: number, p2: number) => {
		const inverse = 1 - t;
		return 3 * inverse * inverse * t * p1 + 3 * inverse * t * t * p2 + t * t * t;
	};
	const derivative = (t: number, p1: number, p2: number) =>
		3 * (1 - t) * (1 - t) * p1 + 6 * (1 - t) * t * (p2 - p1) + 3 * t * t * (1 - p2);

	let t = x;
	for (let iteration = 0; iteration < 8; iteration += 1) {
		const error = sample(t, 0.2, 0) - x;
		const slope = derivative(t, 0.2, 0);
		if (Math.abs(error) < 1e-7) break;
		if (Math.abs(slope) < 1e-7) {
			let low = 0;
			let high = 1;
			for (let step = 0; step < 18; step += 1) {
				t = (low + high) / 2;
				if (sample(t, 0.2, 0) < x) low = t;
				else high = t;
			}
			break;
		}
		t = clamp01(t - error / slope);
	}
	return clamp01(sample(t, 0, 1));
}

/** Deterministic sampling equivalent to applying both SlideAnimations. */
export function sampleTabStripNudgeAnimation(
	plan: Readonly<TabStripNudgeAnimationPlan>,
	elapsedMs: number,
): TabStripNudgeAnimationSample {
	const expansionProgress =
		plan.expansionDurationMs === 0 ? 1 : clamp01(elapsedMs / plan.expansionDurationMs);
	const expansionTween = calculateAccel20Decel100(expansionProgress);
	const opacityProgress =
		!plan.animateOpacity || plan.opacityDurationMs === 0
			? 1
			: clamp01((elapsedMs - plan.opacityDelayMs) / plan.opacityDurationMs);
	const showing = plan.type === TabStripNudgeAnimationSessionType.Show;
	return {
		expansion: showing ? expansionTween : 1 - expansionTween,
		opacity: plan.animateOpacity ? (showing ? opacityProgress : 1 - opacityProgress) : 1,
		done: elapsedMs >= plan.totalDurationMs,
	};
}
