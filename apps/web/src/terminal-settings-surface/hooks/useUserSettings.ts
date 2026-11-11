import * as React from 'react';

import type { UserSettingsControl, UserSettingsState } from '../../user-settings';

/** The settings file as the control last reported it. */
export function useUserSettings(control: UserSettingsControl): UserSettingsState {
	return React.useSyncExternalStore(control.subscribe, control.getSnapshot, control.getSnapshot);
}
