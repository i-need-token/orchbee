import type { ReactElement } from 'react';

import { StateGlyph, type AgentState } from '../state-glyph/StateGlyph';
import { TerminalText } from '../terminal-text/TerminalText';
import type { TerminalRow } from '../terminal-text/terminal-row';
import styles from './TuiPanel.module.css';

/**
 * A Panel of the terminal interface: a framed terminal with its title set into the top edge, an
 * Agent's state glyph before the title, and the focus color when it has focus. The outer element
 * keeps the room the title needs above the frame; a layout sizes it, and carries `data-focus` while
 * the Panel has focus. `bind` names the scene values script writes into it; `leaf` is the Panel's
 * id when a PanelTree draws it.
 */
export function TuiPanel({
	title,
	state,
	focus = false,
	rows,
	bind,
	leaf,
}: {
	title: string;
	state?: AgentState;
	focus?: boolean;
	rows: readonly TerminalRow[];
	bind?: { readonly state?: string; readonly title?: string; readonly rows?: string };
	leaf?: number;
}): ReactElement {
	return (
		<div className={styles['cell']} data-focus={focus ? '' : undefined} data-leaf={leaf}>
			<div className={styles['panel']}>
				<header className={styles['header']}>
					{state === undefined ? null : (
						<StateGlyph state={state} selected={focus} bind={bind?.state} />
					)}
					<span data-scene-text={bind?.title}>{title}</span>
				</header>
				<TerminalText rows={rows} host='panel' bind={bind?.rows} />
			</div>
		</div>
	);
}
