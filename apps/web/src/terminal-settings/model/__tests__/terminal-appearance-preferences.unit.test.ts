import { DEFAULT_USER_PREFERENCES } from '@orchbee/workbench-client-protocol';
import { describe, expect, it } from 'vitest';

import {
	DEFAULT_TERMINAL_APPEARANCE_PREFERENCES,
	MAXIMUM_TERMINAL_FONT_SIZE,
	MINIMUM_TERMINAL_FONT_SIZE,
	normalizeTerminalHexColor,
	resolveTerminalFontSizeChange,
	terminalAppearancePreferencesEqual,
	terminalDerivedPreferencesEqual,
	terminalPresentationConfigurationFromPreferences,
	type TerminalAppearancePreferences,
} from '../terminal-appearance-preferences';

describe('terminal appearance preferences', () => {
	it('defaults the common appearance leaves to the shared preference defaults', () => {
		for (const key of [
			'applicationTheme',
			'lightTheme',
			'darkTheme',
			'terminalDisplayTheme',
			'customForeground',
			'customBackground',
		] as const) {
			expect(DEFAULT_TERMINAL_APPEARANCE_PREFERENCES[key]).toBe(DEFAULT_USER_PREFERENCES[key]);
		}
	});

	it('applies the two-point font-size policy within product bounds', () => {
		expect(resolveTerminalFontSizeChange(13, 'increase')).toBe(15);
		expect(resolveTerminalFontSizeChange(13, 'decrease')).toBe(11);
		expect(resolveTerminalFontSizeChange(MAXIMUM_TERMINAL_FONT_SIZE - 1, 'increase')).toBe(
			MAXIMUM_TERMINAL_FONT_SIZE,
		);
		expect(resolveTerminalFontSizeChange(MINIMUM_TERMINAL_FONT_SIZE + 1, 'decrease')).toBe(
			MINIMUM_TERMINAL_FONT_SIZE,
		);
		expect(resolveTerminalFontSizeChange(31, 'reset')).toBe(
			DEFAULT_TERMINAL_APPEARANCE_PREFERENCES.fontSize,
		);
	});

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
			fontLigatures: false,
			fontSize: 17,
			fontWeight: 300,
			fontWeightBold: 800,
			terminalPadding: 12,
			terminalDisplayTheme: 'custom',
		});

		expect(configuration).toMatchObject({
			allowTransparency: true,
			backgroundOpacity: 0.65,
			defaultColorOverride: null,
			fontFamily: 'Example Mono, monospace',
			fontLigatures: false,
			fontSize: 17,
			fontWeight: 300,
			fontWeightBold: 800,
			terminalPadding: 12,
		});
		expect(Object.isFrozen(configuration)).toBe(true);
	});

	it.each([
		['fontLigatures', false],
		['fontWeight', 300],
		['fontWeightBold', 800],
		['terminalPadding', 12],
	] as const)('treats %s as a terminal-derived presentation preference', (field, value) => {
		expect(
			terminalDerivedPreferencesEqual(DEFAULT_TERMINAL_APPEARANCE_PREFERENCES, {
				...DEFAULT_TERMINAL_APPEARANCE_PREFERENCES,
				[field]: value,
			}),
		).toBe(false);
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
				terminalDisplayTheme: 'theme',
			}).defaultColorOverride,
		).toBeNull();
		expect(
			terminalPresentationConfigurationFromPreferences({
				...DEFAULT_TERMINAL_APPEARANCE_PREFERENCES,
				terminalDisplayTheme: 'host',
			}).defaultColorOverride,
		).toBeNull();
	});
});
