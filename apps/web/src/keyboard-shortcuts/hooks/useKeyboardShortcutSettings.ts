import { useSyncExternalStore } from 'react';

import type {
	KeyboardShortcutSettingsController,
	KeyboardShortcutSettingsSnapshot,
} from '../model';

export function useKeyboardShortcutSettings(
	controller: KeyboardShortcutSettingsController,
): KeyboardShortcutSettingsSnapshot {
	return useSyncExternalStore(controller.subscribe, controller.getSnapshot, controller.getSnapshot);
}
