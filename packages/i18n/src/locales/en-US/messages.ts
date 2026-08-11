import { enUSApplicationRoutingMessages } from './application-routing';
import { enUSDesktopApplicationLifetimeMessages } from './desktop-application-lifetime';
import { enUSDesktopBrowserAccessMessages } from './desktop-browser-access';
import { enUSProjectedTerminalMessages } from './projected-terminal';
import { enUSWorkbenchMessages } from './workbench';

export const enUSMessages = {
	...enUSApplicationRoutingMessages,
	...enUSDesktopApplicationLifetimeMessages,
	...enUSDesktopBrowserAccessMessages,
	...enUSProjectedTerminalMessages,
	...enUSWorkbenchMessages,
} as const;

export type OrchbeeMessageId = keyof typeof enUSMessages;
