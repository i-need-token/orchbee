import { useSyncExternalStore } from 'react';

import type { TerminalSettingsController, TerminalSettingsSnapshot } from '../model';

export function useTerminalSettings(
	controller: TerminalSettingsController,
): TerminalSettingsSnapshot {
	return useSyncExternalStore(
		controller.subscribeSnapshot,
		controller.getSnapshot,
		controller.getSnapshot,
	);
}
