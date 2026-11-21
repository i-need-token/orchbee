import { defineMessages } from 'react-intl';

import { enUSDesktopWindowMessages } from '../locales';
import { createMessageDescriptorFactory } from './createMessageDescriptorFactory';

const message = createMessageDescriptorFactory(enUSDesktopWindowMessages);

export const desktopWindowMessages = defineMessages({
	category: message('desktop.window.category'),
	crashedClose: message('desktop.window.crashedClose'),
	crashedDetail: message('desktop.window.crashedDetail'),
	crashedReopen: message('desktop.window.crashedReopen'),
	crashedTitle: message('desktop.window.crashedTitle'),
	dataFolderLayoutNewer: message('desktop.window.dataFolderLayoutNewer'),
	dataFolderLayoutNewerUnnamed: message('desktop.window.dataFolderLayoutNewerUnnamed'),
	dataFolderNotCreated: message(
		'desktop.window.dataFolderNotCreated',
		'The bundled orchbee program could not create the data folder for a reason it did not name. root is the folder, which is not translated.',
	),
	dataFolderRefused: message('desktop.window.dataFolderRefused'),
	dataFolderUnreadable: message('desktop.window.dataFolderUnreadable'),
	placementNewer: message('desktop.window.placementNewer'),
	placementNotUpdated: message('desktop.window.placementNotUpdated'),
	recordNoticeClose: message('desktop.window.recordNoticeClose'),
	toggleFullScreen: message('desktop.window.action.toggleFullScreen'),
	toggleMaximized: message('desktop.window.action.toggleMaximized'),
});
