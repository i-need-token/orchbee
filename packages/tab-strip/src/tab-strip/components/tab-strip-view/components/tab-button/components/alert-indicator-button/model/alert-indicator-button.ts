import type { TabAlertState } from '../../../../../../../model/tab-model';

export const ALERT_INDICATOR_FADE_IN_MS = 200;
export const ALERT_INDICATOR_FADE_OUT_MS = 1_000;
export const ALERT_INDICATOR_MINIMUM_HOLD_MS = 5_000;
export const ALERT_INDICATOR_FRAME_INTERVAL_MS = 50;
export const ALERT_INDICATOR_RECORDING_CYCLES = 2;
export const ALERT_INDICATOR_RECORDING_ANIMATION_MS =
	ALERT_INDICATOR_RECORDING_CYCLES * (ALERT_INDICATOR_FADE_IN_MS + ALERT_INDICATOR_FADE_OUT_MS) +
	ALERT_INDICATOR_FADE_IN_MS;
export type AlertIndicatorFadeKind = 'none' | 'fade-in' | 'fade-out' | 'recording';

export interface AlertIndicatorSnapshot {
	revision: number;
	alertState: TabAlertState;
	showingAlertState: TabAlertState;
	fadeKind: AlertIndicatorFadeKind;
	fadeDurationMs: number;
}

type Listener = () => void;

export type TabAlertColorRole = 'default' | 'recording' | 'pip';

export const isAudioAlertState = (state: TabAlertState): boolean =>
	state === 'audio-playing' || state === 'audio-muting' || state === 'audio-muted';

export const isPulsingAlertState = (state: TabAlertState): boolean =>
	state === 'media-recording' ||
	state === 'audio-recording' ||
	state === 'video-recording' ||
	state === 'tab-capturing' ||
	state === 'desktop-capturing' ||
	state === 'recording' ||
	state === 'capturing';

export const requiresCameraMicMinimumHold = (state: TabAlertState): boolean =>
	state === 'media-recording' ||
	state === 'audio-recording' ||
	state === 'video-recording' ||
	state === 'recording';

export function getTabAlertColorRole(state: TabAlertState): TabAlertColorRole {
	switch (state) {
		case 'media-recording':
		case 'audio-recording':
		case 'video-recording':
		case 'desktop-capturing':
		case 'recording':
			return 'recording';
		case 'tab-capturing':
		case 'capturing':
		case 'pip-playing':
			return 'pip';
		default:
			return 'default';
	}
}

export function getTabAlertStateText(state: TabAlertState): string {
	switch (state) {
		case 'audio-playing':
			return 'This tab is playing audio';
		case 'audio-muting':
		case 'audio-muted':
			return 'This tab is muted';
		case 'media-recording':
			return 'This tab is using your camera and microphone';
		case 'audio-recording':
			return 'This tab is using your microphone';
		case 'video-recording':
			return 'This tab is using your camera';
		case 'desktop-capturing':
			return 'This tab is sharing your screen';
		case 'tab-capturing':
		case 'capturing':
			return 'This tab is being shared';
		case 'pip-playing':
			return 'This tab is playing picture-in-picture';
		case 'bluetooth-connected':
			return 'This tab is connected to a Bluetooth device';
		case 'bluetooth-scan-active':
			return 'This tab is scanning for Bluetooth devices';
		case 'usb-connected':
			return 'This tab is connected to a USB device';
		case 'hid-connected':
			return 'This tab is connected to a HID device';
		case 'serial-connected':
			return 'This tab is connected to a serial port';
		case 'vr-presenting-in-headset':
			return 'This tab is presenting in a VR headset';
		case 'recording':
			return 'This tab is recording';
		default:
			return '';
	}
}

export function getTabAlertAccessibleDescription(state: TabAlertState): string | null {
	const text = getTabAlertStateText(state);
	if (!text) return null;
	return text.replace(/^This tab is /, '').replace(/^This tab /, 'tab ');
}

const clamp01 = (value: number) => Math.min(1, Math.max(0, value));

