import { defineMessages } from 'react-intl';

import { enUSFileBarMessages } from '../locales';
import { createMessageDescriptorFactory } from './createMessageDescriptorFactory';

const message = createMessageDescriptorFactory(enUSFileBarMessages);

/** The File Bar: the column of file views beside the terminal area, never the Sidebar. */
export const fileBarMessages = defineMessages({
	label: message(
		'fileBar.label',
		'Accessible name of the File Bar column, which holds the Explorer, Search and Source Control views.',
	),
	loadFailed: message(
		'fileBar.loadFailed',
		'Shown in the File Bar column when its views could not be loaded, so it stays empty.',
	),
	toggle: message('fileBar.actions.toggle', 'Action that shows or hides the File Bar column.'),
});
