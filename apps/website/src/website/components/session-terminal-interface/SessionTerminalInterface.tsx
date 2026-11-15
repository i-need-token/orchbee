import { useOrchbeeIntl, websiteProductMessages } from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { CLAUDE_TITLE, sessionWorkspaces } from '../session-scene/session';
import { useSessionFrame } from '../session-scene/useSessionFrame';
import { TuiPanel } from '../tui-panel/TuiPanel';
import { TuiSidebar } from '../tui-sidebar/TuiSidebar';
import { TuiTabStrip } from '../tui-tab-strip/TuiTabStrip';
import { TuiWindow } from '../tui-window/TuiWindow';
import styles from './SessionTerminalInterface.module.css';

/**
 * The Session in the terminal interface: the shell Panel and, once started, Codex below it on the
 * left, Claude Code on the right. Drawn in its finished state and bound to the Session's keys, so
 * it plays inside a SessionScene. The mini and phone layouts keep the left column only.
 */
export function SessionTerminalInterface(): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	const frame = useSessionFrame();
	return (
		<TuiWindow
			label={t(websiteProductMessages.terminalInterface)}
			size='full'
			sidebar={<TuiSidebar project='shop' workspaces={sessionWorkspaces(frame)} />}
		>
			<TuiTabStrip
				title='PowerShell'
				state={frame.states.tab}
				unread={frame.flags['unread:any']}
				bind={{ state: 'tab', unread: 'unread:any' }}
			/>
			<div className={styles['panels']}>
				<div
					className={styles['column']}
					data-open={frame.flags['started:cx'] ? '' : undefined}
					data-scene-open='started:cx'
				>
					<TuiPanel title='PowerShell' focus rows={frame.rows.shell} bind={{ rows: 'shell' }} />
					<TuiPanel
						title={frame.texts['title:cx']}
						state={frame.states.cx}
						rows={frame.rows.codex}
						bind={{ state: 'cx', title: 'title:cx', rows: 'codex' }}
					/>
				</div>
				<div className={styles['secondColumn']}>
					<TuiPanel
						title={CLAUDE_TITLE}
						state={frame.states.cc}
						rows={frame.rows.claude}
						bind={{ state: 'cc', rows: 'claude' }}
					/>
				</div>
			</div>
		</TuiWindow>
	);
}
