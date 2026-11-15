import { useOrchbeeIntl, websiteCliMessages as messages } from '@orchbee/i18n';
import { useId, type ReactElement, type ReactNode } from 'react';

import { CopyButton } from '../../../copy-button/CopyButton';
import { DetailSection } from '../../../detail-section/DetailSection';
import { CLI_OUTPUTS, type OutputLine } from './cli-outputs';
import styles from './CliOutputs.module.css';

// A run's identity is its place in the output, which never changes.
function byPosition<Item>(items: readonly Item[]): { key: string; item: Item }[] {
	return items.map((item, position) => ({ key: String(position), item }));
}

/** The output as one preformatted text, a line break after every line but the last. */
function outputText(lines: readonly OutputLine[]): ReactNode[] {
	return byPosition(lines).flatMap(({ key: at, item: line }, index) => [
		...byPosition(line).map(({ key, item: segment }) =>
			typeof segment === 'string' ? (
				segment
			) : (
				<span key={`${at}:${key}`} data-tone={segment.tone}>
					{segment.text}
				</span>
			),
		),
		...(index < lines.length - 1 ? ['\n'] : []),
	]);
}

/**
 * Commands and what each prints. Every output is drawn; the first shows, and without script the
 * list is a plain list beside it. Activation turns the list into tabs that switch the output.
 */
export function CliOutputs(): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	const id = useId();
	return (
		<DetailSection
			kicker={t(messages.outputKicker)}
			title={t(messages.outputTitle)}
			lead={t(messages.outputLead)}
		>
			<div className={styles['explorer']} data-cli-outputs='' data-reveal=''>
				<ul className={styles['list']} aria-label={t(messages.outputList)} data-output-list=''>
					{CLI_OUTPUTS.map((output, index) => (
						<li
							key={output.command}
							id={`${id}-tab-${index}`}
							className={styles['command']}
							data-output-tab=''
							data-current={index === 0 ? '' : undefined}
						>
							{output.command}
						</li>
					))}
				</ul>
				<div className={styles['outputs']}>
					{CLI_OUTPUTS.map((output, index) => (
						<div
							key={output.command}
							id={`${id}-output-${index}`}
							className={styles['output']}
							data-output-panel=''
							hidden={index !== 0}
						>
							<div className={styles['bar']}>
								<span id={`${id}-command-${index}`} className={styles['barCommand']}>
									{output.command}
								</span>
								<CopyButton text={output.command} />
							</div>
							<pre
								className={styles['body']}
								tabIndex={0}
								aria-labelledby={`${id}-command-${index}`}
								data-output-body=''
							>
								{outputText(output.lines)}
							</pre>
						</div>
					))}
				</div>
			</div>
			<p className={styles['note']}>{t(messages.outputNote)}</p>
		</DetailSection>
	);
}
