import { describe, expect, it } from 'vitest';

import {
	DEFAULT_TERMINAL_INTERACTION_PREFERENCES,
	terminalInteractionConfigurationFromPreferences,
} from '../terminal-interaction-preferences';

describe('terminal interaction preferences', () => {
	it('defaults copy-on-select on and derives one immutable source-neutral configuration', () => {
		expect(DEFAULT_TERMINAL_INTERACTION_PREFERENCES).toEqual({ copyOnSelect: true });

		const configuration = terminalInteractionConfigurationFromPreferences({ copyOnSelect: false });
		expect(configuration).toEqual({ copyOnSelect: false });
		expect(Object.isFrozen(configuration)).toBe(true);
	});
});
