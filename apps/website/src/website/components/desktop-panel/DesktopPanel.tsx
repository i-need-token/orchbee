import type { ReactElement } from 'react';

import { Codicon } from '../codicon/Codicon';
import { StateGlyph, type AgentState } from '../state-glyph/StateGlyph';
import { TerminalText } from '../terminal-text/TerminalText';
import type { TerminalRow } from '../terminal-text/terminal-row';
import styles from './DesktopPanel.module.css';

/**
 * A Panel of the desktop application: its header (an Agent's state glyph before the centered
 * title, More and Close at the end) on the application canvas, then the terminal. A Panel without
 * focus lies under the product's focus scrim. A layout sizes it; a column that carries
 * `data-panel-column` and lacks `data-open` holds the Tab's only Panel in the mini and phone
 * layouts, which then shows no header. `bind` names the scene values script writes into it.
 */
export function DesktopPanel({
	title,
	state,
	focus = false,
	rows,
	bind,
}: {
	title: string;
	state?: AgentState;
	focus?: boolean;
	rows: readonly TerminalRow[];
	bind?: { readonly state?: string; readonly title?: string; readonly rows?: string };
}): ReactElement {
	return (
		<div className={focus ? styles['focusedPanel'] : styles['panel']}>
			<header className={styles['header']}>
				<span
					className={state === undefined ? styles['titleGroup'] : styles['titleGroupWithGlyph']}
				>
					{state === undefined ? null : (
						<StateGlyph state={state} bind={bind?.state} product='small' />
					)}
					<span className={styles['title']} data-scene-text={bind?.title}>
						{title}
					</span>
				</span>
				<span className={styles['actions']}>
					<span className={styles['action']}>
						<Codicon name='ellipsis' />
					</span>
					<span className={styles['action']}>
						<Codicon name='close' />
					</span>
				</span>
			</header>
			<TerminalText rows={rows} host='desktop' bind={bind?.rows} />
		</div>
	);
}
