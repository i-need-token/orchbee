import { defineMessages } from 'react-intl';

import { enUSDesktopSecureKeyboardEntryMessages } from '../locales';

export const desktopSecureKeyboardEntryMessages = defineMessages({
	category: message('desktop.secureKeyboardEntry.category'),
	toggle: message('desktop.secureKeyboardEntry.action.toggle'),
	unavailable: message('desktop.secureKeyboardEntry.unavailable'),
});

function message(id: keyof typeof enUSDesktopSecureKeyboardEntryMessages): {
	readonly defaultMessage: string;
	readonly id: string;
} {
	return { defaultMessage: enUSDesktopSecureKeyboardEntryMessages[id], id };
}
