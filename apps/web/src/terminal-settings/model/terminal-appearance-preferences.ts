import type { TerminalColorTheme, TerminalPresentationConfiguration } from '@orchbee/terminal';
import { z } from 'zod';

import {
	DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION_SOURCE,
	type TerminalPresentationConfigurationSource,
} from '../../terminal-presentation';

type ApplicationTheme = 'dark' | 'light' | 'system';
type TerminalDisplayTheme = 'custom' | 'host' | 'theme';
export type TerminalHexColor = `#${string}`;
/** @internal Exported for the font-size bounds test. */
export const MINIMUM_TERMINAL_FONT_SIZE = 8;
/** @internal Exported for the font-size bounds test. */
export const MAXIMUM_TERMINAL_FONT_SIZE = 48;
const TERMINAL_FONT_SIZE_STEP = 2;
export const TERMINAL_FONT_WEIGHT_OPTIONS = [100, 200, 300, 400, 500, 600, 700, 800, 900] as const;
type TerminalFontWeight = (typeof TERMINAL_FONT_WEIGHT_OPTIONS)[number];
export type TerminalFontSizeChange = 'decrease' | 'increase' | 'reset';

export interface TerminalAppearancePreferences {
	readonly applicationTheme: ApplicationTheme;
	readonly backgroundOpacityPercent: number;
	readonly customBackground: TerminalHexColor;
	readonly customForeground: TerminalHexColor;
	/** The dark slot's stored color-theme id; an id this build does not ship resolves to Orchbee Dark. */
	readonly darkTheme: string;
	readonly fontFamily: string;
	readonly fontLigatures: boolean;
	readonly fontSize: number;
	readonly fontWeight: TerminalFontWeight;
	readonly fontWeightBold: TerminalFontWeight;
	/** The light slot's stored color-theme id, resolved like `darkTheme`. */
	readonly lightTheme: string;
	readonly terminalPadding: number;
	readonly terminalDisplayTheme: TerminalDisplayTheme;
}

export type TerminalResolvedThemePreferences = Omit<
	TerminalAppearancePreferences,
	'applicationTheme'
> & {
	readonly applicationTheme: 'dark' | 'light';
	/** The active slot's theme, resolved once by the controller; its polarity is `applicationTheme`. */
	readonly colorTheme: TerminalColorTheme;
};

const productPresentation = DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION_SOURCE.current();

export const DEFAULT_TERMINAL_APPEARANCE_PREFERENCES: TerminalAppearancePreferences = Object.freeze(
	{
		applicationTheme: 'system',
		backgroundOpacityPercent: 100,
		customBackground: '#fcfcfc',
		customForeground: '#141414',
		darkTheme: 'orchbee-dark',
		fontFamily: productPresentation.fontFamily,
		fontLigatures: true,
		fontSize: productPresentation.fontSize,
		fontWeight: 400,
		fontWeightBold: 700,
		lightTheme: 'orchbee-light',
		terminalPadding: 0,
		terminalDisplayTheme: 'theme',
	},
);

