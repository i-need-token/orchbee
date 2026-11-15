import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { flushSync } from 'react-dom';
import { createRoot } from 'react-dom/client';

import { pageAtPath, release, Website } from '../website';

// The development server serves one empty document for every address, so the page for the
// address is rendered here, in place of the build's static rendering.
export function renderDevelopmentPage(): void {
	const root = document.getElementById('root');
	if (root === null) throw new Error('The website mount point is missing.');
	const { pathname } = window.location;
	const found = pageAtPath(pathname);
	const locale = found?.locale ?? (pathname.startsWith('/zh-CN') ? 'zh-CN' : 'en-US');
	document.documentElement.lang = locale;
	flushSync(() =>
		createRoot(root).render(
			<OrchbeeI18nProvider locale={locale}>
				<Website locale={locale} page={found?.page ?? 'not-found'} release={release} />
			</OrchbeeI18nProvider>,
		),
	);
}
