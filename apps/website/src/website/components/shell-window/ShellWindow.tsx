import type { ReactElement } from 'react';

import { TerminalText } from '../terminal-text/TerminalText';
import type { TerminalRow } from '../terminal-text/terminal-row';
import { WindowCaption } from '../window-caption/WindowCaption';
import styles from './ShellWindow.module.css';

/** The sizes a drawn shell window comes in; `full` narrows to the mini and phone layouts. */
export type ShellWindowSize = 'full';

/**
 * A plain terminal outside Orchbee with one PowerShell tab, in its own colors: where a person or
 * an agent runs the `orchbee` command line. Place it in a WindowFit. It is a picture, labelled by
 * `label`; `bind` names the scene rows script writes into it.
 */
export function ShellWindow({
	label,
	size,
	rows,
	bind,
}: {
	label: string;
	size: ShellWindowSize;
	rows: readonly TerminalRow[];
	bind?: string;
}): ReactElement {
	return (
		<div className={styles['window']} data-size={size} role='img' aria-label={label}>
			<div className={styles['bar']}>
				<span className={styles['tab']}>
					<span>PowerShell</span>
					<svg viewBox='0 0 10 10'>
						<path d='M1.5 1.5l7 7m0-7l-7 7' />
					</svg>
				</span>
				<span className={styles['add']}>
					<svg viewBox='0 0 10 10'>
						<path d='M5 1v8M1 5h8' />
					</svg>
				</span>
				<WindowCaption placement='end' />
			</div>
			<TerminalText rows={rows} host='console' bind={bind} />
		</div>
	);
}
