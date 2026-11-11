import type { TerminalCommandCompletion } from '@orchbee/terminal-protocol';
import type { SettingChange } from '@orchbee/workbench-client-protocol';
import { readSetting, type UserSettingsControl } from '../../user-settings';
import type {
	WorkbenchAgentNotification,
	WorkbenchAgentNotificationObserver,
} from '../../workbench';

import {
	TerminalCompletionReminderMode,
	type TerminalFeedbackPreferences,
} from './terminal-feedback-preferences';

export type TerminalNotificationPermission = 'denied' | 'granted' | 'prompt' | 'unsupported';

interface TerminalNotificationRequest {
	readonly body: string;
	readonly onActivate?: (() => void) | undefined;
	readonly tag: string;
	readonly title: string;
	readonly silent?: boolean;
}

export interface TerminalNotificationCapability {
	permission(): TerminalNotificationPermission;
	requestPermission(): Promise<TerminalNotificationPermission>;
	show(request: TerminalNotificationRequest): boolean;
}

export interface TerminalFeedbackSettingsSnapshot {
	readonly notificationPermission: TerminalNotificationPermission;
	readonly preferences: TerminalFeedbackPreferences;
}

export interface TerminalCompletionObservation {
	readonly completion: TerminalCommandCompletion;
	readonly focused: boolean;
	readonly notification: TerminalNotificationRequest;
	readonly terminalIdentity: string;
}

export interface TerminalCompletionReminderObserver {
	observeCompletion(
		observation: TerminalCompletionObservation,
	): TerminalCompletionObservationResult;
}

export type TerminalCompletionObservationResult =
	| {
			readonly kind: 'ignored';
			readonly reason: 'duplicate' | 'focused' | 'future' | 'short' | 'stale' | 'turned-off';
	  }
	| {
			readonly kind: 'remind';
			readonly notification: 'failed' | 'rate-limited' | 'shown' | 'unavailable';
	  };

type AgentNotificationResult =
	| {
			readonly kind: 'ignored';
			readonly reason: 'duplicate' | 'turned-off';
	  }
	| {
			readonly kind: 'notify';
			readonly notification: 'failed' | 'shown' | 'unavailable';
	  };

export type TerminalFeedbackUpdateResult =
	| { readonly kind: 'disposed' }
	| { readonly kind: 'pending' };

export interface TerminalFeedbackSettingsControllerOptions {
	readonly clock?: () => number;
	readonly notificationCapability: TerminalNotificationCapability;
	readonly userSettings: UserSettingsControl;
}

const COMPLETION_FRESHNESS_MILLISECONDS = 60_000;
const COMPLETION_FUTURE_TOLERANCE_MILLISECONDS = 5_000;
const SYSTEM_NOTIFICATION_INTERVAL_MILLISECONDS = 30_000;

type SnapshotListener = () => void;

