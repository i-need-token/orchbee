import type { OrchbeeLocale } from '@orchbee/i18n';
import type { ReactElement, ReactNode } from 'react';

import type { Release } from '../../release';
import { IconSprite } from '../icon-sprite/IconSprite';
import { SiteFooter } from '../site-footer/SiteFooter';
import { SiteHeader } from '../site-header/SiteHeader';
import styles from './PageShell.module.css';

// The document styles, the background swarm, the header and the footer every page shares.
export function PageShell({
	locale,
	release,
	children,
}: {
	locale: OrchbeeLocale;
	release: Release;
	children: ReactNode;
}): ReactElement {
	const home = locale === 'zh-CN' ? '/zh-CN/' : '/';
	return (
		<div className={styles['page']} id='top'>
			<IconSprite />
			<canvas className={styles['swarm']} data-swarm='' aria-hidden='true' />
			<SiteHeader
				locale={locale}
				home={home}
				alternate={locale === 'zh-CN' ? '/' : '/zh-CN/'}
				release={release}
			/>
			<main className={styles['main']} id='main'>
				{children}
			</main>
			<SiteFooter home={home} />
		</div>
	);
}
