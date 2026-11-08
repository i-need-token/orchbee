import { defineMessages } from 'react-intl';

import { enUSDesktopDiagnosticsMessages } from '../locales';
import { createMessageDescriptorFactory } from './createMessageDescriptorFactory';

const message = createMessageDescriptorFactory(enUSDesktopDiagnosticsMessages);

export const desktopDiagnosticsMessages = defineMessages({
	bundleFailed: message('desktop.diagnostics.bundleFailed'),
	bundleFailedDetail: message('desktop.diagnostics.bundleFailedDetail'),
	bundlePending: message('desktop.diagnostics.bundlePending'),
	category: message('desktop.diagnostics.category'),
	close: message('desktop.diagnostics.close'),
	saveBundle: message('desktop.diagnostics.action.saveBundle'),
	saveDialogTitle: message('desktop.diagnostics.saveDialogTitle'),
	taskDescription: message('desktop.diagnostics.taskDescription'),
	zipFilter: message('desktop.diagnostics.zipFilter'),
});
