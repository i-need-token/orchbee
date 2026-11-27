import { defineMessages } from 'react-intl';

import { enUSDesktopTrayMessages } from '../locales';
import { createMessageDescriptorFactory } from './createMessageDescriptorFactory';

const message = createMessageDescriptorFactory(enUSDesktopTrayMessages);

export const desktopTrayMessages = defineMessages({
	noticeBody: message(
		'desktop.tray.notice.body',
		'Body of the one-time Windows notification shown when closing the window leaves Orchbee running in the notification area. {terminals} is the number of terminals still running.',
	),
	noticeMenuBarBody: message(
		'desktop.tray.notice.menuBarBody',
		'Body of the one-time macOS notification shown when closing the window leaves Orchbee running in the Dock, whose menu-bar icon holds Stop Session. {terminals} is the number of terminals still running. Use the platform terms for the Dock and the menu bar.',
	),
	noticePanelBody: message(
		'desktop.tray.notice.panelBody',
		'Body of the one-time Linux notification shown when closing the window leaves Orchbee running in the background. Opening Orchbee again from the applications returns to the window, and its panel icon, which not every desktop shows, holds Stop Session. {terminals} is the number of terminals still running.',
	),
	noticeTitle: message(
		'desktop.tray.notice.title',
		'Title of the one-time notification, on Windows, macOS and Linux. {application} is the application name, such as Orchbee or Orchbee Test.',
	),
	open: message(
		'desktop.tray.open',
		'Menu item of the Windows notification-area icon, the macOS menu-bar icon and the Linux panel icon, always offered, and of the macOS Window menu while no window shows, that shows and focuses the window, or opens one.',
	),
	quitAll: message(
		'desktop.tray.quitAll',
		'Menu item of the Windows notification-area icon, the macOS menu-bar icon, the Linux panel icon, and the macOS application menu, just above the quit, and the button a terminal shows once Orchbee cannot reconnect to it, that asks to stop the Session, ending every terminal and Agent, and then quit Orchbee. It names the Session as the orchbee session stop command does.',
	),
	quitAllCancel: message(
		'desktop.tray.quitAll.cancel',
		'Button of the Stop Session confirmation that leaves everything running; it has the focus and answers Esc.',
	),
	quitAllConfirm: message(
		'desktop.tray.quitAll.confirm',
		'Button of the Stop Session confirmation that stops every terminal and quits Orchbee.',
	),
	quitAllDetail: message(
		'desktop.tray.quitAll.detail',
		'Body of the Stop Session confirmation. {terminals} is the number of running terminals that stop; {agents} is how many Agents are working in them.',
	),
	quitAllFailureBusy: message(
		'desktop.tray.quitAll.failure.busy',
		'Why Stop Session left everything running: another stop of the same Session is already under way.',
	),
	quitAllFailureClose: message(
		'desktop.tray.quitAll.failure.close',
		'Button that dismisses the Stop Session failure report.',
	),
	quitAllFailureNotSent: message(
		'desktop.tray.quitAll.failure.notSent',
		'Why Stop Session left everything running: the window could not reach the Session, so nothing was asked to stop.',
	),
	quitAllFailureRefused: message(
		'desktop.tray.quitAll.failure.refused',
		'Why Stop Session left everything running: the Session declined the stop.',
	),
	quitAllFailureStopFailed: message(
		'desktop.tray.quitAll.failure.stopFailed',
		'Why Stop Session did not quit: the Session began stopping but its stop failed.',
	),
	quitAllFailureTitle: message(
		'desktop.tray.quitAll.failure.title',
		'Title of the report shown in the reopened window when Stop Session could not stop the Session.',
	),
	quitAllFailureUnconfirmed: message(
		'desktop.tray.quitAll.failure.unconfirmed',
		'Why Stop Session did not quit: the stop was asked for, but its completion could not be confirmed in time.',
	),
	quitAllFailureUnmanaged: message(
		'desktop.tray.quitAll.failure.unmanaged',
		'Why Stop Session left everything running: this Session was not started in a way that lets Orchbee stop it.',
	),
	quitAllStopping: message(
		'desktop.tray.quitAll.stopping',
		'First, inert line of the menu of the Windows notification-area icon, the macOS menu-bar icon or the Linux panel icon, and the tooltip status, while Stop Session stops the Session.',
	),
	quitAllTitle: message(
		'desktop.tray.quitAll.title',
		'Main instruction of the Stop Session confirmation dialog.',
	),
	quitKeepRunning: message(
		'desktop.tray.quitKeepRunning',
		'Last menu item of the Windows notification-area icon, the macOS menu-bar icon and the Linux panel icon, just below Stop Session: Orchbee quits while its terminals and Agents keep running in the background.',
	),
	status: message(
		'desktop.tray.status',
		'First, inert line of the menu of the Windows notification-area icon, the macOS menu-bar icon or the Linux panel icon, also used in the icon tooltip: how many terminals run, none included, and how many Agents failed, need attention, or finished a turn the person has not seen, naming them as Agents.',
	),
	tooltip: message(
		'desktop.tray.tooltip',
		'Tooltip of the Windows notification-area icon, the macOS menu-bar icon and the Linux panel icon. {application} is the application name; {status} is the status line.',
	),
});
