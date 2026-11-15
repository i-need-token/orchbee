import {
	useOrchbeeIntl,
	websiteHomeMessages as messages,
	websiteShellMessages,
} from '@orchbee/i18n';
import { useId, type ReactElement } from 'react';

import { SectionHead } from '../../../section-head/SectionHead';
import { PROMPT } from '../../../session-scene/session';
import { WindowFit } from '../../../window-fit/WindowFit';
import { HexDot } from '../hex-dot/HexDot';
import { MultiplexingCommandLine } from './components/multiplexing-command-line/MultiplexingCommandLine';
import { MultiplexingDesktopApplication } from './components/multiplexing-desktop-application/MultiplexingDesktopApplication';
import { MultiplexingTerminalInterface } from './components/multiplexing-terminal-interface/MultiplexingTerminalInterface';
import { COMMAND_LENGTH, MULTIPLEXING_INITIAL, multiplexingFrame } from './multiplexing-scene';
import styles from './Multiplexing.module.css';

/**
 * One shell attached from two clients at once: a field the visitor types into, the terminal
 * interface and the desktop application side by side, and a console outside Orchbee that reads
 * the same shell. Without script the field stays hidden and the windows show the shell after one
 * command; with script the shell runs sample commands until the visitor types.
 */
export function Multiplexing(): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	const inputId = useId();
	const frame = multiplexingFrame(MULTIPLEXING_INITIAL);
	const terminal = t(websiteShellMessages.clientTerminal);
	const desktop = t(websiteShellMessages.clientDesktop);
	return (
		<section className={styles['section']} id='multiplexing'>
			<SectionHead
				kicker={t(messages.multiplexingKicker)}
				title={t(messages.multiplexingTitle)}
				lead={t(messages.multiplexingLead)}
			/>
			<div className={styles['figure']} data-reveal='' data-scene='multiplexing'>
				<div className={styles['field']} data-multiplexing-field='' hidden>
					<label className={styles['label']} htmlFor={inputId}>
						{t(messages.multiplexingInput)}
					</label>
					<span className={styles['prompt']}>{PROMPT.trimEnd()}</span>
					<input
						className={styles['input']}
						id={inputId}
						type='text'
						autoComplete='off'
						autoCapitalize='off'
						spellCheck={false}
						maxLength={COMMAND_LENGTH}
						placeholder='git status -sb'
					/>
				</div>
				<div className={styles['bus']}>
					<span>{terminal}</span>
					<span className={styles['line']} />
					<b className={styles['session']}>
						<HexDot />
						<span>{t(messages.multiplexingSession)}</span>
					</b>
					<span className={styles['line']} />
					<span>{desktop}</span>
				</div>
				<div className={styles['clients']}>
					<div>
						<p className={styles['client']}>{terminal}</p>
						<WindowFit>
							<MultiplexingTerminalInterface frame={frame} />
						</WindowFit>
					</div>
					<div>
						<p className={styles['client']}>{desktop}</p>
						<WindowFit>
							<MultiplexingDesktopApplication frame={frame} />
						</WindowFit>
					</div>
				</div>
				<p className={styles['reader']}>
					<b className={styles['readerName']}>{t(websiteShellMessages.clientCli)}</b>
					<span>{t(messages.multiplexingReader)}</span>
				</p>
				<WindowFit>
					<MultiplexingCommandLine frame={frame} />
				</WindowFit>
			</div>
		</section>
	);
}
