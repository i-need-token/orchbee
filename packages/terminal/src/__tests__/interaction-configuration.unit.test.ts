import { describe, expect, it } from 'vitest';

import {
	DEFAULT_TERMINAL_INTERACTION_CONFIGURATION,
	normalizeTerminalInteractionConfiguration,
} from '../interaction-configuration';

describe('terminal interaction configuration', () => {
	it('freezes the default and isolates a complete record', () => {
		expect(Object.isFrozen(DEFAULT_TERMINAL_INTERACTION_CONFIGURATION)).toBe(true);
		expect(normalizeTerminalInteractionConfiguration({ copyOnSelect: false })).toEqual({
			configuration: { copyOnSelect: false },
			kind: 'valid',
		});
	});

	it.each([{}, { copyOnSelect: true, fontLigatures: false }, { copyOnSelect: 'yes' }])(
		'rejects incomplete, extra, or invalid records',
		(candidate) => {
			expect(normalizeTerminalInteractionConfiguration(candidate)).toEqual({
				field: 'configuration',
				kind: 'invalid',
			});
		},
	);
});
