import { useOrchbeeIntl, websiteMessages as messages } from '@orchbee/i18n';
import type { ReactElement, ReactNode } from 'react';

import { DESKTOP_LEFT_PICTURE, DESKTOP_PICTURES, HERO_SIDEBAR_PICTURE } from '../../pictures';
import { ProductPicture } from '../product-picture/ProductPicture';
import { SectionHeading } from '../section-heading/SectionHeading';
import { StatusDot, type StatusDotState } from '../status-dot/StatusDot';
import { WindowFrame } from '../window-frame/WindowFrame';
import styles from './Agents.module.css';

// Agents the product can start with `orchbee agent start <agent>`.
const STARTABLE_AGENTS = [
	'pi',
	'claude-code',
	'codex',
	'gemini',
	'cursor',
	'devin',
	'antigravity',
	'cline',
	'omp',
	'mastracode',
	'opencode',
	'github-copilot',
	'kimi',
	'kiro',
	'droid',
	'amp',
	'grok',
	'hermes',
	'kilo',
	'qodercli',
	'qwen-code',
	'letta',
	'maki',
	'muse',
	'atomcode',
	'codebuddy',
	'command-code',
	'deveco',
	'mimo',
	'reasonix',
	'snow',
	'minimax-code',
	'mothx',
	'jcode',
	'openclaw',
	'prime-agent',
] as const;

// Status lights wherever an Agent appears: the desktop app and the terminal interface sidebar,
// the reminder Orchbee can give, and the Agents it can start.
export function Agents(): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	const legend: readonly (readonly [StatusDotState, string])[] = [
		['working', t(messages.stateWorking)],
		['attention', t(messages.stateNeedsYou)],
		['done', t(messages.stateDone)],
		['failed', t(messages.stateFailed)],
		['idle', t(messages.stateIdle)],
		['unread', t(messages.stateUnread)],
	];
	return (
		<section className={styles['scene']} id='agents' data-scene='2'>
			<div className={styles['inner']}>
				<div className={styles['heading']}>
					<SectionHeading
						number='02'
						label={t(messages.sceneAgents)}
						lead={t(messages.agentsTitleLead)}
						accent={t(messages.agentsTitleAccent)}
					>
						{t(messages.agentsBody)}
					</SectionHeading>
				</div>
				<div className={styles['grid']}>
					<figure className={styles['figure']}>
						<WindowFrame title='Orchbee'>
							<ProductPicture
								files={DESKTOP_PICTURES}
								narrow={DESKTOP_LEFT_PICTURE}
								sizes='(max-width: 48rem) calc(100vw - 2.5rem), (max-width: 64rem) calc(100vw - 5rem), 54rem'
								alt={t(messages.desktopAlt)}
							/>
						</WindowFrame>
						<figcaption className={styles['caption']}>
							<b>{t(messages.captionDesktop)}</b>
						</figcaption>
					</figure>
					<div className={styles['side']}>
						<figure className={styles['figure']}>
							<WindowFrame title='orchbee' size='small'>
								<ProductPicture
									files={[HERO_SIDEBAR_PICTURE]}
									sizes='16.25rem'
									alt={t(messages.sidebarAlt)}
								/>
							</WindowFrame>
							<figcaption className={styles['caption']}>
								<b>{t(messages.captionTerminal)}</b>TUI
							</figcaption>
						</figure>
						<p className={styles['reminder']}>
							{t(messages.chimeLine)}
							<small>{t(messages.chimeDefault)}</small>
						</p>
					</div>
				</div>
				<div className={styles['legend']}>
					{legend.map(([state, name]) => (
						<span key={state}>
							<StatusDot state={state} />
							{name}
						</span>
					))}
				</div>
				<div className={styles['kinds']}>
					<p className={styles['kindsLine']}>
						{t(messages.kindsLine, {
							code: (chunks: ReactNode[]) => <code key='code'>{chunks}</code>,
						})}
					</p>
					<div className={styles['kindList']}>
						{STARTABLE_AGENTS.map((agent) => (
							<span key={agent}>{agent}</span>
						))}
					</div>
				</div>
			</div>
		</section>
	);
}
