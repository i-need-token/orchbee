import type { DesktopSecureKeyboardEntryMessageId } from '../en-US';

export const zhCNDesktopSecureKeyboardEntryMessages = {
	'desktop.secureKeyboardEntry.action.toggle': '安全键盘输入',
	'desktop.secureKeyboardEntry.category': '输入安全',
} as const satisfies Readonly<Record<DesktopSecureKeyboardEntryMessageId, string>>;
