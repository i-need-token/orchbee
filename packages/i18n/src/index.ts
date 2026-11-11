export type { IntlShape, MessageDescriptor } from 'react-intl';

export {
	agentIntegrationMessages,
	agentObservationMessages,
	agentStartMessages,
	applicationRoutingMessages,
	desktopDiagnosticsMessages,
	desktopSecureKeyboardEntryMessages,
	desktopTrayMessages,
	desktopWindowMessages,
	keyboardShortcutMessages,
	projectedTerminalMessages,
	sessionConnectionMessages,
	terminalSettingsMessages,
	workbenchMessages,
	workbenchTabMessages,
} from './descriptors';

export { getOrchbeeTextDirection, ORCHBEE_LOCALES, resolveOrchbeeLocale } from './locale';
export type { OrchbeeLocale } from './locale';
export { OrchbeeI18nProvider } from './orchbee-i18n-provider';

export { createOrchbeeIntl, useOrchbeeIntl } from './runtime';
export { websiteMessages } from './descriptors';
