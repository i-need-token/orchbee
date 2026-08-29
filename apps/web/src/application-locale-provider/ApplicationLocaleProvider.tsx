import { OrchbeeI18nProvider } from '@orchbee/i18n';
import type * as React from 'react';

import { type ApplicationLocaleSource, useApplicationLocale } from '../application-locale';

export interface ApplicationLocaleProviderProps {
	readonly children?: React.ReactNode;
	readonly source: ApplicationLocaleSource;
}

export function ApplicationLocaleProvider({
	children,
	source,
}: ApplicationLocaleProviderProps): React.ReactNode {
	const { effectiveLocale } = useApplicationLocale(source);
	return <OrchbeeI18nProvider locale={effectiveLocale}>{children}</OrchbeeI18nProvider>;
}
