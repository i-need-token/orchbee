import {
	orchbeeTerminalColorTheme,
	terminalColorThemeAnsi,
	type TerminalColorTheme,
} from '@orchbee/terminal';

// Build time only: the page carries a theme as data, and applyTerminalTheme applies it in the
// browser. Activations never import this module, which would put the product's terminal package
// into the published script (the page generator's script budget fails such a build).

// The roles a theme file does not name are mixed from its foreground into its background, with
// shares that reproduce the default tokens from Orchbee Dark exactly: dimmed text, the input
// row's band, the terminal interface's frame lines and its selected row. Bold text moves the
// foreground away from the background, toward white on a dark theme and black on a light one.
const DIM_SHARE = 0.57;
const INPUT_SHARE = 0.065;
const LINE_SHARE = 0.265;
const ROW_SELECTION_SHARE = 0.08;
const BOLD_SHARE = 0.7;
const WHITE = 0xff_ff_ff;
const BLACK = 0x00_00_00;

function channel(color: number, shift: number): number {
	return (color >> shift) & 0xff;
}

/** `share` of the way from `from` to `to`, per channel in sRGB. */
function mix(from: number, to: number, share: number): number {
	let mixed = 0;
	for (const shift of [16, 8, 0]) {
		const start = channel(from, shift);
		mixed |= Math.round(start + (channel(to, shift) - start) * share) << shift;
	}
	return mixed;
}

function hex(color: number): string {
	return `#${color.toString(16).padStart(6, '0')}`;
}

function palette(theme: TerminalColorTheme): string {
	const { background, foreground } = theme;
	const colors = new Map<string, number>([
		['background', background],
		['foreground', foreground],
		['cursor', theme.cursor],
		['selection-background', theme.selectionBackground],
		['selection-foreground', theme.selectionForeground],
		['bold', mix(foreground, theme.polarity === 'dark' ? WHITE : BLACK, BOLD_SHARE)],
		['dim', mix(background, foreground, DIM_SHARE)],
		['input', mix(background, foreground, INPUT_SHARE)],
		['line', mix(background, foreground, LINE_SHARE)],
		['row-selection', mix(background, foreground, ROW_SELECTION_SHARE)],
	]);
	for (const [index, color] of terminalColorThemeAnsi(theme).entries())
		colors.set(`ansi-${index}`, color);
	return [...colors].map(([name, color]) => `${name}:${hex(color)}`).join(';');
}

/**
 * The page data of one theme: write `polarity` as the element's `data-terminal-theme` and `colors`
 * as its `data-terminal-colors`, and applyTerminalTheme reads them from that element.
 */
export interface TerminalThemeData {
	readonly polarity: TerminalColorTheme['polarity'];
	readonly colors: string;
}

/**
 * The page data of `theme`, one of the product's `terminalColorThemes`. The default theme, Orchbee
 * Dark, carries no colors, because the stylesheet already holds them.
 */
export function terminalThemeData(theme: TerminalColorTheme): TerminalThemeData {
	const isDefault = theme.id === orchbeeTerminalColorTheme('dark').id;
	return { polarity: theme.polarity, colors: isDefault ? '' : palette(theme) };
}
