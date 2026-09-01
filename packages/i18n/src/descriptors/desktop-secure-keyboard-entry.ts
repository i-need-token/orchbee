import { defineMessages } from 'react-intl';

import { enUSDesktopSecureKeyboardEntryMessages } from '../locales';
import { createMessageDescriptorFactory } from './createMessageDescriptorFactory';

const message = createMessageDescriptorFactory(enUSDesktopSecureKeyboardEntryMessages);

export const desktopSecureKeyboardEntryMessages = defineMessages({
	category: message('desktop.secureKeyboardEntry.category'),
	toggle: message('desktop.secureKeyboardEntry.action.toggle'),
	unavailable: message('desktop.secureKeyboardEntry.unavailable'),
});
