import {
	DEFAULT_USER_PREFERENCES,
	type UserPreferencesPatch,
	type UserPreferencesResponse,
	type UserPreferencesSnapshot,
} from '@orchbee/workbench-client-protocol';

import type { UserPreferencesControl, UserPreferencesState } from '../user-preferences';

type UserPreferencesValues = UserPreferencesSnapshot['values'];

/**
 * The Session's shared preferences held in memory. As through the Server, a save reports itself
 * pending at once and its confirmed snapshot arrives a microtask later, before the save settles. A
 * null resets its key.
 */
export function memoryUserPreferences(
	values: Partial<UserPreferencesValues> = {},
): UserPreferencesControl {
	const listeners = new Set<() => void>();
	let revision = 0;
	let state: UserPreferencesState = {
		applicationIssue: null,
		available: true,
		draftResetRevision: 0,
		failure: null,
		pending: false,
		snapshot: {
			claimed: [],
			source: 'missing',
			values: { ...DEFAULT_USER_PREFERENCES, ...values },
		},
	};
	const publish = (next: UserPreferencesState): void => {
		state = next;
		for (const listener of new Set(listeners)) listener();
	};
	const save = async (changes: UserPreferencesPatch): Promise<UserPreferencesResponse> => {
		publish({ ...state, pending: true });
		await Promise.resolve();
		revision += 1;
		const snapshot: UserPreferencesSnapshot = {
			claimed: [],
			source: revision.toString(16).padStart(64, '0'),
			values: applyPatch(state.snapshot?.values ?? DEFAULT_USER_PREFERENCES, changes),
		};
		publish({ ...state, pending: false, snapshot });
		return { changed: true, kind: 'snapshot', snapshot };
	};
	return {
		getSnapshot: () => state,
		refresh: () => {
			const snapshot = state.snapshot;
			if (snapshot === null) return Promise.resolve({ kind: 'refused', reason: 'unavailable' });
			publish({ ...state, draftResetRevision: state.draftResetRevision + 1 });
			return Promise.resolve({ changed: false, kind: 'snapshot', snapshot });
		},
		reset: (keys) => save(Object.fromEntries(keys.map((key) => [key, null]))),
		subscribe: (listener) => {
			listeners.add(listener);
			return () => listeners.delete(listener);
		},
		update: (changes) => save(changes),
	};
}

function applyPatch(
	current: UserPreferencesValues,
	changes: UserPreferencesPatch,
): UserPreferencesValues {
	return {
		locale: choose(changes.locale, current.locale, DEFAULT_USER_PREFERENCES.locale),
		applicationTheme: choose(
			changes.applicationTheme,
			current.applicationTheme,
			DEFAULT_USER_PREFERENCES.applicationTheme,
		),
		lightTheme: choose(changes.lightTheme, current.lightTheme, DEFAULT_USER_PREFERENCES.lightTheme),
		darkTheme: choose(changes.darkTheme, current.darkTheme, DEFAULT_USER_PREFERENCES.darkTheme),
		terminalDisplayTheme: choose(
			changes.terminalDisplayTheme,
			current.terminalDisplayTheme,
			DEFAULT_USER_PREFERENCES.terminalDisplayTheme,
		),
		customForeground: choose(
			changes.customForeground,
			current.customForeground,
			DEFAULT_USER_PREFERENCES.customForeground,
		),
		customBackground: choose(
			changes.customBackground,
			current.customBackground,
			DEFAULT_USER_PREFERENCES.customBackground,
		),
		copyOnSelect: choose(
			changes.copyOnSelect,
			current.copyOnSelect,
			DEFAULT_USER_PREFERENCES.copyOnSelect,
		),
		trimTrailingBlankRowsForAgents: choose(
			changes.trimTrailingBlankRowsForAgents,
			current.trimTrailingBlankRowsForAgents,
			DEFAULT_USER_PREFERENCES.trimTrailingBlankRowsForAgents,
		),
		agentNotificationMode: choose(
			changes.agentNotificationMode,
			current.agentNotificationMode,
			DEFAULT_USER_PREFERENCES.agentNotificationMode,
		),
		completionReminderMode: choose(
			changes.completionReminderMode,
			current.completionReminderMode,
			DEFAULT_USER_PREFERENCES.completionReminderMode,
		),
		completionReminderMinimumSeconds: choose(
			changes.completionReminderMinimumSeconds,
			current.completionReminderMinimumSeconds,
			DEFAULT_USER_PREFERENCES.completionReminderMinimumSeconds,
		),
		soundEnabled: choose(
			changes.soundEnabled,
			current.soundEnabled,
			DEFAULT_USER_PREFERENCES.soundEnabled,
		),
		scrollbackBytes: choose(
			changes.scrollbackBytes,
			current.scrollbackBytes,
			DEFAULT_USER_PREFERENCES.scrollbackBytes,
		),
		launchProfiles: choose(
			changes.launchProfiles,
			current.launchProfiles,
			DEFAULT_USER_PREFERENCES.launchProfiles,
		),
	};
}

/** Absence keeps the current value and null restores the common default. */
function choose<T>(change: T | null | undefined, current: T, fallback: T): T {
	if (change === undefined) return current;
	return change ?? fallback;
}
