import { z } from 'zod';

/**
 * The machine color presets in the catalog's order, as the record and every command spell them.
 * Each theme makes them readable through its `--ob-sys-color-machine-*` tokens.
 */
export const MACHINE_COLOR_PRESETS = [
	'cyan',
	'teal',
	'sky',
	'blue',
	'indigo',
	'violet',
	'purple',
	'magenta',
	'pink',
	'brown',
	'slate',
	'grey',
] as const;

export type MachineColorPreset = (typeof MACHINE_COLOR_PRESETS)[number];

/** A color of the person's own, `#rrggbb` in lowercase, shown as chosen. */
type MachineCustomColor = `#${string}`;

/** A saved machine's color: a preset or a color of the person's own. */
export type MachineColor = MachineColorPreset | MachineCustomColor;

const CUSTOM_COLOR = /^#[\da-f]{6}$/u;

function isMachineCustomColor(color: string): color is MachineCustomColor {
	return CUSTOM_COLOR.test(color);
}

export const MachineColorSchema = z.union([
	z.enum(MACHINE_COLOR_PRESETS),
	z.custom<MachineCustomColor>((value) => typeof value === 'string' && isMachineCustomColor(value)),
]);

/** Typed `#` and six hexadecimal digits in either case, around blanks; `null` for anything else. */
export function parseMachineCustomColor(text: string): MachineCustomColor | null {
	const color = text.trim().toLowerCase();
	return isMachineCustomColor(color) ? color : null;
}

/**
 * The `--ob-comp-machine-color` value a machine's name, mark or dot is drawn in: a preset's theme
 * token, or the custom color itself.
 */
export function machineColorValue(color: MachineColor): string {
	return isMachineCustomColor(color) ? color : `var(--ob-sys-color-machine-${color})`;
}
