import { defineMessages } from 'react-intl';

import { enUSDesktopWindowMessages } from '../locales';

export const desktopWindowMessages = defineMessages({
	category: message('desktop.window.category'),
	create: message('desktop.window.action.create'),
	maximizedUnavailableInFullScreen: message('desktop.window.maximizedUnavailableInFullScreen'),
	toggleFullScreen: message('desktop.window.action.toggleFullScreen'),
	toggleMaximized: message('desktop.window.action.toggleMaximized'),
	unavailable: message('desktop.window.actionUnavailable'),
});

function message(id: keyof typeof enUSDesktopWindowMessages): {
	readonly defaultMessage: string;
	readonly id: string;
} {
	return { defaultMessage: enUSDesktopWindowMessages[id], id };
}
