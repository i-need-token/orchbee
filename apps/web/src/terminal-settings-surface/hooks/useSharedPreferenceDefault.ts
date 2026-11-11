import {
	DEFAULT_USER_PREFERENCES,
	type UserPreferenceKey,
} from '@orchbee/workbench-client-protocol';
import * as React from 'react';

import type { UserPreferencesControl } from '../../user-preferences';
import { preferenceValuesEqual } from '../utils/preferenceValuesEqual';

interface SharedPreferenceDefault<Key extends UserPreferenceKey> {
	/** Whether the confirmed value differs from the product default. */
	readonly modified: boolean;
	readonly restore: () => void;
	/** The confirmed value, or the product default until the first read answers. */
	readonly value: (typeof DEFAULT_USER_PREFERENCES)[Key];
}

/** One shared preference's value, changed state, and restore intent, which resets only that key. */
export function useSharedPreferenceDefault<Key extends UserPreferenceKey>(
	control: UserPreferencesControl,
	key: Key,
): SharedPreferenceDefault<Key> {
	const state = React.useSyncExternalStore(
		control.subscribe,
		control.getSnapshot,
		control.getSnapshot,
	);
	const values = state.snapshot?.values;
	return {
		modified:
			values !== undefined && !preferenceValuesEqual(values[key], DEFAULT_USER_PREFERENCES[key]),
		restore: (): void => {
			void control.reset([key]);
		},
		value: values === undefined ? DEFAULT_USER_PREFERENCES[key] : values[key],
	};
}
