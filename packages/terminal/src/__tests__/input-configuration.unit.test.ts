import { describe, expect, it } from 'vitest';

import {
	DEFAULT_TERMINAL_INPUT_CONFIGURATION,
	isTerminalOptionAsAltMode,
	TerminalOptionAsAltMode,
} from '../input-configuration';

describe('terminal input configuration', () => {
	it('defines the complete Option-as-Alt domain with a safe off default', () => {
		expect(DEFAULT_TERMINAL_INPUT_CONFIGURATION).toEqual({ optionAsAlt: 'off' });
		expect(Object.values(TerminalOptionAsAltMode)).toEqual(['both', 'left', 'off', 'right']);
		for (const mode of Object.values(TerminalOptionAsAltMode)) {
			expect(isTerminalOptionAsAltMode(mode)).toBe(true);
		}
		expect(isTerminalOptionAsAltMode('automatic')).toBe(false);
		expect(isTerminalOptionAsAltMode(null)).toBe(false);
	});
});
