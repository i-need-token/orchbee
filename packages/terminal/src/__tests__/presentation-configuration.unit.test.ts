import { describe, expect, it } from 'vitest';

import {
	DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION,
	normalizeTerminalPresentationConfiguration,
} from '../presentation-configuration';

describe('terminal presentation configuration', () => {
	it('preserves the established complete default configuration', () => {
		expect(DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION).toEqual({
			allowTransparency: false,
			fontFamily: 'monospace',
			fontLigatures: true,
			fontMetricModifiers: {},
			fontSize: 15,
			fontWeight: 'normal',
			fontWeightBold: 'bold',
			paddingBalance: false,
		});
	});

	it('validates and isolates one complete configuration', () => {
		const candidate = {
			allowTransparency: true,
			fontFamily: 'Orchbee Test Mono',
			fontLigatures: false,
			fontMetricModifiers: { cellHeight: '20%', cursorThickness: 2 },
			fontSize: 18,
			fontWeight: 450,
			fontWeightBold: 700,
			paddingBalance: 'equal',
		} as const;
		const result = normalizeTerminalPresentationConfiguration(candidate);

		expect(result).toEqual({ kind: 'valid', configuration: candidate });
		if (result.kind !== 'valid') throw new Error('configuration did not validate');
		expect(result.configuration).not.toBe(candidate);
		expect(result.configuration.fontMetricModifiers).not.toBe(candidate.fontMetricModifiers);
		expect(Object.isFrozen(result.configuration)).toBe(true);
		expect(Object.isFrozen(result.configuration.fontMetricModifiers)).toBe(true);
	});

	it.each([
		{
			name: 'partial candidate',
			value: { ...DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION, fontFamily: undefined },
		},
		{
			name: 'unknown field',
			value: { ...DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION, theme: {} },
		},
		{
			name: 'non-positive font size',
			value: { ...DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION, fontSize: 0 },
		},
		{
			name: 'invalid weight',
			value: { ...DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION, fontWeight: 1_001 },
		},
		{
			name: 'invalid metric modifier',
			value: {
				...DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION,
				fontMetricModifiers: { cellWidth: 'wider' },
			},
		},
	])('rejects a $name without producing a partial configuration', ({ value }) => {
		expect(normalizeTerminalPresentationConfiguration(value)).toMatchObject({ kind: 'invalid' });
	});

	it.each([
		{
			name: 'top-level ownKeys trap',
			value: new Proxy(
				{},
				{
					ownKeys: () => {
						throw new Error('ownKeys failed');
					},
				},
			),
		},
		{
			name: 'throwing field getter',
			value: Object.defineProperty(
				{ ...DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION },
				'fontFamily',
				{
					enumerable: true,
					get: () => {
						throw new Error('fontFamily failed');
					},
				},
			),
		},
		{
			name: 'nested metric ownKeys trap',
			value: {
				...DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION,
				fontMetricModifiers: new Proxy(
					{},
					{
						ownKeys: () => {
							throw new Error('metric ownKeys failed');
						},
					},
				),
			},
		},
	])('returns typed invalid for a $name instead of throwing', ({ value }) => {
		expect(() => normalizeTerminalPresentationConfiguration(value)).not.toThrow();
		expect(normalizeTerminalPresentationConfiguration(value)).toMatchObject({ kind: 'invalid' });
	});

	it('returns typed invalid for a revoked proxy instead of throwing', () => {
		const { proxy, revoke } = Proxy.revocable({}, {});
		revoke();

		expect(() => normalizeTerminalPresentationConfiguration(proxy)).not.toThrow();
		expect(normalizeTerminalPresentationConfiguration(proxy)).toEqual({
			field: 'configuration',
			kind: 'invalid',
		});
	});

	it.each([`${'9'.repeat(400)}%`, `-${'9'.repeat(400)}%`, '9'.repeat(400), `-${'9'.repeat(400)}`])(
		'rejects a metric modifier with a non-finite parsed representation',
		(cellWidth) => {
			expect(
				normalizeTerminalPresentationConfiguration({
					...DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION,
					fontMetricModifiers: { cellWidth },
				}),
			).toEqual({ field: 'fontMetricModifiers', kind: 'invalid' });
		},
	);
});
