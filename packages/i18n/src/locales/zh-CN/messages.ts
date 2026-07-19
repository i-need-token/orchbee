import type { OrchbeeMessageId } from '../en-US';
import { zhCNWorkbenchMessages } from './workbench';

export const zhCNMessages = {
	...zhCNWorkbenchMessages,
} as const satisfies Readonly<Record<OrchbeeMessageId, string>>;
