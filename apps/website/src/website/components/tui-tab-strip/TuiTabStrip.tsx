import type { ReactElement } from 'react';

import { StateGlyph, type AgentState } from '../state-glyph/StateGlyph';
import styles from './TuiTabStrip.module.css';

/**
 * The Tab strip of the terminal interface with its one selected Tab, its close and new-Tab
 * buttons. The Tab shows the state of its most urgent Agent and an unread mark. `bind` names the
 * scene values script writes into it.
 */
export function TuiTabStrip({
	title,
	state,
	unread,
	bind,
}: {
	title: string;
	state: AgentState;
	unread: boolean;
	bind?: { readonly state?: string; readonly unread?: string };
}): ReactElement {
	return (
		<div className={styles['strip']}>
			<span className={styles['tab']}>
				<StateGlyph state={state} bind={bind?.state} />
				<span>{title}</span>
				<span className={styles['unread']} hidden={!unread} data-scene-show={bind?.unread}>
					•
				</span>
			</span>
			<span className={styles['dim']}>[x]</span>
			<span className={styles['dim']}>[+]</span>
		</div>
	);
}
