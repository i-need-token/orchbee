import type { OrchbeeLocale } from '../locale';
import { enUSMessages, zhCNMessages } from '../locales';

const messagesByLocale = {
	'en-US': enUSMessages,
	'zh-CN': zhCNMessages,
} satisfies Readonly<Record<OrchbeeLocale, Readonly<Record<string, string>>>>;

export function getOrchbeeMessages(locale: OrchbeeLocale): Readonly<Record<string, string>> {
	return messagesByLocale[locale];
}
