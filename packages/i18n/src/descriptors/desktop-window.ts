import { defineMessages } from 'react-intl';

import { enUSDesktopWindowMessages } from '../locales';
import { createMessageDescriptorFactory } from './createMessageDescriptorFactory';

const message = createMessageDescriptorFactory(enUSDesktopWindowMessages);

export const desktopWindowMessages = defineMessages({
	category: message('desktop.window.category'),
	maximizedUnavailableInFullScreen: message('desktop.window.maximizedUnavailableInFullScreen'),
	toggleFullScreen: message('desktop.window.action.toggleFullScreen'),
	toggleMaximized: message('desktop.window.action.toggleMaximized'),
	unavailable: message('desktop.window.actionUnavailable'),
});
