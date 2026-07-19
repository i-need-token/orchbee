import { enUSWorkbenchMessages } from './workbench';

export const enUSMessages = {
	...enUSWorkbenchMessages,
} as const;

export type OrchbeeMessageId = keyof typeof enUSMessages;
