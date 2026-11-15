import type { OrchbeeLocale } from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { CliPage } from './components/cli-page/CliPage';
import { DesktopPage } from './components/desktop-page/DesktopPage';
import { HomePage } from './components/home-page/HomePage';
import { NotFoundPage } from './components/not-found-page/NotFoundPage';
import { PageShell } from './components/page-shell/PageShell';
import { TerminalPage } from './components/terminal-page/TerminalPage';
import type { RenderedPage } from './pages';
import type { Release } from './release';

function PageContent({
	locale,
	page,
	release,
}: {
	locale: OrchbeeLocale;
	page: RenderedPage;
	release: Release;
}): ReactElement {
	switch (page) {
		case 'home':
			return <HomePage locale={locale} release={release} />;
		case 'terminal':
			return <TerminalPage locale={locale} release={release} />;
		case 'desktop':
			return <DesktopPage locale={locale} release={release} />;
		case 'cli':
			return <CliPage locale={locale} release={release} />;
		case 'not-found':
			return <NotFoundPage locale={locale} />;
	}
}

// One page of the site in its shared chrome.
export function Website({
	locale,
	page,
	release,
}: {
	locale: OrchbeeLocale;
	page: RenderedPage;
	release: Release;
}): ReactElement {
	return (
		<PageShell locale={locale} page={page} release={release}>
			<PageContent locale={locale} page={page} release={release} />
		</PageShell>
	);
}
