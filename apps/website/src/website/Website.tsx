import type { OrchbeeLocale } from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { Home } from './components/home/Home';
import { NotFound } from './components/not-found/NotFound';
import { PageShell } from './components/page-shell/PageShell';
import type { Release } from './release';

// One page of the site: the home page, or the page served for an address the site does not have.
export function Website({
	locale,
	release,
	page = 'home',
}: {
	locale: OrchbeeLocale;
	release: Release;
	page?: 'home' | 'not-found';
}): ReactElement {
	return (
		<PageShell locale={locale}>
			{page === 'home' ? <Home release={release} /> : <NotFound locale={locale} />}
		</PageShell>
	);
}
