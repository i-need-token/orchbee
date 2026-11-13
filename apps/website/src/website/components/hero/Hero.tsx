import { useOrchbeeIntl, websiteMessages as messages } from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { NPM_INSTALL_COMMAND, SOURCE_COMMANDS, type Release } from '../../release';
import { BeeMark } from '../bee-mark/BeeMark';
import { CopyButton } from '../copy-button/CopyButton';
import { Icon } from '../icon/Icon';
import { PillButton } from '../pill-button/PillButton';
import { StatusDot } from '../status-dot/StatusDot';
import { TuiScreen } from '../tui-screen/TuiScreen';
import { WindowFrame } from '../window-frame/WindowFrame';
import { compactHeroFrame, heroFrame } from './hero-frames';
import styles from './Hero.module.css';
import { ORBIT_PATH } from './orbit-path';

// The opening: the heading, the way to get Orchbee, and a live session whose window the visitor
// can close while its Agents keep working. The page script runs the story.
export function Hero({ release }: { release: Release }): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	const command = release.published ? NPM_INSTALL_COMMAND : SOURCE_COMMANDS[0];
	const hive = [
		{ name: 'claude-1', kind: 'Claude Code', activity: t(messages.activityEditing) },
		{ name: 'reviewer', kind: 'Codex', activity: t(messages.activityDiff) },
		{ name: 'pwsh', kind: 'pnpm dev', activity: t(messages.activityServer) },
	];
	return (
		<section className={styles['hero']} data-scene='0'>
			<div className={styles['inner']}>
				<div className={styles['copy']}>
					<p className={styles['live']}>
						<StatusDot state='working' hook='live-dot' />
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
							<WindowFrame title='orchbee — ~/code/website' closeLabel={t(messages.closeWindow)}>
								<TuiScreen
									lines={heroFrame(0)}
									size='hero'
									label={t(messages.heroAlt)}
									hook='hero'
									show='wide'
								/>
								<TuiScreen
									lines={compactHeroFrame(0)}
									size='compact'
									label={t(messages.heroAlt)}
									hook='hero-compact'
									show='narrow'
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
									<span className={styles['orbitBee']}>
										<BeeMark />
									</span>
								</div>
								<p className={styles['hiveTitle']}>{t(messages.hiveTitle)}</p>
								<ul className={styles['agents']}>
									{hive.map((agent, index) => (
										<li key={agent.name}>
											<StatusDot state='working' hook={`hive-dot-${index}`} />
											<b>{agent.name}</b>
											<span className={styles['kind']}>{agent.kind}</span>
											<span className={styles['activity']} data-hook={`hive-activity-${index}`}>
												{agent.activity}
											</span>
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
							className={styles['note']}
							type='button'
							data-action='open-window'
							data-hook='note'
							aria-label={t(messages.noteAria)}
							hidden
						>
							<span className={styles['noteIcon']}>
								<BeeMark tile />
							</span>
							<span className={styles['noteBody']}>
								<small>Orchbee · {t(messages.now)}</small>
								<b>{t(messages.noteNeedsAttention, { agent: 'claude-1' })}</b>
								<span>
									{t(messages.noteContext, {
										workspace: 'website',
										tab: 'build',
										panel: 'claude-1',
									})}
								</span>
							</span>
							<Icon name='snd' size={16} />
						</button>
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
