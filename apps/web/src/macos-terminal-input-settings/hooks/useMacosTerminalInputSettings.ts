import { useSyncExternalStore } from 'react';

import type {
	MacosTerminalInputSettingsController,
	MacosTerminalInputSettingsSnapshot,
} from '../model';

export function useMacosTerminalInputSettings(
	controller: MacosTerminalInputSettingsController,
): MacosTerminalInputSettingsSnapshot {
	return useSyncExternalStore(
		controller.subscribeSnapshot,
		controller.getSnapshot,
		controller.getSnapshot,
	);
}
