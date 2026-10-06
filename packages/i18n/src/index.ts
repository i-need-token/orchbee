export { FormattedMessage } from 'react-intl';
export type { IntlShape, MessageDescriptor } from 'react-intl';

export {
	agentIntegrationMessages,
	agentObservationMessages,
	applicationRoutingMessages,
	desktopApplicationLifetimeMessages,
	desktopSecureKeyboardEntryMessages,
	desktopWindowMessages,
	keyboardShortcutMessages,
	projectedTerminalMessages,
	terminalSettingsMessages,
	terminalSettingsCatalogMessages,
	workbenchMessages,
	workbenchTabMessages,
} from './descriptors';
export type { OrchbeeMessageId } from './locales';
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
