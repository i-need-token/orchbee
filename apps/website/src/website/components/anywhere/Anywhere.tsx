import { useOrchbeeIntl, websiteMessages as messages } from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { MirrorPrompt } from '../mirror-prompt/MirrorPrompt';
import { SectionHeading } from '../section-heading/SectionHeading';
import { StatusDot } from '../status-dot/StatusDot';
import { Tag } from '../tag/Tag';
import { TuiScreen } from '../tui-screen/TuiScreen';
import { WindowFrame } from '../window-frame/WindowFrame';
import styles from './Anywhere.module.css';
import { mirrorFrame } from './mirrorFrame';

// One session seen from the desktop app, the terminal and a phone. A line typed into any of
// them appears in all three; the page script keeps them in step.
export function Anywhere(): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	const transcript = (
		<>
			<div className={styles['muted']}>&gt; Add the install section</div>
			<div>
				<span className={styles['right']}>
					<span className={styles['ok']}>+64</span> <span className={styles['error']}>-3</span>
				</span>
				<i className={styles['bullet']} />
				Edit <span className={styles['muted']}>Website.tsx</span>
			</div>
			<div>
				<i className={styles['bullet']} />
				Run {' '}
				<span className={styles['muted']}>pnpm test</span>
			</div>
			<div>
				{'  '}
				<span className={styles['ok']}>14 passed</span>
				<span className={styles['muted']}>{'  '}1.2s</span>
			</div>
			<div>{' '}</div>
			<div>The install section is in.</div>
			<div>{' '}</div>
		</>
	);
	return (
		<section className={styles['scene']} id='anywhere' data-scene='1'>
			<div className={styles['inner']}>
				<div className={styles['heading']}>
					<SectionHeading
						number='01'
						label={t(messages.sceneAnywhere)}
						lead={t(messages.anywhereTitleLead)}
						accent={t(messages.anywhereTitleAccent)}
					>
						{t(messages.anywhereBody)}
					</SectionHeading>
					<p className={styles['hint']}>
						<svg
							width='16'
							height='16'
							viewBox='0 0 24 24'
							fill='none'
							stroke='currentColor'
							strokeWidth='1.6'
							strokeLinecap='round'
							strokeLinejoin='round'
							aria-hidden='true'
						>
							<rect x='2.5' y='6' width='19' height='12' rx='2' />
							<path d='M6 10h.01M10 10h.01M14 10h.01M18 10h.01M7 14h10' />
						</svg>
						<span>{t(messages.anywhereHint)}</span>
					</p>
				</div>
				<div className={styles['devices']} data-mirror=''>
					<figure className={styles['app']}>
						<WindowFrame title='Orchbee' focusRing>
							<div className={styles['desktop']}>
								<div className={styles['side']}>
									<div className={styles['newWorkspace']}>+ New workspace</div>
									<div className={styles['workspaceCurrent']}>
										<b>website</b>
										<small>feat/website</small>
									</div>
									<div className={styles['agent']}>
										<StatusDot state='done' hook='mirror-dot' />
										<span className={styles['agentName']}>
											claude-1 <span className={styles['muted']}>· Claude Code</span>
										</span>
									</div>
									<div className={styles['agent']}>
										<StatusDot state='done' />
										<span className={styles['agentName']}>
											docs <span className={styles['muted']}>· OpenCode</span>
										</span>
										<StatusDot state='unread' />
									</div>
									<div className={styles['agent']}>
										<StatusDot state='idle' />
										<span className={styles['agentName']}>
											sweeper <span className={styles['muted']}>· Gemini CLI</span>
										</span>
									</div>
									<div className={styles['workspace']}>
										<b>api-server</b>
										<small>fix/rate-limit</small>
									</div>
									<div className={styles['agent']}>
										<StatusDot state='failed' />
										<span className={styles['agentName']}>
											api-fix <span className={styles['muted']}>· Claude Code</span>
										</span>
										<StatusDot state='unread' />
									</div>
								</div>
								<div className={styles['main']}>
									<div className={styles['tabs']}>
										<span className={styles['tabOn']}>
											<StatusDot state='done' hook='mirror-dot' />
											claude-1
											<span className={styles['tabClose']}>×</span>
										</span>
										<span className={styles['tab']}>
											<StatusDot state='done' />
											docs
											<StatusDot state='unread' />
										</span>
										<span className={styles['tab']}>shell</span>
										<span className={styles['tabAdd']}>+</span>
									</div>
									<div className={styles['paneHead']}>
										<span className={styles['paneTitle']}>
											<StatusDot state='done' hook='mirror-dot' />
											claude-1
										</span>
										<span className={styles['paneTools']}>··· ×</span>
									</div>
									<div className={styles['terminal']}>
										{transcript}
										<MirrorPrompt label={t(messages.mirrorInput)} variant='desktop' />
									</div>
								</div>
							</div>
						</WindowFrame>
						<figcaption className={styles['captionEnd']}>
							<b>{t(messages.captionDesktop)}</b>
						</figcaption>
					</figure>
					<figure className={styles['term']}>
						<WindowFrame title='orchbee' size='small' focusRing>
							<TuiScreen lines={mirrorFrame('done')} size='mirror' label='orchbee' hook='mirror'>
								<div className={styles['overlay']}>
									<MirrorPrompt label={t(messages.mirrorInput)} variant='terminal' />
								</div>
							</TuiScreen>
						</WindowFrame>
						<figcaption className={styles['caption']}>
							<b>{t(messages.captionTerminal)}</b>TUI + CLI
						</figcaption>
					</figure>
					<figure className={styles['phone']}>
						<div className={styles['bezel']}>
							<div className={styles['screen']}>
								<i className={styles['notch']} />
								<div className={styles['phoneHead']}>
									<StatusDot state='done' hook='mirror-dot' />
									<span>
										<b>claude-1</b>
										<small>Claude Code · website</small>
									</span>
								</div>
								<div className={styles['phoneTerminal']}>
									{transcript}
									<MirrorPrompt label={t(messages.mirrorInput)} variant='phone' />
								</div>
								<div className={styles['keys']}>
									<span>esc</span>
									<span>ctrl</span>
									<span>tab</span>
									<span>↑</span>
									<span>↓</span>
								</div>
							</div>
						</div>
						<figcaption className={styles['caption']}>
							<b>{t(messages.captionPhone)}</b>
							<Tag honey>{t(messages.tagPlanned)}</Tag>
						</figcaption>
					</figure>
				</div>
			</div>
		</section>
	);
}
