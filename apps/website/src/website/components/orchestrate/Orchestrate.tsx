import { useOrchbeeIntl, websiteMessages as messages } from '@orchbee/i18n';
import type { ReactElement, ReactNode } from 'react';

import { joinClassNames } from '../../joinClassNames';
import { SectionHeading } from '../section-heading/SectionHeading';
import { Tag } from '../tag/Tag';
import { TuiScreen } from '../tui-screen/TuiScreen';
import { WindowFrame } from '../window-frame/WindowFrame';
import { orchestrateFrame } from './orchestrateFrame';
import styles from './Orchestrate.module.css';
import { SHELL_LINES } from './shell-script';

const QUOTED = /^(?<before>[^"]*)(?<quote>"[^"]*")(?<after>.*)$/u;

function highlightQuote(text: string): ReactNode {
	const parts = QUOTED.exec(text)?.groups;
	if (parts === undefined) return text;
	return (
		<>
			{parts['before']}
			<span className={styles['quote']}>{parts['quote']}</span>
			{parts['after']}
		</>
	);
}

// A shell drives one Agent from another with orchbee commands while the terminal interface
// beside it shows the result. The static page shows the finished run; the page script loops it.
export function Orchestrate(): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	return (
		<section className={styles['scene']} id='orchestrate' data-scene='3'>
			<div className={styles['inner']}>
				<div className={styles['heading']}>
					<SectionHeading
						number='03'
						label={t(messages.sceneOrchestrate)}
						lead={t(messages.orchestrateTitleLead)}
						accent={t(messages.orchestrateTitleAccent)}
					>
						{t(messages.orchestrateBody)}
					</SectionHeading>
				</div>
				<div className={styles['grid']}>
					<WindowFrame title='pwsh — ~/code/website'>
						<div className={styles['shell']}>
							<div className={styles['lines']} data-shell=''>
								{SHELL_LINES.map((line, index) =>
									line.kind === 'command' ? (
										<div className={styles['line']} key={index} data-from={line.step}>
											<span className={styles['prompt']}>PS&gt; </span>
											<span data-type={line.step}>{highlightQuote(line.text)}</span>
											<i className={styles['caret']} data-typing-caret='' hidden />
										</div>
									) : (
										<div
											className={joinClassNames(
												styles['line'],
												line.tone === 'muted' && styles['muted'],
											)}
											key={index}
											data-from={line.step}
										>
											{line.text}
										</div>
									),
								)}
								<div className={styles['line']} data-prompt-line=''>
									<span className={styles['prompt']}>PS&gt; </span>
									<i className={styles['caret']} />
								</div>
							</div>
						</div>
					</WindowFrame>
					<WindowFrame title='orchbee'>
						<TuiScreen
							lines={orchestrateFrame(4)}
							size='orchestrate'
							label={t(messages.orchestrateAlt)}
							hook='orchestrate'
						/>
					</WindowFrame>
				</div>
				<p className={styles['next']}>
					<Tag>{t(messages.tagInDevelopment)}</Tag>
					<span>{t(messages.orchestrateNext)}</span>
				</p>
			</div>
		</section>
	);
}
