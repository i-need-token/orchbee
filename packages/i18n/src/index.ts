export { FormattedMessage } from 'react-intl';
export type { IntlShape, MessageDescriptor } from 'react-intl';

export { webMessages, workbenchMessages } from './descriptors';
export type { OrchbeeMessageId } from './descriptors';
export {
	DEFAULT_ORCHBEE_LOCALE,
	getOrchbeeTextDirection,
	ORCHBEE_LOCALES,
	resolveOrchbeeLocale,
} from './locale';
export type { OrchbeeLocale, OrchbeeTextDirection } from './locale';
export { OrchbeeI18nProvider } from './orchbee-i18n-provider';
export type { OrchbeeI18nProviderProps } from './orchbee-i18n-provider';
export { createOrchbeeIntl, useOrchbeeIntl } from './runtime';
