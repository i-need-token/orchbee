import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
	AlertIndicatorButtonController,
	getTabAlertColorRole,
	getTabAlertStateText,
} from '../alert-indicator-button';

describe('AlertIndicatorButtonController', () => {
	beforeEach(() => vi.useFakeTimers());
	afterEach(() => vi.useRealTimers());

	it('switches between audio display states without a transition delay', () => {
		const controller = new AlertIndicatorButtonController('audio-playing');
		controller.transitionTo('audio-muting');
		expect(controller.getSnapshot()).toMatchObject({
			alertState: 'audio-muting',
			showingAlertState: 'audio-muting',
			fadeKind: 'none',
		});
	});

	it('holds a short recording until five seconds before the fade is complete', () => {
		const controller = new AlertIndicatorButtonController();
		controller.transitionTo('recording', 1_000);
		controller.transitionTo('none', 2_000);
		expect(controller.getSnapshot()).toMatchObject({
			showingAlertState: 'recording',
			fadeKind: 'fade-out',
			fadeDurationMs: 4_000,
		});
		vi.advanceTimersByTime(3_999);
		expect(controller.getSnapshot().showingAlertState).toBe('recording');
		vi.advanceTimersByTime(1);
		expect(controller.getSnapshot()).toMatchObject({ showingAlertState: 'none', fadeKind: 'none' });
	});

	it('pulses all five Chromium capture states but only holds camera and microphone use', () => {
		for (const state of [
			'media-recording',
			'audio-recording',
			'video-recording',
			'tab-capturing',
			'desktop-capturing',
		] as const) {
			const controller = new AlertIndicatorButtonController();
			controller.transitionTo(state, 1_000);
			expect(controller.getSnapshot()).toMatchObject({
				fadeKind: 'recording',
				fadeDurationMs: 2_600,
			});
			controller.transitionTo('none', 2_000);
			expect(controller.getSnapshot().fadeDurationMs).toBe(
				state === 'tab-capturing' || state === 'desktop-capturing' ? 1_000 : 4_000,
			);
			controller.destroy();
		}
	});

	it('exposes alert text and hover-card color roles', () => {
		expect(getTabAlertStateText('video-recording')).toBe('This tab is using your camera');
		expect(getTabAlertColorRole('desktop-capturing')).toBe('recording');
		expect(getTabAlertColorRole('usb-connected')).toBe('default');
	});
});
