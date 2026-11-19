import {
	useOrchbeeIntl,
	websiteShellMessages as messages,
	type MessageDescriptor,
	type OrchbeeLocale,
} from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { pagePath, type RenderedPage, type WebsitePage } from '../../pages';
import { useLanguageLink } from '../../language-link/useLanguageLink';
import { REPOSITORY_URL, type Release } from '../../release';
import { BrandMark } from '../brand-mark/BrandMark';
import { LinkButton } from '../link-button/LinkButton';
import styles from './SiteHeader.module.css';

const LINKS: readonly {
	page: Exclude<WebsitePage, 'home'>;
	short: MessageDescriptor;
	full: MessageDescriptor;
	label: string;
}[] = [
	{
		page: 'terminal',
		short: messages.navTerminal,
		full: messages.clientTerminal,
		label: '/terminal',
	},
	{ page: 'desktop', short: messages.navDesktop, full: messages.clientDesktop, label: '/desktop' },
	{ page: 'cli', short: messages.navCli, full: messages.clientCli, label: '/cli' },
];

// The sticky navigation bar. At narrow widths its links move into a sheet that the menu button,
// shown by activation, opens.
export function SiteHeader({
	locale,
	page,
	release,
}: {
	locale: OrchbeeLocale;
	page: RenderedPage;
	release: Release;
}): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	const language = useLanguageLink(locale, page);
	const current = (target: WebsitePage): 'page' | undefined =>
		target === page ? 'page' : undefined;
	return (
		<header className={styles['header']} data-site-header=''>
			<a className={styles['skip']} href='#main'>
				{t(messages.skip)}
			</a>
			<div className={styles['bar']}>
				<BrandMark href={pagePath('home', locale)} />
				<nav className={styles['links']} aria-label={t(messages.navigation)}>
					{LINKS.map(({ page: target, short }) => (
						<a
							className={styles['link']}
							href={pagePath(target, locale)}
							aria-current={current(target)}
							key={target}
						>
							{t(short)}
						</a>
					))}
				</nav>
				<div className={styles['end']}>
					<a className={styles['wide']} href={REPOSITORY_URL} target='_blank' rel='noreferrer'>
						GitHub ↗
					</a>
					<a
						className={styles['wide']}
						href={language.href}
						hrefLang={language.locale}
						lang={language.locale}
						data-language-link=''
					>
						{language.label}
					</a>
					<span className={styles['download']}>
						<LinkButton
							href={`${pagePath('home', locale)}#install`}
							size='small'
							downloadTrigger={release.published}
						>
							{t(release.published ? messages.navDownload : messages.navInstall)}
						</LinkButton>
					</span>
					<button
						className={styles['menu']}
						type='button'
						aria-label={t(messages.menu)}
						aria-expanded='false'
						aria-controls='site-menu'
						data-site-menu=''
						hidden
					>
						<i className={styles['bars']} />
					</button>
				</div>
			</div>
			<div className={styles['sheet']} id='site-menu'>
				{LINKS.map(({ page: target, full, label }) => (
					<a
						className={styles['item']}
						href={pagePath(target, locale)}
						aria-current={current(target)}
						key={target}
					>
						<span>{t(full)}</span>
						<span className={styles['path']}>{label}</span>
					</a>
				))}
				<a className={styles['item']} href={REPOSITORY_URL} target='_blank' rel='noreferrer'>
					<span>GitHub</span>
					<span className={styles['path']}>↗</span>
				</a>
				<a
					className={styles['item']}
					href={language.href}
					hrefLang={language.locale}
					lang={language.locale}
					data-language-link=''
				>
					{language.label}
				</a>
			</div>
		</header>
	);
}
