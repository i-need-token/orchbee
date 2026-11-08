export const enUSDesktopDiagnosticsMessages = {
	'desktop.diagnostics.action.saveBundle': 'Save Diagnostic Bundle…',
	'desktop.diagnostics.bundleFailed': 'The diagnostic bundle could not be saved.',
	'desktop.diagnostics.bundleFailedDetail':
		'Nothing was uploaded. To see why, run the "orchbee diagnostics create" command in a terminal.',
	'desktop.diagnostics.bundlePending': 'A diagnostic bundle is being saved.',
	'desktop.diagnostics.category': 'Help',
	'desktop.diagnostics.close': 'Close',
	'desktop.diagnostics.saveDialogTitle': 'Save Diagnostic Bundle',
	'desktop.diagnostics.taskDescription':
		'Save versions, settings without secrets, and the last 7 days of logs to a ZIP file you choose.',
	'desktop.diagnostics.zipFilter': 'ZIP archive',
} as const;

export type DesktopDiagnosticsMessageId = keyof typeof enUSDesktopDiagnosticsMessages;
