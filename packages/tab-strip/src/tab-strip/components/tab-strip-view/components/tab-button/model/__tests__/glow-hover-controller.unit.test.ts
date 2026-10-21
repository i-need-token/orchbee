import { describe, expect, it } from 'vitest';
import {
	HideHoverStyle,
	ShowHoverStyle,
	GlowHoverController,
	sampleGlowHoverTween,
} from '../glow-hover-controller';

describe('GlowHoverController', () => {
	it('eases a subtle show out and a gradual hide in', () => {
		const controller = new GlowHoverController(200);
		controller.show(ShowHoverStyle.Subtle, 1_000);
		expect(controller.getAnimationValue(1_000)).toBe(0);
		expect(controller.getAnimationValue(1_100)).toBeGreaterThan(0.5);
		expect(controller.getAnimationValue(1_200)).toBe(1);
		expect(controller.shouldDraw(1_200)).toBe(true);

		controller.hide(HideHoverStyle.Gradual, 2_000);
		expect(controller.getAnimationValue(2_100)).toBeGreaterThan(0.5);
		expect(controller.getAnimationValue(2_200)).toBe(0);
		expect(controller.shouldDraw(2_200)).toBe(false);
	});

	it('shows pronounced hover and hides immediate hover without animation', () => {
		const controller = new GlowHoverController();
		controller.show(ShowHoverStyle.Pronounced, 10);
		expect(controller.getAnimationValue(10)).toBe(1);
		expect(controller.getAlpha(10)).toBe(255);
		controller.hide(HideHoverStyle.Immediate, 20);
		expect(controller.getAnimationValue(20)).toBe(0);
		expect(controller.shouldDraw(20)).toBe(false);
	});

	it('applies a configurable subtle alpha scale', () => {
		const controller = new GlowHoverController(0);
		controller.setSubtleOpacityScale(0.2);
		controller.show(ShowHoverStyle.Subtle, 0);
		expect(controller.getAlpha(0)).toBe(51);
	});

	it('uses Chromium ease-in/ease-out curves', () => {
		expect(sampleGlowHoverTween('ease-out', 0.5)).toBeGreaterThan(0.5);
		expect(sampleGlowHoverTween('ease-in', 0.5)).toBeLessThan(0.5);
	});
});
