import { useOrchbeeIntl, websiteMessages as messages } from '@orchbee/i18n';
import type { ReactElement, ReactNode } from 'react';

import { BeeMark } from '../bee-mark/BeeMark';
import { PillButton } from '../pill-button/PillButton';
import { SectionHeading } from '../section-heading/SectionHeading';
import { StatusDot, type StatusDotState } from '../status-dot/StatusDot';
import { TuiScreen } from '../tui-screen/TuiScreen';
import { WindowFrame } from '../window-frame/WindowFrame';
import styles from './Agents.module.css';
import { BOARD_AGENTS, boardFrame } from './board-frame';

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

const HIT_ROWS = ['hit0', 'hit1', 'hit2', 'hit3', 'hit4'] as const;

// Status lights everywhere an Agent appears: an Agent board the visitor can move through, the
// notifications Orchbee sends, and the Agents it can start.
export function Agents(): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	const stateNames: readonly string[] = [
		t(messages.stateNeedsYou),
		t(messages.stateWorking),
		t(messages.stateDone),
		t(messages.stateFailed),
		t(messages.stateIdle),
	];
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
					<WindowFrame title={`Ctrl+B g · ${t(messages.boardTitle)}`}>
						<TuiScreen
							lines={boardFrame(0)}
							size='board'
							label={t(messages.boardAlt)}
							role='group'
							hook='board'
						>
							{BOARD_AGENTS.map((agent, index) => (
								<button
									key={agent.title}
									className={styles[HIT_ROWS[index] ?? 'hit0']}
									type='button'
									data-board-row={index}
									aria-label={`${agent.title} · ${stateNames[index] ?? ''}`}
									aria-pressed={index === 0}
								/>
							))}
						</TuiScreen>
					</WindowFrame>
					<div className={styles['side']}>
						<Notification state='attention' time={t(messages.now)}>
							<b>{t(messages.noteNeedsAttention, { agent: 'claude-1' })}</b>
							<span>
								{t(messages.noteContext, { workspace: 'website', tab: 'build', panel: 'claude-1' })}
							</span>
						</Notification>
						<Notification state='done' time='5m'>
							<b>{t(messages.noteFinished, { agent: 'docs' })}</b>
							<span>
								{t(messages.noteContext, { workspace: 'website', tab: 'docs', panel: 'docs' })}
							</span>
						</Notification>
						<Notification state='failed' time='12m'>
							<b>{t(messages.noteFinished, { agent: 'api-fix' })}</b>
							<span>
								{t(messages.noteContext, {
									workspace: 'api-server',
									tab: 'tests',
									panel: 'api-fix',
								})}
							</span>
						</Notification>
						<div className={styles['sound']}>
							<PillButton variant='glass' size='small' icon='snd' action='chime'>
								{t(messages.hearIt)}
							</PillButton>
							<span className={styles['soundText']}>
								{t(messages.chimeLine)}
								<small>{t(messages.chimeDefault)}</small>
							</span>
						</div>
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

function Notification({
	state,
	time,
	children,
}: {
	state: StatusDotState;
	time: string;
	children: ReactNode;
}): ReactElement {
	return (
		<div className={styles['card']}>
			<span className={styles['tile']}>
				<BeeMark tile />
				<StatusDot state={state} place='badge' />
			</span>
			<span className={styles['cardBody']}>
				<small>Orchbee · {time}</small>
				{children}
			</span>
		</div>
	);
}
