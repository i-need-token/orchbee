import type { OrchbeeMessageId } from '../en-US';
import { zhCNApplicationRoutingMessages } from './application-routing';
import { zhCNDesktopApplicationLifetimeMessages } from './desktop-application-lifetime';
import { zhCNProjectedTerminalMessages } from './projected-terminal';
import { zhCNWorkbenchMessages } from './workbench';

export const zhCNMessages = {
	...zhCNApplicationRoutingMessages,
	...zhCNDesktopApplicationLifetimeMessages,
	...zhCNProjectedTerminalMessages,
	...zhCNWorkbenchMessages,
} as const satisfies Readonly<Record<OrchbeeMessageId, string>>;
