import type { TerminalPresentationConfiguration } from '@orchbee/terminal';

import {
	DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION_SOURCE,
	type TerminalPresentationConfigurationSource,
} from '../../terminal-presentation';

type ApplicationTheme = 'dark' | 'light';
type TerminalDisplayTheme = 'custom' | 'dark' | 'host' | 'light';
export type TerminalHexColor = `#${string}`;
export const TERMINAL_FONT_WEIGHT_OPTIONS = [100, 200, 300, 400, 500, 600, 700, 800, 900] as const;
export type TerminalFontWeight = (typeof TERMINAL_FONT_WEIGHT_OPTIONS)[number];

export interface TerminalAppearancePreferences {
	readonly applicationTheme: ApplicationTheme;
	readonly backgroundOpacityPercent: number;
	readonly customBackground: TerminalHexColor;
	readonly customForeground: TerminalHexColor;
	readonly fontFamily: string;
	readonly fontLigatures: boolean;
	readonly fontSize: number;
	readonly fontWeight: TerminalFontWeight;
	readonly fontWeightBold: TerminalFontWeight;
	readonly terminalPadding: number;
	readonly terminalDisplayTheme: TerminalDisplayTheme;
}

const productPresentation = DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION_SOURCE.current();

export const DEFAULT_TERMINAL_APPEARANCE_PREFERENCES: TerminalAppearancePreferences = Object.freeze(
	{
		applicationTheme: 'light',
		backgroundOpacityPercent: 100,
		customBackground: '#fcfcfc',
		customForeground: '#141414',
		fontFamily: productPresentation.fontFamily,
		fontLigatures: true,
		fontSize: productPresentation.fontSize,
		fontWeight: 400,
		fontWeightBold: 700,
		terminalPadding: 0,
		terminalDisplayTheme: 'host',
	},
);

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
		left.fontFamily === right.fontFamily &&
		left.fontLigatures === right.fontLigatures &&
		left.fontSize === right.fontSize &&
		left.fontWeight === right.fontWeight &&
		left.fontWeightBold === right.fontWeightBold &&
		left.terminalPadding === right.terminalPadding &&
		left.terminalDisplayTheme === right.terminalDisplayTheme
	);
}

export function normalizeTerminalHexColor(value: string): TerminalHexColor | null {
	if (!/^#[\da-fA-F]{6}$/u.test(value)) return null;
	const normalized: TerminalHexColor = `#${value.slice(1).toLowerCase()}`;
	return normalized;
}

export type TerminalAppearancePresentationSource = TerminalPresentationConfigurationSource;
