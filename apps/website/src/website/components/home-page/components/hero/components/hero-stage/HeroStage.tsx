import {
	useOrchbeeIntl,
	websiteHomeMessages as messages,
	websiteShellMessages,
} from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { richText } from '../../../../../../rich-text';
import { FigureCaption } from '../../../../../figure-caption/FigureCaption';
import { SessionCommandLine } from '../../../../../session-command-line/SessionCommandLine';
import { SessionDesktopApplication } from '../../../../../session-desktop-application/SessionDesktopApplication';
import { sessionAgents } from '../../../../../session-scene/session';
import { useSessionFrame } from '../../../../../session-scene/useSessionFrame';
import { SessionTerminalInterface } from '../../../../../session-terminal-interface/SessionTerminalInterface';
import { StateGlyph } from '../../../../../state-glyph/StateGlyph';
import { StateLegend } from '../../../../../state-legend/StateLegend';
import { WindowFit } from '../../../../../window-fit/WindowFit';
import styles from './HeroStage.module.css';

// The clients the stage shows the Session in, in switch order; the first is shown first.
const VIEWS = [
	{
		id: 'desktop',
		name: websiteShellMessages.clientDesktop,
		shortName: websiteShellMessages.navDesktop,
		Window: SessionDesktopApplication,
	},
	{
		id: 'terminal',
		name: websiteShellMessages.clientTerminal,
		shortName: websiteShellMessages.navTerminal,
		Window: SessionTerminalInterface,
	},
	{
		id: 'cli',
		name: websiteShellMessages.clientCli,
		shortName: websiteShellMessages.navCli,
		Window: SessionCommandLine,
	},
] as const;

/**
 * The drawn Session under the home headline: a switch between the clients that show it, the live
 * Session status, a Close button with the note that invites the visitor to press it, the card that
 * shows the Session still running once they do, and the caption with the legend of Agent states. Without script the stage shows the
 * first client's window and no controls.
 */
export function HeroStage(): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	const frame = useSessionFrame();
	return (
		<div data-hero-stage=''>
			<div className={styles['bar']} data-stage-bar='' hidden>
				<div className={styles['switch']} role='group' aria-label={t(messages.stageView)}>
					{VIEWS.map((view, index) => (
						<button key={view.id} type='button' data-view={view.id} aria-pressed={index === 0}>
							<span className={styles['name']}>{t(view.name)}</span>
							<span className={styles['shortName']}>{t(view.shortName)}</span>
						</button>
					))}
				</div>
				<span className={styles['barEnd']}>
					<span className={styles['status']} data-live=''>
						<StateGlyph state={frame.states.summary} bind='summary' />
						<span data-scene-text='summary'>{frame.texts.summary}</span>
					</span>
					<span className={styles['hint']}>{t(messages.stageCloseHint)}</span>
					<button className={styles['close']} type='button' data-stage-close=''>
						{t(messages.stageClose)}
					</button>
				</span>
			</div>
			<div className={styles['stage']} data-stage=''>
				<div className={styles['views']}>
					{VIEWS.map(({ id, Window }, index) => (
						<div
							key={id}
							className={styles['view']}
							data-view={id}
							data-on={index === 0 ? '' : undefined}
						>
							<WindowFit>
								<Window />
							</WindowFit>
						</div>
					))}
				</div>
				<div className={styles['away']}>
					<div className={styles['awayContent']}>
						<p className={styles['awayTitle']}>{t(messages.stageAwayTitle)}</p>
						<p className={styles['awayBody']}>{t(messages.stageAwayBody)}</p>
						<div className={styles['agents']}>
							{sessionAgents(frame).map((agent) => (
								<div
									key={agent.id}
									className={styles['agent']}
									hidden={!agent.shown}
									data-scene-show={agent.bind.shown}
								>
									<StateGlyph state={agent.state} bind={agent.bind.state} />
									<span className={styles['agentName']}>
										<span data-scene-text={agent.bind.title}>{agent.title}</span>{' '}
										<em className={styles['provider']}>· {agent.provider}</em>
									</span>
									<small className={styles['word']} data-scene-text={agent.bind.word}>
										{agent.word}
									</small>
								</div>
							))}
						</div>
						<div className={styles['awayFoot']}>
							<span>
								{t(messages.stageClosedFor)} <span data-stage-clock=''>00:00</span>
							</span>
							<button className={styles['reattach']} type='button' data-stage-reattach=''>
								{t(messages.stageReattach)}
							</button>
						</div>
					</div>
				</div>
			</div>
			<FigureCaption aside={<StateLegend states={['working', 'attention', 'done', 'idle']} />}>
				{t(messages.stageCaption, richText)}
			</FigureCaption>
		</div>
	);
}
