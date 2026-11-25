import { terminalSettingsMessages as messages } from '@orchbee/i18n';

/** A machine's colors in the catalog's palette order, each with its name. */
export const MACHINE_COLORS = [
	['cyan', messages.pageMachinesColorCyan],
	['purple', messages.pageMachinesColorPurple],
	['pink', messages.pageMachinesColorPink],
	['grey', messages.pageMachinesColorGrey],
] as const;
