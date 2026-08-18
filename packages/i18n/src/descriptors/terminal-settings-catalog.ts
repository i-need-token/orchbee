import type { MessageDescriptor } from 'react-intl';

import {
	enUSTerminalSettingsCatalogMessages,
	type TerminalSettingsCatalogMessageId,
} from '../locales';

export const terminalSettingsCatalogMessages = {
	get(messageId: TerminalSettingsCatalogMessageId): MessageDescriptor {
		return {
			defaultMessage: enUSTerminalSettingsCatalogMessages[messageId],
			id: messageId,
		};
	},
	has(messageId: string): messageId is TerminalSettingsCatalogMessageId {
		return Object.hasOwn(enUSTerminalSettingsCatalogMessages, messageId);
	},
} as const;
