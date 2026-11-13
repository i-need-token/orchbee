import type { OrchbeeLocale } from '@orchbee/i18n';
import type { ReactElement, ReactNode } from 'react';

import { SiteFooter } from '../site-footer/SiteFooter';
import { SiteHeader } from '../site-header/SiteHeader';
import styles from './PageShell.module.css';

// The document styles, header and footer every page of the site shares.
export function PageShell({
	locale,
	children,
}: {
	locale: OrchbeeLocale;
	children: ReactNode;
}): ReactElement {
	const home = locale === 'zh-CN' ? '/zh-CN/' : '/';
	return (
		<div className={styles['page']}>
			<SiteHeader
				locale={locale}
				home={home}
				alternate={locale === 'zh-CN' ? '/' : '/zh-CN/'}
				installHref={`${home}#install`}
			/>
			<main id='main'>{children}</main>
			<SiteFooter home={home} installHref={`${home}#install`} />
		</div>
	);
}