export class TerminalFeedbackSettingsController
	implements TerminalCompletionReminderObserver, WorkbenchAgentNotificationObserver
{
	get userSettings(): UserSettingsControl {
		return this.#userSettings;
	}
	readonly #clock: () => number;
	readonly #agentEventKeys: string[] = [];
	readonly #agentEventKeySet = new Set<string>();
	readonly #lastCompletionSequence = new Map<string, bigint>();
	readonly #lastNotificationMilliseconds = new Map<string, number>();
	readonly #notificationCapability: TerminalNotificationCapability;
	readonly #snapshotListeners = new Set<SnapshotListener>();
	readonly #userSettings: UserSettingsControl;
	readonly #removeUserSettings: () => void;
	#disposed = false;
	#permissionRequestEpoch = 0;
	#snapshot: TerminalFeedbackSettingsSnapshot;

	constructor({
		clock = Date.now,
		notificationCapability,
		userSettings,
	}: TerminalFeedbackSettingsControllerOptions) {
		this.#clock = clock;
		this.#notificationCapability = notificationCapability;
		this.#userSettings = userSettings;
		this.#snapshot = snapshot(
			safePermission(notificationCapability),
			feedbackPreferences(userSettings),
		);
		this.#removeUserSettings = userSettings.subscribe(() => {
			const preferences = feedbackPreferences(userSettings);
			if (preferencesEqual(this.#snapshot.preferences, preferences)) return;
			this.#snapshot = snapshot(this.#snapshot.notificationPermission, preferences);
			this.#emitSnapshot();
		});
	}

	getSnapshot = (): TerminalFeedbackSettingsSnapshot => this.#snapshot;

	subscribe = (listener: SnapshotListener): (() => void) => {
		if (this.#disposed) return () => undefined;
		this.#snapshotListeners.add(listener);
		return () => this.#snapshotListeners.delete(listener);
	};

	update(patch: Partial<TerminalFeedbackPreferences>): TerminalFeedbackUpdateResult {
		if (this.#disposed) return { kind: 'disposed' };
		void this.#userSettings.edit(feedbackChanges(patch));
		return { kind: 'pending' };
	}

	reset(): TerminalFeedbackUpdateResult {
		if (this.#disposed) return { kind: 'disposed' };
		void this.#userSettings.restore([
			'notifications.agent_reminder',
			'notifications.completion_reminder',
			'notifications.completion_minimum_seconds',
			'notifications.sound',
		]);
		return { kind: 'pending' };
	}

	async requestNotificationPermission(): Promise<TerminalNotificationPermission> {
		if (this.#disposed) return this.#snapshot.notificationPermission;
		if (this.#snapshot.notificationPermission !== 'prompt') {
			return this.#snapshot.notificationPermission;
		}
		const epoch = ++this.#permissionRequestEpoch;
		let permission: TerminalNotificationPermission;
		try {
			permission = await this.#notificationCapability.requestPermission();
		} catch {
			permission = safePermission(this.#notificationCapability);
		}
		if (this.#disposed || epoch !== this.#permissionRequestEpoch) {
			return this.#snapshot.notificationPermission;
		}
		this.#snapshot = snapshot(permission, this.#snapshot.preferences);
		this.#emitSnapshot();
		return permission;
	}

	observeCompletion({
		completion,
		focused,
		notification,
		terminalIdentity,
	}: TerminalCompletionObservation): TerminalCompletionObservationResult {
		if (this.#disposed) return { kind: 'ignored', reason: 'turned-off' };
		const previousSequence = this.#lastCompletionSequence.get(terminalIdentity);
		if (previousSequence !== undefined && completion.sequence <= previousSequence) {
			return { kind: 'ignored', reason: 'duplicate' };
		}
		this.#lastCompletionSequence.set(terminalIdentity, completion.sequence);
		const now = this.#clock();
		const completedAt = Number(completion.completedAtMilliseconds);
		if (
			!Number.isSafeInteger(completedAt) ||
			completedAt > now + COMPLETION_FUTURE_TOLERANCE_MILLISECONDS
		) {
			return { kind: 'ignored', reason: 'future' };
		}
		if (now - completedAt > COMPLETION_FRESHNESS_MILLISECONDS) {
			return { kind: 'ignored', reason: 'stale' };
		}
		const preferences = this.#snapshot.preferences;
		if (preferences.completionReminderMode === TerminalCompletionReminderMode.never) {
			return { kind: 'ignored', reason: 'turned-off' };
		}
		if (
			completion.durationMilliseconds < BigInt(preferences.completionReminderMinimumSeconds * 1000)
		) {
			return { kind: 'ignored', reason: 'short' };
		}
		if (
			preferences.completionReminderMode === TerminalCompletionReminderMode.unfocused &&
			focused
		) {
			return { kind: 'ignored', reason: 'focused' };
		}
		if (this.#snapshot.notificationPermission !== 'granted') {
			return { kind: 'remind', notification: 'unavailable' };
		}
		const lastNotification = this.#lastNotificationMilliseconds.get(terminalIdentity);
		if (
			lastNotification !== undefined &&
			now - lastNotification < SYSTEM_NOTIFICATION_INTERVAL_MILLISECONDS
		) {
			return { kind: 'remind', notification: 'rate-limited' };
		}
		this.#lastNotificationMilliseconds.set(terminalIdentity, now);
		let shown = false;
		try {
			shown = this.#notificationCapability.show({
				...notification,
				silent: !preferences.soundEnabled,
			});
		} catch {
			shown = false;
		}
		return { kind: 'remind', notification: shown ? 'shown' : 'failed' };
	}

	/** The Workbench already withheld states the person saw; this owns preference and permission. */
	observeAgentNotification({
		key,
		notification,
	}: WorkbenchAgentNotification): AgentNotificationResult {
		if (this.#disposed) return { kind: 'ignored', reason: 'turned-off' };
		// Delivery stays idempotent per exact state, however many Workbench views observe it.
		if (this.#agentEventKeySet.has(key)) return { kind: 'ignored', reason: 'duplicate' };
		this.#agentEventKeySet.add(key);
		this.#agentEventKeys.push(key);
		if (this.#agentEventKeys.length > 2048) {
			const retired = this.#agentEventKeys.shift();
			if (retired !== undefined) this.#agentEventKeySet.delete(retired);
		}
		if (this.#snapshot.preferences.agentNotificationMode === 'never') {
			return { kind: 'ignored', reason: 'turned-off' };
		}
		if (this.#snapshot.notificationPermission !== 'granted') {
			return { kind: 'notify', notification: 'unavailable' };
		}
		let shown = false;
		try {
			shown = this.#notificationCapability.show({
				...notification,
				silent: !this.#snapshot.preferences.soundEnabled,
			});
		} catch {
			shown = false;
		}
		return { kind: 'notify', notification: shown ? 'shown' : 'failed' };
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#removeUserSettings();
		this.#permissionRequestEpoch += 1;
		this.#lastCompletionSequence.clear();
		this.#lastNotificationMilliseconds.clear();
		this.#agentEventKeys.length = 0;
		this.#agentEventKeySet.clear();
		this.#snapshotListeners.clear();
	}

	#emitSnapshot(): void {
		for (const listener of new Set(this.#snapshotListeners)) {
			if (this.#disposed) break;
			try {
				listener();
			} catch {
				// External-store listeners are isolated from each other.
			}
		}
	}
}

function safePermission(
	capability: TerminalNotificationCapability,
): TerminalNotificationPermission {
	try {
		return capability.permission();
	} catch {
		return 'unsupported';
	}
}

function preferencesEqual(
	left: TerminalFeedbackPreferences,
	right: TerminalFeedbackPreferences,
): boolean {
	return (
		left.agentNotificationMode === right.agentNotificationMode &&
		left.soundEnabled === right.soundEnabled &&
		left.completionReminderMinimumSeconds === right.completionReminderMinimumSeconds &&
		left.completionReminderMode === right.completionReminderMode
	);
}

function snapshot(
	notificationPermission: TerminalNotificationPermission,
	preferences: TerminalFeedbackPreferences,
): TerminalFeedbackSettingsSnapshot {
	return Object.freeze({ notificationPermission, preferences });
}

function feedbackPreferences(control: UserSettingsControl): TerminalFeedbackPreferences {
	const settings = control.getSnapshot().snapshot;
	return {
		agentNotificationMode: readSetting(settings, 'notifications.agent_reminder'),
		completionReminderMinimumSeconds: readSetting(
			settings,
			'notifications.completion_minimum_seconds',
		),
		completionReminderMode: readSetting(settings, 'notifications.completion_reminder'),
		soundEnabled: readSetting(settings, 'notifications.sound'),
	};
}

function feedbackChanges(patch: Partial<TerminalFeedbackPreferences>): SettingChange[] {
	const changes: SettingChange[] = [];
	if (patch.agentNotificationMode !== undefined) {
		changes.push({ key: 'notifications.agent_reminder', value: patch.agentNotificationMode });
	}
	if (patch.completionReminderMode !== undefined) {
		changes.push({ key: 'notifications.completion_reminder', value: patch.completionReminderMode });
	}
	if (patch.completionReminderMinimumSeconds !== undefined) {
		changes.push({
			key: 'notifications.completion_minimum_seconds',
			value: patch.completionReminderMinimumSeconds,
		});
	}
	if (patch.soundEnabled !== undefined) {
		changes.push({ key: 'notifications.sound', value: patch.soundEnabled });
	}
	return changes;
}
