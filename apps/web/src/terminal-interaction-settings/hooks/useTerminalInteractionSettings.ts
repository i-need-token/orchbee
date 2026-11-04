import { useMemo, useSyncExternalStore } from 'react';

import {
	DEFAULT_TERMINAL_INTERACTION_PREFERENCES,
	type TerminalInteractionSettingsController,
	type TerminalInteractionSettingsSnapshot,
} from '../model';

const DEFAULT_SNAPSHOT: TerminalInteractionSettingsSnapshot = Object.freeze({
	preferences: DEFAULT_TERMINAL_INTERACTION_PREFERENCES,
});
export function useTerminalInteractionSettings(
	controller?: TerminalInteractionSettingsController | undefined,
): TerminalInteractionSettingsSnapshot {
	const source = useMemo(
		() =>
			controller ?? {
				getSnapshot: (): TerminalInteractionSettingsSnapshot => DEFAULT_SNAPSHOT,
				subscribeSnapshot: (): (() => void) => () => undefined,
			},
		[controller],
	);
	return useSyncExternalStore(source.subscribeSnapshot, source.getSnapshot, source.getSnapshot);
}
