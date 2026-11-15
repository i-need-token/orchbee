import type { OrchbeeLocale } from '@orchbee/i18n';
import type { ReactElement } from 'react';

import type { Release } from '../../release';
import { Clients } from './components/clients/Clients';
import { Hero } from './components/hero/Hero';
import { Install } from './components/install/Install';

// The home page, section by section. Each section is a private component under `components/`.
export function HomePage({
	locale,
	release,
}: {
	locale: OrchbeeLocale;
	release: Release;
}): ReactElement {
	return (
		<>
			<Hero release={release} />
			<Clients locale={locale} />
			<Install release={release} />
		</>
	);
}
