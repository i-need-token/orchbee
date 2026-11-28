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
	closeAllEditors: message(
		'fileOverlay.actions.closeAllEditors',
		'Action that closes every file tab in the file overlay except the pinned ones. VS Code calls a file open in a tab an editor.',
	),
	closeEditor: message(
		'fileOverlay.actions.closeEditor',
		'Action that closes a file tab in the file overlay, the active one unless a tab is named. VS Code calls a file open in a tab an editor.',
	),
	closeEditorsToTheRight: message(
		'fileOverlay.actions.closeEditorsToTheRight',
		'Action that closes the file tabs after a file tab in the file overlay’s tab row, except the pinned ones; the tab is the active one unless a tab is named. VS Code calls a file open in a tab an editor.',
	),
	closeOtherEditors: message(
		'fileOverlay.actions.closeOtherEditors',
		'Action that closes every file tab in the file overlay but one, except the pinned ones; the one kept is the active tab unless a tab is named. VS Code calls a file open in a tab an editor.',
	),
	closeSavedEditors: message(
		'fileOverlay.actions.closeSavedEditors',
		'Action that closes every file tab in the file overlay without unsaved edits, except the pinned ones. VS Code calls a file open in a tab an editor.',
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
	lastEditor: message(
		'fileOverlay.actions.lastEditor',
		'Action that shows the file of the last tab in the file overlay’s tab row. VS Code calls a file open in a tab an editor.',
	),
	loadFailed: message(
		'fileOverlay.loadFailed',
		'Error card shown when the file overlay could not be loaded, so the file opened does not show.',
	),
	nextEditor: message(
		'fileOverlay.actions.nextEditor',
		'Action that shows the file of the tab after the active one in the file overlay’s tab row, the first after the last. VS Code calls a file open in a tab an editor.',
	),
	pinEditor: message(
		'fileOverlay.actions.pinEditor',
		'Action that pins a file tab to the start of the file overlay’s tab row, the active one unless a tab is named. VS Code calls a file open in a tab an editor.',
	),
	pinnedTabName: message(
		'fileOverlay.tabs.pinnedName',
		'Accessible name of a file tab pinned to the start of the file overlay’s tab row. {name} is the file name the tab shows, with its folder when another tab shows the same name.',
	),
	positionEditor: message(
		'fileOverlay.actions.positionEditor',
		'Action that shows the file of the tab at a given place in the file overlay’s tab row, counted from the start. VS Code calls a file open in a tab an editor.',
	),
	positionEditorNumbered: message(
		'fileOverlay.actions.positionEditorNumbered',
		'Name of the keyboard shortcut that shows the file of one tab in the file overlay’s tab row. {position} is that tab’s place in the row, 1 to 9, counted from the start. VS Code calls a file open in a tab an editor.',
	),
	previewTabName: message(
		'fileOverlay.tabs.previewName',
		'Accessible name of the file tab in preview, which the next file opened in preview replaces. {name} is the file name the tab shows, with its folder when another tab shows the same name.',
	),
	previewTabPath: message(
		'fileOverlay.tabs.previewPath',
		'Tooltip of the file tab in preview, which the next file opened in preview replaces. {path} is the file’s full path.',
	),
	previousEditor: message(
		'fileOverlay.actions.previousEditor',
		'Action that shows the file of the tab before the active one in the file overlay’s tab row, the last before the first. VS Code calls a file open in a tab an editor.',
	),
	reopenClosedEditor: message(
		'fileOverlay.actions.reopenClosedEditor',
		'Action that opens again the file overlay tabs closed last, together if they closed together. VS Code calls a file open in a tab an editor.',
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
	tabMenuClose: message(
		'fileOverlay.tabs.menu.close',
		'Item in a file tab’s context menu in the file overlay that closes that tab.',
	),
	tabMenuCloseAll: message(
		'fileOverlay.tabs.menu.closeAll',
		'Item in a file tab’s context menu in the file overlay that closes every tab except the pinned ones.',
	),
	tabMenuCloseOthers: message(
		'fileOverlay.tabs.menu.closeOthers',
		'Item in a file tab’s context menu in the file overlay that closes every tab but that one, except the pinned ones.',
	),
	tabMenuCloseRight: message(
		'fileOverlay.tabs.menu.closeRight',
		'Item in a file tab’s context menu in the file overlay that closes the tabs after that one in the row, except the pinned ones.',
	),
	tabMenuCloseSaved: message(
		'fileOverlay.tabs.menu.closeSaved',
		'Item in a file tab’s context menu in the file overlay that closes every tab without unsaved edits, except the pinned ones.',
	),
	tabMenuKeepOpen: message(
		'fileOverlay.tabs.menu.keepOpen',
		'Item in a file tab’s context menu in the file overlay that keeps that tab in preview open, so the next file opened in preview does not replace it. It is greyed for a tab not in preview.',
	),
	tabMenuPin: message(
		'fileOverlay.tabs.menu.pin',
		'Item in a file tab’s context menu in the file overlay that pins that tab to the start of the row.',
	),
	tabMenuUnpin: message(
		'fileOverlay.tabs.menu.unpin',
		'Item in a pinned file tab’s context menu in the file overlay that unpins that tab.',
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
