import { defineMessages } from 'react-intl';

import { enUSFileBarMessages } from '../locales';
import { createMessageDescriptorFactory } from './createMessageDescriptorFactory';

const message = createMessageDescriptorFactory(enUSFileBarMessages);

/** The File Bar: the column of file views beside the terminal area, never the Sidebar. */
export const fileBarMessages = defineMessages({
	badgeLabelled: message(
		'fileBar.badge.labelled',
		'Accessible name and tooltip of a File Bar toggle that carries a count badge: the toggle’s name, then what the badge counts, as VS Code titles an activity with a badge.',
	),
	badgeOverThousands: message(
		'fileBar.badge.overThousands',
		'Text of a count badge above a whole number of thousands, as VS Code shows 1,500 changes as 1K+.',
	),
	badgeThousands: message(
		'fileBar.badge.thousands',
		'Text of a count badge at a whole number of thousands, as VS Code shows 2,000 changes as 2K.',
	),
	explorer: message(
		'fileBar.views.explorer',
		'Name of the File Bar view that browses the active Workspace’s files, as VS Code names its Explorer.',
	),
	hideFileBar: message(
		'fileBar.actions.hide',
		'Label of the button that hides the File Bar column while it shows.',
	),
	label: message(
		'fileBar.label',
		'Accessible name of the File Bar column, which holds the Explorer, Search and Source Control views.',
	),
	loadFailed: message(
		'fileBar.loadFailed',
		'Shown in the File Bar column when its views could not be loaded, so it stays empty.',
	),
	showExplorer: message(
		'fileBar.actions.showExplorer',
		'Action that shows the File Bar’s Explorer view and focuses it, or returns focus to the terminal from the focused view.',
	),
	showFileBar: message(
		'fileBar.actions.show',
		'Label of the button that shows the File Bar column while it is hidden.',
	),
	showSourceControl: message(
		'fileBar.actions.showSourceControl',
		'Action that shows the File Bar’s Source Control view and focuses it, or returns focus to the terminal from the focused view.',
	),
	sourceControl: message(
		'fileBar.views.sourceControl',
		'Name of the File Bar view that shows the active Workspace’s Git changes, as VS Code names its Source Control.',
	),
	toggle: message('fileBar.actions.toggle', 'Action that shows or hides the File Bar column.'),
	viewSwitcher: message(
		'fileBar.viewSwitcher',
		'Accessible name of the row of toggles at the top of the File Bar that switches between its views.',
	),
});
