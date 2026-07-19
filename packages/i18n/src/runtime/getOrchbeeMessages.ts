import type { OrchbeeMessageId } from '../descriptors';
import type { OrchbeeLocale } from '../locale';
import {
	enUSWebMessages,
	enUSWorkbenchMessages,
	zhCNWebMessages,
	zhCNWorkbenchMessages,
} from '../locales';

const enUSMessages = {
	...enUSWebMessages,
	...enUSWorkbenchMessages,
} satisfies Readonly<Record<OrchbeeMessageId, string>>;

const zhCNMessages = {
	...zhCNWebMessages,
	...zhCNWorkbenchMessages,
} satisfies Readonly<Record<OrchbeeMessageId, string>>;

const messagesByLocale = {
	'en-US': enUSMessages,
	'zh-CN': zhCNMessages,
} satisfies Readonly<Record<OrchbeeLocale, Readonly<Record<OrchbeeMessageId, string>>>>;

export function getOrchbeeMessages(locale: OrchbeeLocale): Readonly<Record<string, string>> {
	return messagesByLocale[locale];
}
