import { enUSDesktopApplicationLifetimeMessages } from './desktop-application-lifetime';
import { enUSDesktopTerminalMessages } from './desktop-terminal';
import { enUSWorkbenchMessages } from './workbench';

export const enUSMessages = {
	...enUSDesktopApplicationLifetimeMessages,
	...enUSDesktopTerminalMessages,
	...enUSWorkbenchMessages,
} as const;

export type OrchbeeMessageId = keyof typeof enUSMessages;
