import type { OrchbeeMessageId } from '../en-US';
import { zhCNApplicationRoutingMessages } from './application-routing';
import { zhCNDesktopApplicationLifetimeMessages } from './desktop-application-lifetime';
import { zhCNDesktopBrowserAccessMessages } from './desktop-browser-access';
import { zhCNProjectedTerminalMessages } from './projected-terminal';
import { zhCNWorkbenchMessages } from './workbench';

export const zhCNMessages = {
	...zhCNApplicationRoutingMessages,
	...zhCNDesktopApplicationLifetimeMessages,
	...zhCNDesktopBrowserAccessMessages,
	...zhCNProjectedTerminalMessages,
	...zhCNWorkbenchMessages,
} as const satisfies Readonly<Record<OrchbeeMessageId, string>>;
