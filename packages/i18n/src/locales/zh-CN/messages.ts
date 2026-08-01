import type { OrchbeeMessageId } from '../en-US';
import { zhCNDesktopTerminalMessages } from './desktop-terminal';
import { zhCNWorkbenchMessages } from './workbench';

export const zhCNMessages = {
	...zhCNDesktopTerminalMessages,
	...zhCNWorkbenchMessages,
} as const satisfies Readonly<Record<OrchbeeMessageId, string>>;
