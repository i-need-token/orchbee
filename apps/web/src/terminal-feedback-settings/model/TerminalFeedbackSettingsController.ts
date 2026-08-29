import type { TerminalCommandCompletion } from '@orchbee/terminal-protocol';
import type {
	WorkbenchAgentEventObservation,
	WorkbenchAgentNotificationObserver,
} from '@orchbee/workbench';

import {
	DEFAULT_TERMINAL_FEEDBACK_PREFERENCES,
	TerminalCompletionReminderMode,
	type TerminalFeedbackPreferences,
} from './terminal-feedback-preferences';
import {
	normalizeTerminalFeedbackPreferences,
	readTerminalFeedbackPreferences,
	writeTerminalFeedbackPreferences,
	type TerminalFeedbackPersistenceIssue,
	type TerminalFeedbackStorage,
} from './terminal-feedback-preferences-storage';

export type TerminalNotificationPermission = 'denied' | 'granted' | 'prompt' | 'unsupported';

interface TerminalNotificationRequest {
	readonly body: string;
	readonly onActivate?: (() => void) | undefined;
	readonly tag: string;
	readonly title: string;
}

export interface TerminalNotificationCapability {
	permission(): TerminalNotificationPermission;
	requestPermission(): Promise<TerminalNotificationPermission>;
	show(request: TerminalNotificationRequest): boolean;
}

export interface TerminalFeedbackSettingsSnapshot {
	readonly notificationPermission: TerminalNotificationPermission;
	readonly persistenceIssue: TerminalFeedbackPersistenceIssue | null;
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

type AgentEventObservationResult =
	| {
			readonly kind: 'ignored';
			readonly reason: 'duplicate' | 'turned-off' | 'unsupported' | 'visible';
	  }
	| {
			readonly kind: 'notify';
			readonly notification: 'failed' | 'shown' | 'unavailable';
	  };

export type TerminalFeedbackUpdateResult =
	| { readonly kind: 'applied-and-saved' }
	| { readonly kind: 'applied-session-only' }
	| { readonly kind: 'disposed' }
	| { readonly kind: 'invalid' }
	| { readonly kind: 'unchanged' };

export interface TerminalFeedbackSettingsControllerOptions {
	readonly clock?: () => number;
	readonly notificationCapability: TerminalNotificationCapability;
	readonly storage: TerminalFeedbackStorage;
}

const COMPLETION_FRESHNESS_MILLISECONDS = 60_000;
const COMPLETION_FUTURE_TOLERANCE_MILLISECONDS = 5_000;
const SYSTEM_NOTIFICATION_INTERVAL_MILLISECONDS = 30_000;

type SnapshotListener = () => void;

export class TerminalFeedbackSettingsController
	implements TerminalCompletionReminderObserver, WorkbenchAgentNotificationObserver
{
	readonly #clock: () => number;
	readonly #agentEventKeys: string[] = [];
	readonly #agentEventKeySet = new Set<string>();
	readonly #lastCompletionSequence = new Map<string, bigint>();
	readonly #lastNotificationMilliseconds = new Map<string, number>();
	readonly #notificationCapability: TerminalNotificationCapability;
	readonly #snapshotListeners = new Set<SnapshotListener>();
	readonly #storage: TerminalFeedbackStorage;
	#disposed = false;
	#permissionRequestEpoch = 0;
	#snapshot: TerminalFeedbackSettingsSnapshot;

	constructor({
		clock = Date.now,
		notificationCapability,
		storage,
	}: TerminalFeedbackSettingsControllerOptions) {
		this.#clock = clock;
		this.#notificationCapability = notificationCapability;
		this.#storage = storage;
		const restored = readTerminalFeedbackPreferences(storage);
		this.#snapshot = snapshot(
			safePermission(notificationCapability),
			restored.issue,
			restored.preferences,
		);
	}

	getSnapshot = (): TerminalFeedbackSettingsSnapshot => this.#snapshot;

	subscribe = (listener: SnapshotListener): (() => void) => {
		if (this.#disposed) return () => undefined;
		this.#snapshotListeners.add(listener);
		return () => this.#snapshotListeners.delete(listener);
	};

	update(patch: Partial<TerminalFeedbackPreferences>): TerminalFeedbackUpdateResult {
		if (this.#disposed) return { kind: 'disposed' };
		const candidate = normalizeTerminalFeedbackPreferences({
			...this.#snapshot.preferences,
			...patch,
		});
		if (candidate === null) return { kind: 'invalid' };
		if (
			preferencesEqual(candidate, this.#snapshot.preferences) &&
			this.#snapshot.persistenceIssue === null
		) {
			return { kind: 'unchanged' };
		}
		const saved = writeTerminalFeedbackPreferences(this.#storage, candidate);
		this.#snapshot = snapshot(
			this.#snapshot.notificationPermission,
			saved ? null : 'write-failed',
			candidate,
		);
		this.#emitSnapshot();
		return saved ? { kind: 'applied-and-saved' } : { kind: 'applied-session-only' };
	}

	reset(): TerminalFeedbackUpdateResult {
		return this.update(DEFAULT_TERMINAL_FEEDBACK_PREFERENCES);
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
		this.#snapshot = snapshot(
			permission,
			this.#snapshot.persistenceIssue,
			this.#snapshot.preferences,
		);
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
			shown = this.#notificationCapability.show(notification);
		} catch {
			shown = false;
		}
		return { kind: 'remind', notification: shown ? 'shown' : 'failed' };
	}

	observeAgentEvent({
		event,
		notification,
		visible,
	}: WorkbenchAgentEventObservation): AgentEventObservationResult {
		if (this.#disposed) return { kind: 'ignored', reason: 'turned-off' };
		if (
			event.type !== 'attention-requested' &&
			(event.type !== 'turn-outcome' ||
				(event.outcome !== 'failed' && event.outcome !== 'succeeded'))
		) {
			return { kind: 'ignored', reason: 'unsupported' };
		}
		const key = `${event.sessionId}\0${event.eventId}`;
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
		if (visible) return { kind: 'ignored', reason: 'visible' };
		if (this.#snapshot.notificationPermission !== 'granted') {
			return { kind: 'notify', notification: 'unavailable' };
		}
		let shown = false;
		try {
			shown = this.#notificationCapability.show(notification);
		} catch {
			shown = false;
		}
		return { kind: 'notify', notification: shown ? 'shown' : 'failed' };
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
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
		left.completionReminderMinimumSeconds === right.completionReminderMinimumSeconds &&
		left.completionReminderMode === right.completionReminderMode
	);
}

function snapshot(
	notificationPermission: TerminalNotificationPermission,
	persistenceIssue: TerminalFeedbackPersistenceIssue | null,
	preferences: TerminalFeedbackPreferences,
): TerminalFeedbackSettingsSnapshot {
	return Object.freeze({ notificationPermission, persistenceIssue, preferences });
}
