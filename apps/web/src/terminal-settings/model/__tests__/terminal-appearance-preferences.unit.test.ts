import { describe, expect, it } from 'vitest';

import {
	DEFAULT_TERMINAL_APPEARANCE_PREFERENCES,
	normalizeTerminalHexColor,
	terminalAppearancePreferencesEqual,
	terminalDerivedPreferencesEqual,
	terminalPresentationConfigurationFromPreferences,
	type TerminalAppearancePreferences,
} from '../terminal-appearance-preferences';

describe('terminal appearance preferences', () => {
	it('normalizes only a complete hexadecimal terminal color', () => {
		expect(normalizeTerminalHexColor('#ABCDEF')).toBe('#abcdef');
		expect(normalizeTerminalHexColor('#abc')).toBeNull();
		expect(normalizeTerminalHexColor('abcdef')).toBeNull();
	});

	it('derives only the terminal presentation fields from one immutable preference record', () => {
		const configuration = terminalPresentationConfigurationFromPreferences({
			...DEFAULT_TERMINAL_APPEARANCE_PREFERENCES,
			backgroundOpacityPercent: 65,
			customBackground: '#abcdef',
			customForeground: '#123456',
			fontFamily: 'Example Mono, monospace',
			fontSize: 17,
			terminalDisplayTheme: 'custom',
		});

		expect(configuration).toMatchObject({
			allowTransparency: true,
			backgroundOpacity: 0.65,
			defaultColorOverride: null,
			fontFamily: 'Example Mono, monospace',
			fontSize: 17,
		});
		expect(Object.isFrozen(configuration)).toBe(true);
	});

	it('keeps retained custom colors out of the terminal publication while Host is active', () => {
		const retainedColors: TerminalAppearancePreferences = {
			...DEFAULT_TERMINAL_APPEARANCE_PREFERENCES,
			customBackground: '#abcdef',
			customForeground: '#123456',
		};

		expect(
			terminalAppearancePreferencesEqual(DEFAULT_TERMINAL_APPEARANCE_PREFERENCES, retainedColors),
		).toBe(false);
		expect(
			terminalDerivedPreferencesEqual(DEFAULT_TERMINAL_APPEARANCE_PREFERENCES, retainedColors),
		).toBe(true);
		expect(
			terminalPresentationConfigurationFromPreferences(retainedColors).defaultColorOverride,
		).toBeNull();
	});

	it('leaves all terminal colors to the revisioned Host theme transaction', () => {
		expect(
			terminalPresentationConfigurationFromPreferences({
				...DEFAULT_TERMINAL_APPEARANCE_PREFERENCES,
				terminalDisplayTheme: 'light',
			}).defaultColorOverride,
		).toBeNull();
		expect(
			terminalPresentationConfigurationFromPreferences({
				...DEFAULT_TERMINAL_APPEARANCE_PREFERENCES,
				terminalDisplayTheme: 'dark',
			}).defaultColorOverride,
		).toBeNull();
	});
});
