import {
	useOrchbeeIntl,
	websiteShellMessages as messages,
	type MessageDescriptor,
	type OrchbeeLocale,
} from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { pagePath, type WebsitePage } from '../../pages';
import { richText } from '../../rich-text';
import styles from './ClientIndex.module.css';

type ClientPage = Exclude<WebsitePage, 'home'>;

const CLIENTS: readonly {
	page: ClientPage;
	name: MessageDescriptor;
	summary: MessageDescriptor;
	label: string;
}[] = [
	{
		page: 'terminal',
		name: messages.clientTerminal,
		summary: messages.clientTerminalSummary,
		label: '/terminal',
	},
	{
		page: 'desktop',
		name: messages.clientDesktop,
		summary: messages.clientDesktopSummary,
		label: '/desktop',
	},
	{ page: 'cli', name: messages.clientCli, summary: messages.clientCliSummary, label: '/cli' },
];

// The cards that lead to the client pages, side by side, leaving out the page the visitor is on.
export function ClientIndex({
	locale,
	current,
}: {
	locale: OrchbeeLocale;
	current?: ClientPage;
}): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	return (
		<div className={styles['index']} data-reveal=''>
			{CLIENTS.filter(({ page }) => page !== current).map(({ page, name, summary, label }) => (
				<a className={styles['card']} href={pagePath(page, locale)} key={page}>
					<h3 className={styles['name']}>{t(name)}</h3>
					<p className={styles['summary']}>{t(summary, richText)}</p>
					<span className={styles['go']}>
						{label}
						<svg className={styles['hexagon']} viewBox='0 0 22 20' aria-hidden='true'>
							<path d='M6 1h10l5 9-5 9H6L1 10Z' />
						</svg>
					</span>
				</a>
			))}
		</div>
	);
}
