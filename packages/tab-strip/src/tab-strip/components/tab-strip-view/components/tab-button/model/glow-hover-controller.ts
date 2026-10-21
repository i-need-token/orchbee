const GLOW_HOVER_DEFAULT_DURATION_MS = 200;
const GLOW_HOVER_SUBTLE_OPACITY_SCALE = 0.45;
const GLOW_HOVER_PRONOUNCED_OPACITY_SCALE = 1;

export const ShowHoverStyle = {
	Subtle: 'subtle',
	Pronounced: 'pronounced',
} as const;

export type ShowHoverStyle = (typeof ShowHoverStyle)[keyof typeof ShowHoverStyle];

export const HideHoverStyle = {
	Gradual: 'gradual',
	Immediate: 'immediate',
} as const;

export type HideHoverStyle = (typeof HideHoverStyle)[keyof typeof HideHoverStyle];

type HoverTween = 'ease-in' | 'ease-out' | 'none';

const clamp01 = (value: number) => Math.min(1, Math.max(0, value));

function sampleBezier(state: number, x1: number, y1: number, x2: number, y2: number): number {
	const x = clamp01(state);
	if (x === 0 || x === 1) return x;
	const sample = (t: number, p1: number, p2: number) => {
		const inverse = 1 - t;
		return 3 * inverse * inverse * t * p1 + 3 * inverse * t * t * p2 + t ** 3;
	};
	const derivative = (t: number, p1: number, p2: number) =>
		3 * (1 - t) ** 2 * p1 + 6 * (1 - t) * t * (p2 - p1) + 3 * t ** 2 * (1 - p2);
	let t = x;
	for (let iteration = 0; iteration < 8; iteration += 1) {
		const slope = derivative(t, x1, x2);
		if (Math.abs(slope) < 1e-7) break;
		const error = sample(t, x1, x2) - x;
		if (Math.abs(error) < 1e-7) break;
		t = clamp01(t - error / slope);
	}
	return clamp01(sample(t, y1, y2));
}

export function sampleGlowHoverTween(tween: HoverTween, state: number): number {
	if (tween === 'ease-in') return sampleBezier(state, 0.42, 0, 1, 1);
	if (tween === 'ease-out') return sampleBezier(state, 0, 0, 0.58, 1);
	return clamp01(state);
}

/** Browser-independent state machine for Chromium's GlowHoverController. */
export class GlowHoverController {
	private readonly durationMs: number;
	private subtleOpacityScale = GLOW_HOVER_SUBTLE_OPACITY_SCALE;
	private opacityScale = GLOW_HOVER_SUBTLE_OPACITY_SCALE;
	private startValue = 0;
	private targetValue = 0;
	private startedAt = 0;
	private tween: HoverTween = 'none';
	private animating = false;

	public constructor(durationMs = GLOW_HOVER_DEFAULT_DURATION_MS) {
		this.durationMs = Math.max(0, durationMs);
	}

	public setSubtleOpacityScale(opacityScale: number): void {
		this.subtleOpacityScale = clamp01(opacityScale);
	}

	public show(style: ShowHoverStyle, now = Date.now()): void {
		if (style === ShowHoverStyle.Pronounced) {
			this.opacityScale = GLOW_HOVER_PRONOUNCED_OPACITY_SCALE;
			this.finishAt(1, now);
			return;
		}
		this.opacityScale = this.subtleOpacityScale;
		this.beginTransition(1, 'ease-out', now);
	}

	public hide(style: HideHoverStyle, now = Date.now()): void {
		if (style === HideHoverStyle.Immediate) {
			this.finishAt(0, now);
			return;
		}
		this.beginTransition(0, 'ease-in', now);
	}

	public getAnimationValue(now = Date.now()): number {
		if (!this.animating) return this.targetValue;
		const elapsed = Math.max(0, now - this.startedAt);
		if (this.durationMs === 0 || elapsed >= this.durationMs) {
			this.animating = false;
			this.startValue = this.targetValue;
			return this.targetValue;
		}
		const progress = sampleGlowHoverTween(this.tween, elapsed / this.durationMs);
		return this.startValue + (this.targetValue - this.startValue) * progress;
	}

	public getAlpha(now = Date.now()): number {
		return Math.round(255 * this.getAnimationValue(now) * this.opacityScale);
	}

	public shouldDraw(now = Date.now()): boolean {
		this.getAnimationValue(now);
		return this.targetValue === 1 || this.animating;
	}

	private beginTransition(target: number, tween: HoverTween, now: number): void {
		const current = this.getAnimationValue(now);
		this.startValue = current;
		this.targetValue = target;
		this.startedAt = now;
		this.tween = tween;
		this.animating = this.durationMs > 0 && current !== target;
		if (!this.animating) this.startValue = target;
	}

	private finishAt(target: number, now: number): void {
		this.startValue = target;
		this.targetValue = target;
		this.startedAt = now;
		this.tween = 'none';
		this.animating = false;
	}
}
