export const enUSDesktopTrayMessages = {
	'desktop.tray.notice.body':
		'{terminals, plural, one {# terminal keeps working.} other {# terminals keep working.}} Click the Orchbee icon at the lower right to reopen it, or right-click it to stop the Session.',
	'desktop.tray.notice.menuBarBody':
		'{terminals, plural, one {# terminal keeps working.} other {# terminals keep working.}} Click Orchbee in the Dock to reopen it, or click the Orchbee icon in the menu bar to stop the Session.',
	'desktop.tray.notice.panelBody':
		'{terminals, plural, one {# terminal keeps working.} other {# terminals keep working.}} Open Orchbee again to return to it, or use the Orchbee icon in the panel to stop the Session.',
	'desktop.tray.notice.title': '{application} is still running in the background',
	'desktop.tray.open': 'Open Orchbee',
	'desktop.tray.quitAll': 'Stop Session…',
	'desktop.tray.quitAll.cancel': 'Cancel',
	'desktop.tray.quitAll.confirm': 'Stop and Quit',
	'desktop.tray.quitAll.detail':
		'{terminals, plural, one {# terminal will stop} other {# terminals will stop}}{agents, plural, =0 {.} one {, including # working Agent.} other {, including # working Agents.}} The layout and working directories are kept; screen contents and the Agents’ work in progress are not. Orchbee also closes in terminal windows connected to this Session.',
	'desktop.tray.quitAll.failure.busy': 'Another stop of this Session is already under way.',
	'desktop.tray.quitAll.failure.close': 'Close',
	'desktop.tray.quitAll.failure.notSent': "Couldn't reach the Session, so nothing was stopped.",
	'desktop.tray.quitAll.failure.refused': 'The Session declined the stop, so nothing was stopped.',
	'desktop.tray.quitAll.failure.stopFailed':
		"The Session started stopping but couldn't finish, so some terminals may still be running.",
	'desktop.tray.quitAll.failure.title': "Couldn't stop the Session",
	'desktop.tray.quitAll.failure.unconfirmed': 'Not sure it finished. Check, then try again.',
	'desktop.tray.quitAll.failure.unmanaged': 'This Session can’t be stopped from Orchbee.',
	'desktop.tray.quitAll.stopping': 'Stopping the Session…',
	'desktop.tray.quitAll.title': 'Stop the Session and quit Orchbee?',
	'desktop.tray.quitKeepRunning': 'Quit Orchbee',
	'desktop.tray.status':
		'{terminals, plural, =0 {No terminals running} one {# terminal running} other {# terminals running}}{failed, plural, =0 {} one { · # Agent failed} other { · # Agents failed}}{attention, plural, =0 {} one { · # Agent needs attention} other { · # Agents need attention}}{done, plural, =0 {} one { · # Agent finished} other { · # Agents finished}}',
	'desktop.tray.tooltip': '{application} · {status}',
} as const;

export type DesktopTrayMessageId = keyof typeof enUSDesktopTrayMessages;
