export const enUSDesktopTerminalMessages = {
	'desktop.terminal.connectionFailure': 'The terminal connection failed.',
	'desktop.terminal.input': 'Terminal input',
	'desktop.terminal.screen': 'Terminal screen',
} as const;

export type DesktopTerminalMessageId = keyof typeof enUSDesktopTerminalMessages;
