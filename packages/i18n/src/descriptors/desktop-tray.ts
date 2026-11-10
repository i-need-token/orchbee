import { defineMessages } from 'react-intl';

import { enUSDesktopTrayMessages } from '../locales';
import { createMessageDescriptorFactory } from './createMessageDescriptorFactory';

const message = createMessageDescriptorFactory(enUSDesktopTrayMessages);

export const desktopTrayMessages = defineMessages({
	noticeBody: message(
		'desktop.tray.notice.body',
		'Body of the one-time Windows notification shown when closing the window leaves Orchbee running in the notification area. {terminals} is the number of terminals still running.',
	),
	noticeTitle: message(
		'desktop.tray.notice.title',
		'Title of the one-time notification. {application} is the application name, such as Orchbee or Orchbee Test.',
	),
	quitKeepRunning: message(
		'desktop.tray.quitKeepRunning',
		'Notification-area menu item: Orchbee quits while its terminals and Agents keep running in the background.',
	),
	reopen: message(
		'desktop.tray.reopen',
		'Notification-area menu item that shows the window again.',
	),
	status: message(
		'desktop.tray.status',
		'First, inert line of the notification-area menu, also used in its tooltip: how many terminals run, and how many Agents failed, need attention, or finished a turn the person has not seen.',
	),
	tooltip: message(
		'desktop.tray.tooltip',
		'Tooltip of the notification-area icon. {application} is the application name; {status} is the status line.',
	),
});
