export const enUSDesktopTrayMessages = {
	'desktop.tray.notice.body':
		'{terminals, plural, one {# terminal keeps working.} other {# terminals keep working.}} Click the Orchbee icon at the lower right to reopen it, or right-click it for Quit All.',
	'desktop.tray.notice.title': '{application} is still running in the background',
	'desktop.tray.quitAll': 'Quit All…',
	'desktop.tray.quitAll.cancel': 'Cancel',
	'desktop.tray.quitAll.confirm': 'Stop All and Quit',
	'desktop.tray.quitAll.detail':
		'{terminals, plural, one {# terminal will stop} other {# terminals will stop}}{agents, plural, =0 {.} one {, including # working Agent.} other {, including # working Agents.}} The layout and working directories are kept; screen contents and the Agents’ work in progress are not. Orchbee also closes in terminal windows connected to this Session.',
	'desktop.tray.quitAll.failure.busy': 'Another stop of this Session is already under way.',
	'desktop.tray.quitAll.failure.close': 'Close',
	'desktop.tray.quitAll.failure.notSent':
		'Orchbee could not reach the Session, so nothing was stopped.',
	'desktop.tray.quitAll.failure.refused': 'The Session declined the stop, so nothing was stopped.',
	'desktop.tray.quitAll.failure.stopFailed':
		'The Session started stopping but could not finish, so some terminals may still be running. Retry with server stop --retry and request ID {stopId}.',
	'desktop.tray.quitAll.failure.title': 'Orchbee could not stop all terminals',
	'desktop.tray.quitAll.failure.unconfirmed':
		'Orchbee could not confirm that the Session stopped, so some terminals may still be running. Check with server stop-status and request ID {stopId}.',
	'desktop.tray.quitAll.failure.unmanaged': 'This Session can’t be stopped from Orchbee.',
	'desktop.tray.quitAll.stopping': 'Stopping all terminals…',
	'desktop.tray.quitAll.title': 'Stop all terminals and quit Orchbee?',
	'desktop.tray.quitKeepRunning': 'Quit Orchbee, Keep Terminals Running',
	'desktop.tray.reopen': 'Reopen Orchbee',
	'desktop.tray.status':
		'{terminals, plural, one {# terminal running} other {# terminals running}}{failed, plural, =0 {} other { · # failed}}{attention, plural, =0 {} one { · # needs attention} other { · # need attention}}{done, plural, =0 {} other { · # finished}}',
	'desktop.tray.tooltip': '{application} · {status}',
} as const;

export type DesktopTrayMessageId = keyof typeof enUSDesktopTrayMessages;
