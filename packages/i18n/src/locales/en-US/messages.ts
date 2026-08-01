import { enUSDesktopTerminalMessages } from './desktop-terminal';
import { enUSWorkbenchMessages } from './workbench';

export const enUSMessages = {
	...enUSDesktopTerminalMessages,
	...enUSWorkbenchMessages,
} as const;

export type OrchbeeMessageId = keyof typeof enUSMessages;
