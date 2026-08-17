import type { TerminalPresentationConfiguration } from '@orchbee/terminal';

import {
	DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION_SOURCE,
	type TerminalPresentationConfigurationSource,
} from '../../terminal-presentation';

type ApplicationTheme = 'dark' | 'light';
type TerminalDisplayTheme = 'custom' | 'dark' | 'host' | 'light';
export type TerminalHexColor = `#${string}`;

export interface TerminalAppearancePreferences {
	readonly applicationTheme: ApplicationTheme;
	readonly backgroundOpacityPercent: number;
	readonly customBackground: TerminalHexColor;
	readonly customForeground: TerminalHexColor;
	readonly fontFamily: string;
	readonly fontSize: number;
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
		fontSize: productPresentation.fontSize,
		terminalDisplayTheme: 'host',
	},
);

const LIGHT_DEFAULT_COLOR_OVERRIDE: NonNullable<
	TerminalPresentationConfiguration['defaultColorOverride']
> = Object.freeze({
	background: '#fcfcfc',
	foreground: '#141414',
});
const DARK_DEFAULT_COLOR_OVERRIDE: NonNullable<
	TerminalPresentationConfiguration['defaultColorOverride']
> = Object.freeze({
	background: '#1e1e1e',
	foreground: '#d4d4d4',
});

export function terminalPresentationConfigurationFromPreferences(
	preferences: TerminalAppearancePreferences,
): TerminalPresentationConfiguration {
	const defaultColorOverride = resolveDefaultColorOverride(preferences);
	return Object.freeze({
		...productPresentation,
		allowTransparency: preferences.backgroundOpacityPercent < 100,
		backgroundOpacity: preferences.backgroundOpacityPercent / 100,
		defaultColorOverride,
		fontFamily: preferences.fontFamily,
		fontSize: preferences.fontSize,
	});
}

export function terminalDerivedPreferencesEqual(
	left: TerminalAppearancePreferences,
	right: TerminalAppearancePreferences,
): boolean {
	if (
		left.backgroundOpacityPercent !== right.backgroundOpacityPercent ||
		left.fontFamily !== right.fontFamily ||
		left.fontSize !== right.fontSize ||
		left.terminalDisplayTheme !== right.terminalDisplayTheme
	) {
		return false;
	}
	return (
		left.terminalDisplayTheme !== 'custom' ||
		(left.customBackground === right.customBackground &&
			left.customForeground === right.customForeground)
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
		left.fontSize === right.fontSize &&
		left.terminalDisplayTheme === right.terminalDisplayTheme
	);
}

export function normalizeTerminalHexColor(value: string): TerminalHexColor | null {
	if (!/^#[\da-fA-F]{6}$/u.test(value)) return null;
	const normalized: TerminalHexColor = `#${value.slice(1).toLowerCase()}`;
	return normalized;
}

export type TerminalAppearancePresentationSource = TerminalPresentationConfigurationSource;

function resolveDefaultColorOverride(
	preferences: TerminalAppearancePreferences,
): TerminalPresentationConfiguration['defaultColorOverride'] {
	switch (preferences.terminalDisplayTheme) {
		case 'host':
			return null;
		case 'light':
			return LIGHT_DEFAULT_COLOR_OVERRIDE;
		case 'dark':
			return DARK_DEFAULT_COLOR_OVERRIDE;
		case 'custom':
			return Object.freeze({
				background: preferences.customBackground,
				foreground: preferences.customForeground,
			});
	}
}
