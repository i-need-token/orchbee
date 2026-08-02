import type { OrchbeeMessageId } from '../en-US';
import { zhCNDesktopApplicationLifetimeMessages } from './desktop-application-lifetime';
import { zhCNDesktopTerminalMessages } from './desktop-terminal';
import { zhCNWorkbenchMessages } from './workbench';

export const zhCNMessages = {
	...zhCNDesktopApplicationLifetimeMessages,
	...zhCNDesktopTerminalMessages,
	...zhCNWorkbenchMessages,
} as const satisfies Readonly<Record<OrchbeeMessageId, string>>;