const hexColorSchema = z
	.string()
	.regex(/^#[\da-fA-F]{6}$/u)
	.transform((value): TerminalHexColor => `#${value.slice(1).toLowerCase()}`);

const colorThemeIdSchema = z
	.string()
	.max(64)
	.regex(/^[\da-z]+(?:-[\da-z]+)*$/u);

const terminalFontWeightSchema = z.union(
	TERMINAL_FONT_WEIGHT_OPTIONS.map((weight) => z.literal(weight)),
);
const terminalAppearancePreferencesSchema = z.strictObject({
	applicationTheme: z.enum(['light', 'dark', 'system']),
	backgroundOpacityPercent: z.number().int().min(10).max(100),
	customBackground: hexColorSchema,
	customForeground: hexColorSchema,
	darkTheme: colorThemeIdSchema,
	fontFamily: z.string().trim().min(1).max(256),
	fontSize: z.number().int().min(MINIMUM_TERMINAL_FONT_SIZE).max(MAXIMUM_TERMINAL_FONT_SIZE),
	terminalDisplayTheme: z.enum(['theme', 'host', 'custom']),
	fontLigatures: z.boolean(),
	fontWeight: terminalFontWeightSchema,
	fontWeightBold: terminalFontWeightSchema,
	lightTheme: colorThemeIdSchema,
	terminalPadding: z.number().int().min(0).max(32),
});

/** The appearance `input` describes when every field is in range, normalized; otherwise null. */
export function normalizeTerminalAppearancePreferences(
	input: unknown,
): TerminalAppearancePreferences | null {
	const result = terminalAppearancePreferencesSchema.safeParse(input);
	return result.success ? Object.freeze(result.data) : null;
}
export function terminalPresentationConfigurationFromPreferences(
	preferences: TerminalAppearancePreferences,
): TerminalPresentationConfiguration {
	return Object.freeze({
		...productPresentation,
		allowTransparency: preferences.backgroundOpacityPercent < 100,
		backgroundOpacity: preferences.backgroundOpacityPercent / 100,
		defaultColorOverride: null,
		fontFamily: preferences.fontFamily,
		fontLigatures: preferences.fontLigatures,
		fontSize: preferences.fontSize,
		fontWeight: preferences.fontWeight,
		fontWeightBold: preferences.fontWeightBold,
		terminalPadding: preferences.terminalPadding,
	});
}

export function terminalDerivedPreferencesEqual(
	left: TerminalAppearancePreferences,
	right: TerminalAppearancePreferences,
): boolean {
	if (
		left.backgroundOpacityPercent !== right.backgroundOpacityPercent ||
		left.fontFamily !== right.fontFamily ||
		left.fontLigatures !== right.fontLigatures ||
		left.fontSize !== right.fontSize ||
		left.fontWeight !== right.fontWeight ||
		left.fontWeightBold !== right.fontWeightBold ||
		left.terminalPadding !== right.terminalPadding
	) {
		return false;
	}
	return true;
}

export function terminalThemePreferencesEqual(
	left: TerminalAppearancePreferences,
	right: TerminalAppearancePreferences,
): boolean {
	return (
		left.applicationTheme === right.applicationTheme &&
		left.lightTheme === right.lightTheme &&
		left.darkTheme === right.darkTheme &&
		left.terminalDisplayTheme === right.terminalDisplayTheme &&
		(left.terminalDisplayTheme !== 'custom' ||
			(left.customBackground === right.customBackground &&
				left.customForeground === right.customForeground))
	);
}

export function terminalAppearancePreferencesEqual(
	left: TerminalAppearancePreferences,
	right: TerminalAppearancePreferences,
): boolean {
	return (
		left.applicationTheme === right.applicationTheme &&
		left.backgroundOpacityPercent === right.backgroundOpacityPercent &&
		left.customBackground === right.customBackground &&
		left.customForeground === right.customForeground &&
		left.darkTheme === right.darkTheme &&
		left.fontFamily === right.fontFamily &&
		left.fontLigatures === right.fontLigatures &&
		left.fontSize === right.fontSize &&
		left.fontWeight === right.fontWeight &&
		left.fontWeightBold === right.fontWeightBold &&
		left.lightTheme === right.lightTheme &&
		left.terminalPadding === right.terminalPadding &&
		left.terminalDisplayTheme === right.terminalDisplayTheme
	);
}

export function normalizeTerminalHexColor(value: string): TerminalHexColor | null {
	if (!/^#[\da-fA-F]{6}$/u.test(value)) return null;
	const normalized: TerminalHexColor = `#${value.slice(1).toLowerCase()}`;
	return normalized;
}

export function resolveTerminalFontSizeChange(
	currentFontSize: number,
	change: TerminalFontSizeChange,
): number {
	if (change === 'reset') return DEFAULT_TERMINAL_APPEARANCE_PREFERENCES.fontSize;
	const candidate =
		currentFontSize + (change === 'increase' ? TERMINAL_FONT_SIZE_STEP : -TERMINAL_FONT_SIZE_STEP);
	return Math.min(MAXIMUM_TERMINAL_FONT_SIZE, Math.max(MINIMUM_TERMINAL_FONT_SIZE, candidate));
}

export type TerminalAppearancePresentationSource = TerminalPresentationConfigurationSource;
