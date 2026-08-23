import { TERMINAL_FONT_WEIGHT_OPTIONS, type TerminalFontWeight } from '../../terminal-settings';

export function parseTerminalFontWeight(value: string): TerminalFontWeight | null {
	const parsed = Number(value);
	return TERMINAL_FONT_WEIGHT_OPTIONS.find((weight) => weight === parsed) ?? null;
}

export function parseTerminalPadding(value: string): number | null {
	if (!/^(?:0|[1-9][0-9]*)$/u.test(value)) return null;
	const parsed = Number(value);
	return Number.isSafeInteger(parsed) && parsed <= 32 ? parsed : null;
}

export function parseScrollbackBytes(value: string): number | null {
	if (!/^(?:0|[1-9][0-9]*)$/u.test(value)) return null;
	const parsed = Number(value);
	return Number.isSafeInteger(parsed) && parsed <= 1_000_000_000 ? parsed : null;
}
