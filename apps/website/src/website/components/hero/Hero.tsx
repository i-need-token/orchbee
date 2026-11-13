import { useOrchbeeIntl, websiteMessages as messages } from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { HERO_NARROW_PICTURE, HERO_PICTURES } from '../../pictures';
import { NPM_INSTALL_COMMAND, SOURCE_COMMANDS, type Release } from '../../release';
import { BrandMark } from '../brand-mark/BrandMark';
import { CopyButton } from '../copy-button/CopyButton';
import { Icon } from '../icon/Icon';
import { PillButton } from '../pill-button/PillButton';
import { ProductPicture } from '../product-picture/ProductPicture';
import { StatusDot, type StatusDotState } from '../status-dot/StatusDot';
import { WindowFrame } from '../window-frame/WindowFrame';
import styles from './Hero.module.css';
import { ORBIT_PATH } from './orbit-path';

// The opening: the heading, the way to get Orchbee, and a screenshot of the terminal interface
// whose window the visitor can close while its Agents keep working. The page script runs the story.
export function Hero({ release }: { release: Release }): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	const command = release.published ? NPM_INSTALL_COMMAND : SOURCE_COMMANDS[0];
	// The Agents in the screenshot, with the state their sidebar rows show.
	const hive: readonly { name: string; workspace: string; state: StatusDotState }[] = [
		{ name: 'OpenCode', workspace: 'shop-api', state: 'working' },
		{ name: 'Gemini CLI', workspace: 'shop-api', state: 'working' },
		{ name: 'Grok', workspace: 'shop-web', state: 'done' },
	];
	const stateName: Readonly<Partial<Record<StatusDotState, string>>> = {
		working: t(messages.stateWorking),
		done: t(messages.stateDone),
	};
	return (
		<section className={styles['hero']} data-scene='0'>
			<div className={styles['inner']}>
				<div className={styles['copy']}>
					<p className={styles['live']}>
						<StatusDot state='working' />
						<span data-hook='live-text'>{t(messages.liveWorking)}</span>
					</p>
					<h1 className={styles['title']}>
						<span className={styles['line']}>{t(messages.heroTitleLead)}</span>
						<span className={styles['accent']}>{t(messages.heroTitleAccent)}</span>
					</h1>
					<p className={styles['lead']}>{t(messages.heroLead)}</p>
					<div className={styles['actions']}>
						{release.published ? (
							<PillButton variant='primary' icon='dl' href={release.installerUrl}>
								{t(messages.downloadForWindows)}
							</PillButton>
						) : (
							<PillButton variant='primary' icon='dl' href='#install'>
								{t(messages.getSource)}
							</PillButton>
						)}
						<div className={styles['command']}>
							<span className={styles['prompt']}>PS&gt;</span>
							<code>{command}</code>
							<CopyButton text={command} />
						</div>
					</div>
					<p className={styles['platforms']}>
						{t(messages.platformsNow)} <span aria-hidden='true'>/</span> {t(messages.platformsNext)}
					</p>
				</div>
				<div className={styles['stage']} data-hook='stage'>
					<p className={styles['try']} data-hook='try' hidden>
						<svg
							width='12'
							height='14'
							viewBox='0 0 12 14'
							fill='none'
							stroke='currentColor'
							strokeWidth='1.5'
							strokeLinecap='round'
							strokeLinejoin='round'
							aria-hidden='true'
						>
							<path d='M6 1v11M2 8l4 4 4-4' />
						</svg>
						<span>{t(messages.tryClose)}</span>
					</p>
					<div className={styles['demo']} data-hook='demo'>
						<div className={styles['window']}>
							<WindowFrame title='orchbee' closeLabel={t(messages.closeWindow)}>
								<ProductPicture
									files={HERO_PICTURES}
									narrow={HERO_NARROW_PICTURE}
									sizes='(max-width: 48rem) calc(100vw - 2.5rem), (max-width: 77.5rem) calc(100vw - 5rem), 72.5rem'
									alt={t(messages.heroAlt)}
									eager
								/>
							</WindowFrame>
						</div>
						<div className={styles['hive']} aria-live='polite'>
							<div className={styles['hiveInner']}>
								<div className={styles['orbit']} aria-hidden='true'>
									<svg viewBox='0 0 240 120'>
										<path className={styles['orbitPath']} d={ORBIT_PATH} />
									</svg>
									<i className={styles['bead']} />
									<i className={styles['bead']} />
									<i className={styles['bead']} />
									<span className={styles['orbitMark']}>
										<BrandMark />
									</span>
								</div>
								<p className={styles['hiveTitle']}>{t(messages.hiveTitle)}</p>
								<ul className={styles['agents']}>
									{hive.map((agent) => (
										<li key={agent.name}>
											<StatusDot state={agent.state} />
											<b>{agent.name}</b>
											<span className={styles['workspace']}>{agent.workspace}</span>
											<span className={styles['state']}>{stateName[agent.state]}</span>
										</li>
									))}
								</ul>
								<div className={styles['hiveFoot']}>
									<PillButton variant='glass' size='small' icon='win' action='open-window'>
										{t(messages.reopenWindow)}
									</PillButton>
									<span className={styles['clock']} data-hook='closed-for' />
								</div>
							</div>
						</div>
						<button
							className={styles['replay']}
							type='button'
							data-action='replay'
							data-hook='replay'
							hidden
						>
							<Icon name='replay' size={13} />
							<span>{t(messages.replay)}</span>
						</button>
					</div>
				</div>
			</div>
		</section>
	);
}
