import { useOrchbeeIntl, websiteCliMessages as messages } from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { sceneFinalState } from '../../../../scene/scene';
import { richText } from '../../../../rich-text';
import { DetailSection } from '../../../detail-section/DetailSection';
import { FigureCaption } from '../../../figure-caption/FigureCaption';
import { WindowFit } from '../../../window-fit/WindowFit';
import {
	COLLABORATION_COMMANDS,
	collaborationFrame,
	createCollaborationScene,
	stepProgress,
} from './collaboration-scene';
import { CollaborationWindow } from './components/collaboration-window/CollaborationWindow';
import styles from './CliCollaboration.module.css';

// Each step's command and what it does.
const STEPS = [
	{ command: COLLABORATION_COMMANDS[0], what: messages.collaborationStep1 },
	{ command: COLLABORATION_COMMANDS[1], what: messages.collaborationStep2 },
	{ command: COLLABORATION_COMMANDS[2], what: messages.collaborationStep3 },
	{ command: COLLABORATION_COMMANDS[3], what: messages.collaborationStep4 },
	{ command: COLLABORATION_COMMANDS[4], what: messages.collaborationStep5 },
] as const;

/**
 * One agent directing another through the command line: the drawn window, its caption, and the
 * rail of the five commands. Without script the window and the rail show all five steps finished
 * and the steps are not buttons yet; with script the steps play in turn, and a step replays from
 * its start when chosen (`activateCliCollaboration`).
 */
export function CliCollaboration(): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	const state = sceneFinalState(createCollaborationScene());
	return (
		<DetailSection
			kicker={t(messages.collaborationKicker)}
			title={t(messages.collaborationTitle)}
			lead={t(messages.collaborationLead)}
		>
			<div className={styles['figure']} data-reveal='' data-scene='collaboration'>
				<WindowFit>
					<CollaborationWindow frame={collaborationFrame(state)} />
				</WindowFit>
				<FigureCaption>
					<span className={styles['sideBySide']}>{t(messages.collaborationCaption, richText)}</span>
					<span className={styles['stacked']}>
						{t(messages.collaborationCaptionStacked, richText)}
					</span>
				</FigureCaption>
				<ol className={styles['rail']}>
					{STEPS.map(({ command, what }, index) => (
						<li
							key={command}
							className={styles['step']}
							data-step={index}
							data-done={stepProgress(state, index) === 'done' ? '' : undefined}
						>
							<span className={styles['progress']} data-step-progress='' />
							<button className={styles['button']} type='button' disabled>
								<i className={styles['mark']}>{index + 1}</i>
								<code className={styles['command']}>{command}</code>
								<span className={styles['what']}>{t(what, richText)}</span>
							</button>
						</li>
					))}
				</ol>
			</div>
		</DetailSection>
	);
}
