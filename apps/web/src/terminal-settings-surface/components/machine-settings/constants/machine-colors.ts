import { terminalSettingsMessages as messages } from '@orchbee/i18n';

import type { MachineColorPreset } from '../../../../machine-color';

/** Each machine color preset's name, in the catalog's order. */
export const MACHINE_COLOR_NAMES = [
	['cyan', messages.pageMachinesColorCyan],
	['teal', messages.pageMachinesColorTeal],
	['sky', messages.pageMachinesColorSky],
	['blue', messages.pageMachinesColorBlue],
	['indigo', messages.pageMachinesColorIndigo],
	['violet', messages.pageMachinesColorViolet],
	['purple', messages.pageMachinesColorPurple],
	['magenta', messages.pageMachinesColorMagenta],
	['pink', messages.pageMachinesColorPink],
	['brown', messages.pageMachinesColorBrown],
	['slate', messages.pageMachinesColorSlate],
	['grey', messages.pageMachinesColorGrey],
] as const satisfies readonly (readonly [MachineColorPreset, unknown])[];
