import type { OrchbeeLocale } from '@orchbee/i18n';
import type { ReactElement, ReactNode } from 'react';

import type { RenderedPage } from '../../pages';
import type { Release } from '../../release';
import { DownloadDialog } from '../download-dialog/DownloadDialog';
import { Lattice } from '../lattice/Lattice';
import { SiteFooter } from '../site-footer/SiteFooter';
import { SiteHeader } from '../site-header/SiteHeader';
import styles from './PageShell.module.css';

// The chrome every page shares: the navigation bar, the hexagon lattice behind the top of the page,
// and the footer. The page's own content goes between them.
export function PageShell({
	locale,
	page,
	release,
	children,
}: {
	locale: OrchbeeLocale;
	page: RenderedPage;
	release: Release;
	children: ReactNode;
}): ReactElement {
	return (
		<div className={styles['root']}>
			<SiteHeader locale={locale} page={page} release={release} />
			<main className={styles['main']} id='main'>
				<Lattice />
				<div className={styles['page']}>{children}</div>
			</main>
			<SiteFooter locale={locale} page={page} />
			<DownloadDialog release={release} />
		</div>
	);
}
