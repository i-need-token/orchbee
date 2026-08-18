import { describe, expect, it } from 'vitest';

import { enUSTerminalSettingsCatalogMessages } from '../../locales';
import { terminalSettingsCatalogMessages } from '../terminal-settings-catalog';

describe('terminalSettingsCatalogMessages', () => {
	it('creates descriptors from the canonical English settings catalog', () => {
		const messageId = 'terminal.settings.catalog.categories.general.title';
		expect(terminalSettingsCatalogMessages.get(messageId)).toEqual({
			defaultMessage: enUSTerminalSettingsCatalogMessages[messageId],
			id: messageId,
		});
		expect(terminalSettingsCatalogMessages.has(messageId)).toBe(true);
		expect(terminalSettingsCatalogMessages.has('terminal.settings.catalog.missing')).toBe(false);
	});
});
