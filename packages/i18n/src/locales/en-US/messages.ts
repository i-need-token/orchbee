import { enUSApplicationRoutingMessages } from './application-routing';
import { enUSDesktopApplicationLifetimeMessages } from './desktop-application-lifetime';
import { enUSProjectedTerminalMessages } from './projected-terminal';
import { enUSWorkbenchMessages } from './workbench';

export const enUSMessages = {
	...enUSApplicationRoutingMessages,
	...enUSDesktopApplicationLifetimeMessages,
	...enUSProjectedTerminalMessages,
	...enUSWorkbenchMessages,
} as const;

export type OrchbeeMessageId = keyof typeof enUSMessages;
