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
	quitAll: message(
		'desktop.tray.quitAll',
		'Notification-area menu item that asks to stop the Session, ending every terminal and Agent, and then quit Orchbee.',
	),
	quitAllCancel: message(
		'desktop.tray.quitAll.cancel',
		'Button of the Quit All confirmation that leaves everything running; it has the focus and answers Esc.',
	),
	quitAllConfirm: message(
		'desktop.tray.quitAll.confirm',
		'Button of the Quit All confirmation that stops every terminal and quits Orchbee.',
	),
	quitAllDetail: message(
		'desktop.tray.quitAll.detail',
		'Body of the Quit All confirmation. {terminals} is the number of running terminals that stop; {agents} is how many Agents are working in them.',
	),
	quitAllFailureBusy: message(
		'desktop.tray.quitAll.failure.busy',
		'Why Quit All left everything running: another stop of the same Session is already under way.',
	),
	quitAllFailureClose: message(
		'desktop.tray.quitAll.failure.close',
		'Button that dismisses the Quit All failure report.',
	),
	quitAllFailureNotSent: message(
		'desktop.tray.quitAll.failure.notSent',
		'Why Quit All left everything running: the window could not reach the Session, so nothing was asked to stop.',
	),
	quitAllFailureRefused: message(
		'desktop.tray.quitAll.failure.refused',
		'Why Quit All left everything running: the Session declined the stop.',
	),
	quitAllFailureStopFailed: message(
		'desktop.tray.quitAll.failure.stopFailed',
		'Why Quit All did not quit: the Session began stopping but its stop failed. {stopId} is the stop request ID that `server status` and `server stop --retry` take.',
	),
	quitAllFailureTitle: message(
		'desktop.tray.quitAll.failure.title',
		'Title of the report shown in the reopened window when Quit All could not stop the Session.',
	),
	quitAllFailureUnconfirmed: message(
		'desktop.tray.quitAll.failure.unconfirmed',
		'Why Quit All did not quit: the stop was asked for, but its completion could not be confirmed in time. {stopId} is the stop request ID that `server status` takes.',
	),
	quitAllFailureUnmanaged: message(
		'desktop.tray.quitAll.failure.unmanaged',
		'Why Quit All left everything running: this Session was not started in a way that lets Orchbee stop it.',
	),
	quitAllStopping: message(
		'desktop.tray.quitAll.stopping',
		'First, inert line of the notification-area menu and the tooltip status while Quit All stops the Session.',
	),
	quitAllTitle: message(
		'desktop.tray.quitAll.title',
		'Main instruction of the Quit All confirmation dialog.',
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
