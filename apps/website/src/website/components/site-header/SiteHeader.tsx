import { useOrchbeeIntl, websiteMessages as messages, type OrchbeeLocale } from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { REPOSITORY_URL } from '../../release';
import styles from './SiteHeader.module.css';

export function SiteHeader({
	locale,
	home,
	alternate,
	installHref,
}: {
	locale: OrchbeeLocale;
	home: string;
	alternate: string;
	installHref: string;
}): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	const otherLocale = locale === 'zh-CN' ? 'en-US' : 'zh-CN';
	return (
		<header className={styles['header']}>
			<a className={styles['skip']} href='#main'>
				{t(messages.skip)}
			</a>
			<div className={styles['inner']}>
				<a className={styles['brand']} href={home}>
					<img src='/icon.svg' width='26' height='26' alt='' />
					Orchbee
				</a>
				<nav className={styles['navigation']} aria-label={t(messages.navigation)}>
					<a className={styles['link']} href={REPOSITORY_URL}>
						GitHub
					</a>
					<a
						className={styles['toggle']}
						href={alternate}
						hrefLang={otherLocale}
						lang={otherLocale}
						aria-label={t(messages.languageSwitchLabel)}
						data-language-link=''
					>
						{t(messages.languageSwitch)}
					</a>
					<button
						className={styles['toggle']}
						type='button'
						data-appearance=''
						data-label-system={t(messages.themeSystem)}
						data-label-light={t(messages.themeLight)}
						data-label-dark={t(messages.themeDark)}
						aria-label={t(messages.themeSystem)}
						title={t(messages.themeSystem)}
						hidden
					>
						<svg
							className={styles['iconSystem']}
							width='16'
							height='16'
							viewBox='0 0 24 24'
							fill='none'
							stroke='currentColor'
							strokeWidth='1.8'
							strokeLinecap='round'
							strokeLinejoin='round'
							aria-hidden='true'
						>
							<rect x='3' y='4' width='18' height='12' rx='2' />
							<path d='M8 20h8M12 16v4' />
						</svg>
						<svg
							className={styles['iconLight']}
							width='16'
							height='16'
							viewBox='0 0 24 24'
							fill='none'
							stroke='currentColor'
							strokeWidth='1.8'
							strokeLinecap='round'
							strokeLinejoin='round'
							aria-hidden='true'
						>
							<circle cx='12' cy='12' r='4' />
							<path d='M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4' />
						</svg>
						<svg
							className={styles['iconDark']}
							width='16'
							height='16'
							viewBox='0 0 24 24'
							fill='none'
							stroke='currentColor'
							strokeWidth='1.8'
							strokeLinecap='round'
							strokeLinejoin='round'
							aria-hidden='true'
						>
							<path d='M20 14.5A8 8 0 0 1 9.5 4a8 8 0 1 0 10.5 10.5Z' />
						</svg>
					</button>
					<a className={styles['cta']} href={installHref}>
						{t(messages.getOrchbee)}
					</a>
				</nav>
			</div>
		</header>
	);
}
