import { defineMessages } from 'react-intl';

import { enUSFileOverlayMessages } from '../locales';
import { createMessageDescriptorFactory } from './createMessageDescriptorFactory';

const message = createMessageDescriptorFactory(enUSFileOverlayMessages);

/** The file overlay: the frame over the terminal area where files open in tabs. */
export const fileOverlayMessages = defineMessages({
	close: message(
		'fileOverlay.actions.close',
		'Accessible name of the button at the end of a file tab in the file overlay that closes that tab.',
	),
	closeEditor: message(
		'fileOverlay.actions.closeEditor',
		'Action that closes a file tab in the file overlay, the active one unless a tab is named. VS Code calls a file open in a tab an editor.',
	),
	expand: message(
		'fileOverlay.actions.expand',
		'Label of the button at the end of the file overlay’s tab row that widens the overlay to the whole window.',
	),
	keepEditor: message(
		'fileOverlay.actions.keepEditor',
		'Action that keeps the file overlay’s tab in preview open, so the next file opened in preview does not replace it. VS Code calls a file open in a tab an editor.',
	),
	label: message(
		'fileOverlay.label',
		'Accessible name of the file overlay, the frame over the terminal area where files open in tabs.',
	),
	loadFailed: message(
		'fileOverlay.loadFailed',
		'Error card shown when the file overlay could not be loaded, so the file opened does not show.',
	),
	pinnedTabName: message(
		'fileOverlay.tabs.pinnedName',
		'Accessible name of a file tab pinned to the start of the file overlay’s tab row. {name} is the file name the tab shows, with its folder when another tab shows the same name.',
	),
	previewTabName: message(
		'fileOverlay.tabs.previewName',
		'Accessible name of the file tab in preview, which the next file opened in preview replaces. {name} is the file name the tab shows, with its folder when another tab shows the same name.',
	),
	previewTabPath: message(
		'fileOverlay.tabs.previewPath',
		'Tooltip of the file tab in preview, which the next file opened in preview replaces. {path} is the file’s full path.',
	),
	restore: message(
		'fileOverlay.actions.restore',
		'Label of the button at the end of the file overlay’s tab row that returns the widened overlay to its size before.',
	),
	retry: message(
		'fileOverlay.actions.retry',
		'Button on the error card that tries loading the file overlay again.',
	),
	showEditor: message(
		'fileOverlay.actions.showEditor',
		'Action that shows the file of one tab in the file overlay. VS Code calls a file open in a tab an editor.',
	),
	tabsLabel: message(
		'fileOverlay.tabs.label',
		'Accessible name of the row of file tabs at the top of the file overlay. VS Code calls a file open in a tab an editor.',
	),
	toggleFull: message(
		'fileOverlay.actions.toggleFull',
		'Action that widens the file overlay to the whole window, or returns it to its size before.',
	),
	unpin: message(
		'fileOverlay.actions.unpin',
		'Accessible name of the pin button at the end of a pinned file tab in the file overlay, which unpins that tab.',
	),
	unpinEditor: message(
		'fileOverlay.actions.unpinEditor',
		'Action that unpins a file tab pinned to the start of the file overlay’s tab row, the active one unless a tab is named. VS Code calls a file open in a tab an editor.',
	),
});
