export const enUSDesktopTrayMessages = {
	'desktop.tray.notice.body':
		'{terminals, plural, one {# terminal keeps working.} other {# terminals keep working.}} Click the Orchbee icon at the lower right to reopen it, or right-click it to quit.',
	'desktop.tray.notice.title': '{application} is still running in the background',
	'desktop.tray.quitKeepRunning': 'Quit Orchbee, Keep Terminals Running',
	'desktop.tray.reopen': 'Reopen Orchbee',
	'desktop.tray.status':
		'{terminals, plural, one {# terminal running} other {# terminals running}}{failed, plural, =0 {} other { · # failed}}{attention, plural, =0 {} one { · # needs attention} other { · # need attention}}{done, plural, =0 {} other { · # finished}}',
	'desktop.tray.tooltip': '{application} · {status}',
} as const;

export type DesktopTrayMessageId = keyof typeof enUSDesktopTrayMessages;
