import { describe, expect, it } from 'vitest';
import {
	calculateAccel20Decel100,
	createTabStripNudgeAnimationPlan,
	sampleTabStripNudgeAnimation,
	TabStripNudgeAnimationSessionType,
} from '../tab-strip-nudge-animation';

describe('TabStripNudgeAnimationSession port', () => {
	it('uses the exact show timing and delayed opacity stage', () => {
		const plan = createTabStripNudgeAnimationPlan(TabStripNudgeAnimationSessionType.Show);
		expect(plan).toMatchObject({
			expansionDurationMs: 500,
			opacityDurationMs: 300,
			opacityDelayMs: 100,
			totalDurationMs: 500,
		});
		expect(sampleTabStripNudgeAnimation(plan, 0)).toEqual({
			expansion: 0,
			opacity: 0,
			done: false,
		});
		expect(sampleTabStripNudgeAnimation(plan, 100).opacity).toBe(0);
		expect(sampleTabStripNudgeAnimation(plan, 250).opacity).toBe(0.5);
		expect(sampleTabStripNudgeAnimation(plan, 500)).toEqual({
			expansion: 1,
			opacity: 1,
			done: true,
		});
	});

	it('uses the shorter hide timing with immediate opacity loss', () => {
		const plan = createTabStripNudgeAnimationPlan(TabStripNudgeAnimationSessionType.Hide);
		expect(plan).toMatchObject({
			expansionDurationMs: 250,
			opacityDurationMs: 100,
			opacityDelayMs: 0,
			totalDurationMs: 250,
		});
		expect(sampleTabStripNudgeAnimation(plan, 50).opacity).toBe(0.5);
		expect(sampleTabStripNudgeAnimation(plan, 100).opacity).toBe(0);
		expect(sampleTabStripNudgeAnimation(plan, 250)).toEqual({
			expansion: 0,
			opacity: 0,
			done: true,
		});
	});

	it('keeps Glic and actor controls opaque when opacity is owned elsewhere', () => {
		const plan = createTabStripNudgeAnimationPlan(TabStripNudgeAnimationSessionType.Show, {
			animateOpacity: false,
		});
		expect(sampleTabStripNudgeAnimation(plan, 0).opacity).toBe(1);
		expect(sampleTabStripNudgeAnimation(plan, 250).opacity).toBe(1);
	});

	it('snaps both channels when rich animation is disabled', () => {
		const plan = createTabStripNudgeAnimationPlan(TabStripNudgeAnimationSessionType.Show, {
			richAnimations: false,
		});
		expect(sampleTabStripNudgeAnimation(plan, 0)).toEqual({ expansion: 1, opacity: 1, done: true });
	});

	it('ports Chromium ACCEL_20_DECEL_100 as the 0.2/0/0/1 bezier', () => {
		expect(calculateAccel20Decel100(0)).toBe(0);
		expect(calculateAccel20Decel100(0.5)).toBeGreaterThan(0.8);
		expect(calculateAccel20Decel100(1)).toBe(1);
	});
});
