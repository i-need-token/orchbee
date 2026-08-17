import { describe, expect, it } from 'vitest';

import {
	DEFAULT_MACOS_TERMINAL_INPUT_PREFERENCES,
	terminalInputConfigurationFromMacosPreferences,
} from '../macos-terminal-input-preferences';

describe('macOS terminal input preferences', () => {
	it('derive one complete source-neutral input configuration', () => {
		expect(DEFAULT_MACOS_TERMINAL_INPUT_PREFERENCES).toEqual({ optionAsAlt: 'off' });
		expect(terminalInputConfigurationFromMacosPreferences({ optionAsAlt: 'left' })).toEqual({
			optionAsAlt: 'left',
		});
	});
});