export function sampleAlertIndicatorOpacity(
	kind: AlertIndicatorFadeKind,
	elapsedMs: number,
	durationMs: number,
): number {
	if (kind === 'none') return 1;
	if (kind === 'recording') {
		let remaining = Math.max(0, elapsedMs);
		for (let cycle = 0; cycle < ALERT_INDICATOR_RECORDING_CYCLES; cycle += 1) {
			if (remaining <= ALERT_INDICATOR_FADE_IN_MS)
				return clamp01(remaining / ALERT_INDICATOR_FADE_IN_MS) ** 2;
			remaining -= ALERT_INDICATOR_FADE_IN_MS;
			if (remaining <= ALERT_INDICATOR_FADE_OUT_MS)
				return 1 - clamp01(remaining / ALERT_INDICATOR_FADE_OUT_MS) ** 2;
			remaining -= ALERT_INDICATOR_FADE_OUT_MS;
		}
		return clamp01(remaining / ALERT_INDICATOR_FADE_IN_MS) ** 2;
	}
	const value = clamp01(durationMs === 0 ? 1 : elapsedMs / durationMs) ** 2;
	return kind === 'fade-out' ? 1 - value : value;
}

/** Browser-independent alert-indicator transition state. */
export class AlertIndicatorButtonController {
	private readonly listeners = new Set<Listener>();
	private alertState: TabAlertState;
	private showingAlertState: TabAlertState;
	private fadeKind: AlertIndicatorFadeKind = 'none';
	private fadeDurationMs = 0;
	private cameraMicStartedAt: number | null = null;
	private fadeTimer: ReturnType<typeof setTimeout> | null = null;
	private revision = 0;
	private snapshot: AlertIndicatorSnapshot;

	public constructor(initialState: TabAlertState = 'none') {
		this.alertState = initialState;
		this.showingAlertState = initialState;
		if (requiresCameraMicMinimumHold(initialState)) this.cameraMicStartedAt = Date.now();
		this.snapshot = this.createSnapshot();
	}

	public getSnapshot = (): AlertIndicatorSnapshot => this.snapshot;
	public subscribe = (listener: Listener): (() => void) => {
		this.listeners.add(listener);
		return () => this.listeners.delete(listener);
	};

	public transitionTo(nextState: TabAlertState, now = Date.now()): void {
		if (nextState === this.alertState) return;
		const previous = this.alertState;
		this.clearFadeTimer();

		if (isAudioAlertState(previous) && isAudioAlertState(nextState)) {
			this.alertState = nextState;
			this.showingAlertState = nextState;
			this.fadeKind = 'none';
			this.fadeDurationMs = 0;
			this.commit();
			return;
		}

		this.alertState = nextState;
		if (nextState === 'none') {
			this.showingAlertState = previous;
			this.fadeKind = 'fade-out';
			this.fadeDurationMs =
				this.cameraMicStartedAt === null
					? ALERT_INDICATOR_FADE_OUT_MS
					: Math.max(
							ALERT_INDICATOR_MINIMUM_HOLD_MS - (now - this.cameraMicStartedAt),
							ALERT_INDICATOR_FADE_OUT_MS,
						);
			this.cameraMicStartedAt = null;
		} else if (isPulsingAlertState(nextState)) {
			this.showingAlertState = nextState;
			this.fadeKind = 'recording';
			this.fadeDurationMs = ALERT_INDICATOR_RECORDING_ANIMATION_MS;
			if (requiresCameraMicMinimumHold(nextState)) this.cameraMicStartedAt ??= now;
		} else {
			this.showingAlertState = nextState;
			this.fadeKind = 'fade-in';
			this.fadeDurationMs = ALERT_INDICATOR_FADE_IN_MS;
		}
		this.fadeTimer = setTimeout(() => {
			this.fadeTimer = null;
			this.showingAlertState = this.alertState;
			this.fadeKind = 'none';
			this.fadeDurationMs = 0;
			this.commit();
		}, this.fadeDurationMs);
		this.commit();
	}

	public destroy(): void {
		this.clearFadeTimer();
		this.listeners.clear();
	}

	private clearFadeTimer(): void {
		if (this.fadeTimer !== null) clearTimeout(this.fadeTimer);
		this.fadeTimer = null;
	}

	private createSnapshot(): AlertIndicatorSnapshot {
		return Object.freeze({
			revision: this.revision,
			alertState: this.alertState,
			showingAlertState: this.showingAlertState,
			fadeKind: this.fadeKind,
			fadeDurationMs: this.fadeDurationMs,
		});
	}

	private commit(): void {
		this.revision += 1;
		this.snapshot = this.createSnapshot();
		this.listeners.forEach((listener) => listener());
	}
}
