import * as React from 'react';

import type {
	TerminalSettingsMachines,
	TerminalSettingsMachinesView,
} from '../types/terminal-settings-surface';

const subscribeNothing = (): (() => void) => () => undefined;
const nothing = (): null => null;

/** The machines' last report, or null without one. */
export function useTerminalSettingsMachines(
	machines: TerminalSettingsMachines | undefined,
): TerminalSettingsMachinesView | null {
	return React.useSyncExternalStore(
		machines?.subscribe ?? subscribeNothing,
		machines?.getSnapshot ?? nothing,
		machines?.getSnapshot ?? nothing,
	);
}
