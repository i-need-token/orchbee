export const enUSDesktopDiagnosticsMessages = {
	'desktop.diagnostics.action.saveBundle': 'Save Diagnostic Bundle…',
	'desktop.diagnostics.bundleFailed': "Couldn't save the diagnostic bundle.",
	'desktop.diagnostics.bundleFailedDetail': 'Nothing was uploaded.',
	'desktop.diagnostics.category': 'Help',
	'desktop.diagnostics.close': 'Close',
	'desktop.diagnostics.saveDialogTitle': 'Save Diagnostic Bundle',
	'desktop.diagnostics.taskDescription':
		'Save versions, settings without secrets, and the last 7 days of logs to a ZIP file you choose.',
	'desktop.diagnostics.zipFilter': 'ZIP archive',
} as const;

export type DesktopDiagnosticsMessageId = keyof typeof enUSDesktopDiagnosticsMessages;
