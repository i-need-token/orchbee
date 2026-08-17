export const enUSDesktopSecureKeyboardEntryMessages = {
	'desktop.secureKeyboardEntry.action.toggle': 'Secure Keyboard Entry',
	'desktop.secureKeyboardEntry.category': 'Input Security',
	'desktop.secureKeyboardEntry.unavailable': 'Secure Keyboard Entry is unavailable.',
} as const;

export type DesktopSecureKeyboardEntryMessageId =
	keyof typeof enUSDesktopSecureKeyboardEntryMessages;
