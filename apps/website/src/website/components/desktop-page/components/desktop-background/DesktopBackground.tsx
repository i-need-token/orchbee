import {
	desktopTrayMessages,
	useOrchbeeIntl,
	websiteDesktopMessages as messages,
} from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { Kicker } from '../../../kicker/Kicker';
import { DetailDuo } from '../detail-duo/DetailDuo';
import styles from './DesktopBackground.module.css';

// What the figure shows: three terminals still running after the window closed, one Agent waiting
// for an answer and one that finished a turn not yet seen.
const TERMINALS = 3;
const ATTENTION = 1;
const DONE = 1;

// The state badge of the notification-area icon: a pointy-top hexagon in the lower-right corner of
// the mark, outlined in the mark's ink (TRAY_STATUS_HEXAGON in scripts/brand-assets.mjs, which
// renders the icons the application shows).
const STATE_HEXAGON = 'M760 520 967.8 640V880L760 1000 552.2 880V640Z';
const STATE_OUTLINE = 48;

/** The mark as the notification-area icon draws it, with the attention badge. */
function TrayIcon(): ReactElement {
	return (
		<span className={styles['trayIcon']}>
			<img className={styles['mark']} src='/icon.svg' alt='' width='18' height='18' />
			<svg className={styles['badge']} viewBox='0 0 1024 1024' aria-hidden='true'>
				<path d={STATE_HEXAGON} strokeWidth={STATE_OUTLINE} strokeLinejoin='round' />
			</svg>
		</span>
	);
}

/**
 * Running in the background: what closing the window does, beside the Windows notification it
 * shows and the notification-area menu, both in the application's own words.
 */
export function DesktopBackground(): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	return (
		<DetailDuo
			first={
				<>
					<Kicker>{t(messages.backgroundKicker)}</Kicker>
					<h2 className={styles['title']}>{t(messages.backgroundTitle)}</h2>
					<p className={styles['paragraph']}>{t(messages.backgroundBody)}</p>
					<p className={styles['paragraph']}>{t(messages.backgroundQuitAll)}</p>
				</>
			}
			second={
				<div className={styles['figure']} role='img' aria-label={t(messages.trayFigureLabel)}>
					<p className={styles['label']}>{t(messages.trayWhenClosed)}</p>
					<div className={styles['notice']}>
						<img className={styles['mark']} src='/icon.svg' alt='' width='18' height='18' />
						<div>
							<b>{t(desktopTrayMessages.noticeTitle, { application: 'Orchbee' })}</b>
							<span>{t(desktopTrayMessages.noticeBody, { terminals: TERMINALS })}</span>
						</div>
					</div>
					<p className={styles['label']}>{t(messages.trayWhenMenu)}</p>
					<div className={styles['menu']}>
						<span className={styles['disabled']}>
							{t(desktopTrayMessages.status, {
								terminals: TERMINALS,
								failed: 0,
								attention: ATTENTION,
								done: DONE,
							})}
						</span>
						<i className={styles['rule']} />
						<span className={styles['current']}>{t(desktopTrayMessages.reopen)}</span>
						<i className={styles['rule']} />
						<span>{t(desktopTrayMessages.quitKeepRunning)}</span>
						<span>{t(desktopTrayMessages.quitAll)}</span>
					</div>
					<div className={styles['taskbar']}>
						<i className={styles['chevron']} />
						<span className={styles['currentIcon']}>
							<TrayIcon />
						</span>
						<span>10:24</span>
					</div>
				</div>
			}
		/>
	);
}
