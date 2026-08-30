import { describe, expect, it } from 'vitest';

import {
	DEFAULT_TERMINAL_INTERACTION_PREFERENCES,
	terminalInteractionConfigurationFromPreferences,
} from '../terminal-interaction-preferences';

describe('terminal interaction preferences', () => {
	it('defaults both interactions on and derives one immutable terminal configuration', () => {
		expect(DEFAULT_TERMINAL_INTERACTION_PREFERENCES).toEqual({
			copyOnSelect: true,
			trimTrailingBlankRowsForAgents: true,
		});

		const configuration = terminalInteractionConfigurationFromPreferences({
			copyOnSelect: false,
			trimTrailingBlankRowsForAgents: false,
		});
		expect(configuration).toEqual({ copyOnSelect: false });
		expect(Object.isFrozen(configuration)).toBe(true);
	});
});
