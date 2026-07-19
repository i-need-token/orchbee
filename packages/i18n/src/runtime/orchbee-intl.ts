import { createIntl, createIntlCache, type IntlShape } from 'react-intl';

import { DEFAULT_ORCHBEE_LOCALE, type OrchbeeLocale } from '../locale';
import { getOrchbeeMessages } from './catalogs';

const intlCache = createIntlCache();

export function createOrchbeeIntl(locale: OrchbeeLocale): IntlShape {
	return createIntl(
		{
			defaultLocale: DEFAULT_ORCHBEE_LOCALE,
			locale,
			messages: getOrchbeeMessages(locale),
		},
		intlCache,
	);
}
