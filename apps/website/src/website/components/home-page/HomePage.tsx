import type { OrchbeeLocale } from '@orchbee/i18n';
import type { ReactElement } from 'react';

import type { Release } from '../../release';
import { Agents } from './components/agents/Agents';
import { Benefits } from './components/benefits/Benefits';
import { Clients } from './components/clients/Clients';
import { Faq } from './components/faq/Faq';
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
			<Hero locale={locale} release={release} />
			<Benefits />
			<Agents />
			<Clients locale={locale} />
			<Faq release={release} />
			<Install release={release} />
		</>
	);
}
