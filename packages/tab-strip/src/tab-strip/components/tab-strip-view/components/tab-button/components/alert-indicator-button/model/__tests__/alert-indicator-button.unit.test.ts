import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
	AlertIndicatorButtonController,
	ALERT_INDICATOR_RECORDING_ANIMATION_MS,
	getTabAlertColorRole,
	getTabAlertStateText,
	isPulsingAlertState,
	requiresCameraMicMinimumHold,
	sampleAlertIndicatorOpacity,
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

	it('runs two recording pulses and finishes visible after 2600ms', () => {
		expect(ALERT_INDICATOR_RECORDING_ANIMATION_MS).toBe(2_600);
		expect(sampleAlertIndicatorOpacity('recording', 0, 2_600)).toBe(0);
		expect(sampleAlertIndicatorOpacity('recording', 200, 2_600)).toBe(1);
		expect(sampleAlertIndicatorOpacity('recording', 1_200, 2_600)).toBe(0);
		expect(sampleAlertIndicatorOpacity('recording', 2_600, 2_600)).toBe(1);
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
		const pulsing = [
			'media-recording',
			'audio-recording',
			'video-recording',
			'tab-capturing',
			'desktop-capturing',
		] as const;
		expect(pulsing.every(isPulsingAlertState)).toBe(true);
		expect(pulsing.filter(requiresCameraMicMinimumHold)).toEqual([
			'media-recording',
			'audio-recording',
			'video-recording',
		]);

		const tabCapture = new AlertIndicatorButtonController();
		tabCapture.transitionTo('tab-capturing', 1_000);
		tabCapture.transitionTo('none', 2_000);
		expect(tabCapture.getSnapshot().fadeDurationMs).toBe(1_000);
	});

	it('exposes alert text and hover-card color roles', () => {
		expect(getTabAlertStateText('video-recording')).toBe('This tab is using your camera');
		expect(getTabAlertColorRole('desktop-capturing')).toBe('recording');
		expect(getTabAlertColorRole('usb-connected')).toBe('default');
	});
});
