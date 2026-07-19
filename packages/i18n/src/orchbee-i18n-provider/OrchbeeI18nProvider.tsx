import { IntlProvider } from 'react-intl';
import type * as React from 'react';

import { DEFAULT_ORCHBEE_LOCALE, type OrchbeeLocale } from '../locale';
import { getOrchbeeMessages } from '../runtime';

export interface OrchbeeI18nProviderProps {
	readonly children?: React.ReactNode;
	readonly locale: OrchbeeLocale;
}

export function OrchbeeI18nProvider({
	children,
	locale,
}: OrchbeeI18nProviderProps): React.ReactNode {
	return (
		<IntlProvider
			defaultLocale={DEFAULT_ORCHBEE_LOCALE}
			locale={locale}
			messages={getOrchbeeMessages(locale)}
		>
			{children}
		</IntlProvider>
	);
}
