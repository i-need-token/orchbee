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
	compactItem: message(
		'fileBar.tree.compactItem',
		'Accessible name of one folder in a compact row of a File Bar tree, which joins folders that each hold a single folder, as VS Code’s compact folders do. `name` is the folder’s name and `parents` lists the folders before it in the row, nearest first.',
	),
	explorer: message(
		'fileBar.views.explorer',
		'Name of the File Bar view that browses the active Workspace’s files, as VS Code names its Explorer.',
	),
	findClose: message(
		'fileBar.tree.find.close',
		'Accessible name and tooltip of the button that closes the find above a File Bar tree, as VS Code’s tree find widget names it.',
	),
	findFilterMode: message(
		'fileBar.tree.find.filterMode',
		'Accessible name and tooltip of the toggle in a File Bar tree’s find that hides the rows that do not match instead of highlighting the ones that do, as VS Code names its Filter toggle.',
	),
	findFilterPlaceholder: message(
		'fileBar.tree.find.filterPlaceholder',
		'Placeholder of a File Bar tree’s find input while it filters the rows, as VS Code’s tree find widget shows it.',
	),
	findFuzzyMatch: message(
		'fileBar.tree.find.fuzzyMatch',
		'Accessible name and tooltip of the toggle in a File Bar tree’s find that matches the typed characters in order anywhere in a name instead of as one run, as VS Code names its Fuzzy Match toggle.',
	),
	findLabel: message(
		'fileBar.tree.find.label',
		'Accessible name of the find region above a File Bar tree, which finds its rows by name.',
	),
	findNotFound: message(
		'fileBar.tree.find.notFound',
		'Shown under a File Bar tree’s find input when no row matches what was typed, as VS Code’s tree find widget says it.',
	),
	findResults: message(
		'fileBar.tree.find.results',
		'Announced to screen readers after a File Bar tree’s find applies what was typed. `count` is how many items matched.',
	),
	findSearchPlaceholder: message(
		'fileBar.tree.find.searchPlaceholder',
		'Accessible name of a File Bar tree’s find input, and its placeholder while it highlights the matching rows, as VS Code’s tree find widget shows it.',
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
	moveLeft: message(
		'fileBar.actions.moveLeft',
		'Action and menu item that moves the File Bar to the left of the terminal area, between the Sidebar and the terminal area, as VS Code’s Move Primary Side Bar Left.',
	),
	moveRight: message(
		'fileBar.actions.moveRight',
		'Action and menu item that moves the File Bar to the right of the terminal area, as VS Code’s Move Primary Side Bar Right.',
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
	togglePosition: message(
		'fileBar.actions.togglePosition',
		'Action that moves the File Bar to the other side of the terminal area, as VS Code’s Toggle Primary Side Bar Position.',
	),
	viewSwitcher: message(
		'fileBar.viewSwitcher',
		'Accessible name of the row of toggles at the top of the File Bar that switches between its views.',
	),
});
