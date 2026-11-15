import { useOrchbeeIntl, websiteHomeMessages as messages } from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { TuiPanel } from '../../../../../tui-panel/TuiPanel';
import { TuiTabStrip } from '../../../../../tui-tab-strip/TuiTabStrip';
import { TuiWindow } from '../../../../../tui-window/TuiWindow';
import type { CollaborationFrame } from '../../collaboration-scene';
import styles from './CollaborationWindow.module.css';

/**
 * The terminal interface without its sidebar: Claude Code's Panel, and Codex's Panel opening
 * beside it when the first command starts it, below it in a narrow fit.
 */
export function CollaborationWindow({ frame }: { frame: CollaborationFrame }): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	return (
		<TuiWindow label={t(messages.collaborationWindow)} size='duo'>
			<TuiTabStrip title='Claude Code' state='working' />
			<div
				className={styles['panels']}
				data-open={frame.flags.split ? '' : undefined}
				data-scene-open='split'
			>
				<TuiPanel
					title='Review with Codex'
					state='working'
					focus
					rows={frame.rows.claude}
					bind={{ rows: 'claude' }}
				/>
				<TuiPanel
					title={frame.texts['title:codex']}
					state={frame.states.codex}
					rows={frame.rows.codex}
					bind={{ state: 'codex', title: 'title:codex', rows: 'codex' }}
				/>
			</div>
		</TuiWindow>
	);
}
