import { defineMessages } from 'react-intl';

import { enUSDesktopMenuMessages } from '../locales';
import { createMessageDescriptorFactory } from './createMessageDescriptorFactory';

const message = createMessageDescriptorFactory(enUSDesktopMenuMessages);

/**
 * The standard roots and items of Desktop's menu bar, which Electron would otherwise label in
 * English. Each follows the platform's own menu wording; on macOS, Apple's.
 */
export const desktopMenuMessages = defineMessages({
	about: message(
		'desktop.menu.about',
		'macOS application menu item that shows the About panel. {application} is the application name, such as Orchbee or Orchbee Test.',
	),
	actualSize: message(
		'desktop.menu.actualSize',
		"View menu item that returns the page's zoom to 100%.",
	),
	bringAllToFront: message(
		'desktop.menu.bringAllToFront',
		"macOS Window menu item that brings every Orchbee window in front of other applications' windows.",
	),
	close: message(
		'desktop.menu.close',
		'Windows and Linux Window menu item that closes the window.',
	),
	closeWindow: message('desktop.menu.closeWindow', 'macOS File menu item that closes the window.'),
	copy: message('desktop.menu.copy'),
	cut: message('desktop.menu.cut'),
	delete: message('desktop.menu.delete', 'Edit menu item that deletes the selected text.'),
	edit: message('desktop.menu.edit', 'Title of the Edit menu in the menu bar.'),
	exit: message('desktop.menu.exit', 'Windows File menu item that quits Orchbee.'),
	file: message('desktop.menu.file', 'Title of the File menu in the menu bar.'),
	forceReload: message(
		'desktop.menu.forceReload',
		'View menu item that reloads the page without its cache.',
	),
	help: message('desktop.menu.help', 'Title of the Help menu in the menu bar.'),
	hide: message(
		'desktop.menu.hide',
		'macOS application menu item that hides every Orchbee window. {application} is the application name.',
	),
	hideOthers: message(
		'desktop.menu.hideOthers',
		'macOS application menu item that hides every other application.',
	),
	minimize: message('desktop.menu.minimize'),
	paste: message('desktop.menu.paste'),
	pasteAndMatchStyle: message('desktop.menu.pasteAndMatchStyle', 'macOS Edit menu item.'),
	quit: message(
		'desktop.menu.quit',
		'Item of the macOS application menu, and of the Linux File menu, that quits Orchbee. {application} is the application name.',
	),
	redo: message('desktop.menu.redo'),
	reload: message('desktop.menu.reload', 'View menu item that reloads the page.'),
	selectAll: message('desktop.menu.selectAll'),
	services: message(
		'desktop.menu.services',
		"macOS application menu submenu that holds the system's Services.",
	),
	showAll: message(
		'desktop.menu.showAll',
		'macOS application menu item that shows every hidden application again.',
	),
	speech: message(
		'desktop.menu.speech',
		'macOS Edit menu submenu that reads the selected text aloud.',
	),
	startSpeaking: message('desktop.menu.startSpeaking', 'Item of the macOS Speech submenu.'),
	stopSpeaking: message('desktop.menu.stopSpeaking', 'Item of the macOS Speech submenu.'),
	toggleDeveloperTools: message(
		'desktop.menu.toggleDeveloperTools',
		"View menu item that shows or hides the page's developer tools.",
	),
	undo: message('desktop.menu.undo'),
	view: message(
		'desktop.menu.view',
		"Title of the View menu in the menu bar; use the platform's own term, on macOS Apple's.",
	),
	window: message('desktop.menu.window', 'Title of the Window menu in the menu bar.'),
	zoomIn: message('desktop.menu.zoomIn', 'View menu item that enlarges the page.'),
	zoomOut: message('desktop.menu.zoomOut', 'View menu item that shrinks the page.'),
});
