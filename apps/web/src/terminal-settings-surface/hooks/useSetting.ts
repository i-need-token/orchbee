import { useOrchbeeIntl } from '@orchbee/i18n';
import type { SettingKey, SettingValues } from '@orchbee/workbench-client-protocol';

import {
	isSettingSet,
	isSettingsFileReadOnly,
	readSetting,
	settingFailure,
	type UserSettingsControl,
	type UserSettingsFailure,
} from '../../user-settings';
import { settingsFailureMessage } from '../utils/settings-failure-message';
import { useUserSettings } from './useUserSettings';

export interface SettingView<Key extends SettingKey> {
	/** Why the last change of this key did not land, for the row to show; null when it did. */
	readonly failure: UserSettingsFailure | null;
	readonly failureMessage: string | null;
	/** Whether the file sets this key, which is what the changed-setting dot shows. */
	readonly modified: boolean;
	/** The file cannot be edited now, so the row shows its value without a working control. */
	readonly readOnly: boolean;
	/** The file's value, or the key's default until the first snapshot arrives. */
	readonly value: SettingValues[Key];
	readonly restore: () => void;
	readonly set: (value: SettingValues[Key]) => void;
}

/** One key of the settings file: its value, changed state, last failure, and edits of that key. */
export function useSetting<Key extends SettingKey>(
	control: UserSettingsControl,
	key: Key,
): SettingView<Key> {
	const intl = useOrchbeeIntl();
	const state = useUserSettings(control);
	const failure = settingFailure(state, key);
	return {
		failure,
		failureMessage: failure === null ? null : intl.formatMessage(settingsFailureMessage(failure)),
		modified: isSettingSet(state.snapshot, key),
		readOnly: isSettingsFileReadOnly(state.snapshot),
		restore: (): void => {
			void control.restore([key]);
		},
		set: (value): void => {
			void control.set(key, value);
		},
		value: readSetting(state.snapshot, key),
	};
}
