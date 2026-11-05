import type { DecodedTerminalFrame } from '@orchbee/terminal-protocol';
import type { ITheme } from '../types';
import type { TerminalPresentationConfiguration } from '../presentation-configuration';

export const DEFAULT_THEME: Required<ITheme> = {
	foreground: '#d4d4d4',
	background: '#1e1e1e',
	cursor: '#ffffff',
	cursorAccent: '#1e1e1e',
	selectionBackground: '#415576',
	selectionForeground: '#d4d4d4',
	searchMatchBackground: '#515c6a',
	searchMatchForeground: '#ffffff',
	searchActiveMatchBackground: '#f5a97f',
	searchActiveMatchForeground: '#1e1e1e',
	black: '#000000',
	red: '#cd3131',
	green: '#0dbc79',
	yellow: '#e5e510',
	blue: '#2472c8',
	magenta: '#bc3fbc',
	cyan: '#11a8cd',
	white: '#e5e5e5',
	brightBlack: '#666666',
	brightRed: '#f14c4c',
	brightGreen: '#23d18b',
	brightYellow: '#f5f543',
	brightBlue: '#3b8eea',
	brightMagenta: '#d670d6',
	brightCyan: '#29b8db',
	brightWhite: '#ffffff',
};

export function hexToRGB(color: string): [number, number, number] {
	const hex = color.startsWith('#') ? color.slice(1) : color;
	if (/^[\da-fA-F]{3}$/.test(hex)) {
		return [
			Number.parseInt(hex.charAt(0).repeat(2), 16),
			Number.parseInt(hex.charAt(1).repeat(2), 16),
			Number.parseInt(hex.charAt(2).repeat(2), 16),
		];
	}
	if (/^[\da-fA-F]{6}(?:[\da-fA-F]{2})?$/.test(hex)) {
		return [
			Number.parseInt(hex.slice(0, 2), 16),
			Number.parseInt(hex.slice(2, 4), 16),
			Number.parseInt(hex.slice(4, 6), 16),
		];
	}
	const rgb = color.match(/^rgba?\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)/);
	if (rgb) {
		return [
			Math.min(255, Number(rgb[1])),
			Math.min(255, Number(rgb[2])),
			Math.min(255, Number(rgb[3])),
		];
	}
	throw new Error(`Unsupported renderer color: ${color}`);
}

export function rgbChannels(value: number): [number, number, number] {
	return [(value >>> 16) & 0xff, (value >>> 8) & 0xff, value & 0xff];
}

export interface PackedPresentationColors {
	readonly ansi: readonly number[];
	readonly background: readonly [number, number, number];
	readonly cursor: readonly [number, number, number];
	readonly cursorAccent: readonly [number, number, number];
	readonly foreground: readonly [number, number, number];
	readonly searchActiveBackground: readonly [number, number, number];
	readonly searchActiveForeground: readonly [number, number, number];
	readonly searchBackground: readonly [number, number, number];
	readonly searchForeground: readonly [number, number, number];
	readonly selectionBackground: readonly [number, number, number];
	readonly selectionForeground: readonly [number, number, number];
}

export function resolvePackedPresentationColors(
	defaultColors: DecodedTerminalFrame['defaultColors'],
	theme: Required<ITheme>,
	cursorAccentOverride?: string,
	defaultColorOverride: TerminalPresentationConfiguration['defaultColorOverride'] = null,
): PackedPresentationColors {
	const hostBackground = rgbChannels(defaultColors.backgroundRgb);
	const background =
		defaultColorOverride === null ? hostBackground : hexToRGB(defaultColorOverride.background);
	const foreground =
		defaultColorOverride === null
			? rgbChannels(defaultColors.foregroundRgb)
			: hexToRGB(defaultColorOverride.foreground);
	const cursorAccent = defaultColorOverride?.cursorAccent ?? cursorAccentOverride;
	return {
		ansi:
			defaultColorOverride?.ansi ??
			[
				theme.black,
				theme.red,
				theme.green,
				theme.yellow,
				theme.blue,
				theme.magenta,
				theme.cyan,
				theme.white,
				theme.brightBlack,
				theme.brightRed,
				theme.brightGreen,
				theme.brightYellow,
				theme.brightBlue,
				theme.brightMagenta,
				theme.brightCyan,
				theme.brightWhite,
			].map((color) => {
				const [red, green, blue] = hexToRGB(color);
				return (red << 16) | (green << 8) | blue;
			}),
		background,
		cursor:
			defaultColorOverride?.cursor === undefined
				? rgbChannels(defaultColors.cursorRgb ?? defaultColors.foregroundRgb)
				: hexToRGB(defaultColorOverride.cursor),
		cursorAccent: cursorAccent === undefined ? hostBackground : hexToRGB(cursorAccent),
		foreground,
		searchActiveBackground: hexToRGB(theme.searchActiveMatchBackground),
		searchActiveForeground: hexToRGB(theme.searchActiveMatchForeground),
		searchBackground: hexToRGB(theme.searchMatchBackground),
		searchForeground: hexToRGB(theme.searchMatchForeground),
		selectionBackground: hexToRGB(theme.selectionBackground),
		selectionForeground: hexToRGB(theme.selectionForeground),
	};
}
