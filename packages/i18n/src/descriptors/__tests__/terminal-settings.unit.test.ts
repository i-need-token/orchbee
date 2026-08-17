import { describe, expect, it } from 'vitest';

import { enUSTerminalSettingsMessages } from '../../locales';
import { terminalSettingsMessages } from '../terminal-settings';

describe('terminalSettingsMessages', () => {
	it('uses canonical English defaults for every terminal-settings descriptor', () => {
		for (const descriptor of Object.values(terminalSettingsMessages)) {
			expect(descriptor.id).toBeTypeOf('string');
			expect(descriptor.defaultMessage).toBe(
				enUSTerminalSettingsMessages[descriptor.id as keyof typeof enUSTerminalSettingsMessages],
			);
		}
	});
});
