import { useOrchbeeIntl, websiteProductMessages } from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { DesktopPanel } from '../desktop-panel/DesktopPanel';
import { DesktopSidebar } from '../desktop-sidebar/DesktopSidebar';
import { DesktopTabStrip } from '../desktop-tab-strip/DesktopTabStrip';
import { DesktopWindow } from '../desktop-window/DesktopWindow';
import { CLAUDE_TITLE, sessionWorkspaces } from '../session-scene/session';
import { useSessionFrame } from '../session-scene/useSessionFrame';
import styles from './SessionDesktopApplication.module.css';

/**
 * The Session in the desktop application: the shell Panel and, once started, Codex below it on the
 * left, Claude Code on the right. Drawn in its finished state and bound to the Session's keys, so
 * it plays inside a SessionScene. The mini and phone layouts keep the left column only.
 */
export function SessionDesktopApplication(): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	const frame = useSessionFrame();
	return (
		<DesktopWindow
			label={t(websiteProductMessages.desktopApplication)}
			size='full'
			sidebar={<DesktopSidebar project='shop' workspaces={sessionWorkspaces(frame)} />}
		>
			<DesktopTabStrip
				title='PowerShell'
				state={frame.states.tab}
				unread={frame.flags['unread:any']}
				bind={{ state: 'tab', unread: 'unread:any' }}
			/>
			<div className={styles['panels']}>
				<div
					className={styles['column']}
					data-panel-column=''
					data-open={frame.flags['started:cx'] ? '' : undefined}
					data-scene-open='started:cx'
				>
					<DesktopPanel title='PowerShell' focus rows={frame.rows.shell} bind={{ rows: 'shell' }} />
					<DesktopPanel
						title={frame.texts['title:cx']}
						state={frame.states.cx}
						rows={frame.rows.codex}
						bind={{ state: 'cx', title: 'title:cx', rows: 'codex' }}
					/>
				</div>
				<div className={styles['secondColumn']}>
					<DesktopPanel
						title={CLAUDE_TITLE}
						state={frame.states.cc}
						rows={frame.rows.claude}
						bind={{ state: 'cc', rows: 'claude' }}
					/>
				</div>
			</div>
		</DesktopWindow>
	);
}
