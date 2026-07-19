import { type MessageDescriptor, useOrchbeeIntl, workbenchMessages } from '@orchbee/i18n';
import type * as React from 'react';

import type { WorkbenchSession, WorkbenchSessionStatus } from '../sessions';
import styles from './TerminalPane.module.css';
import { useTerminalPane } from './hooks/useTerminalPane';

export interface TerminalPaneProps {
	readonly session: WorkbenchSession;
	readonly active: boolean;
	readonly onFocus: () => void;
	readonly showHeader?: boolean;
}

const SESSION_STATUS_MESSAGES = {
	disconnected: workbenchMessages.sessionStatusDisconnected,
	exited: workbenchMessages.sessionStatusExited,
	running: workbenchMessages.sessionStatusRunning,
	starting: workbenchMessages.sessionStatusStarting,
} satisfies Readonly<Record<WorkbenchSessionStatus, MessageDescriptor>>;

export function TerminalPane({
	session,
	active,
	onFocus,
	showHeader = true,
}: TerminalPaneProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const terminalPane = useTerminalPane(session);

	const rootClassName = active ? styles['activeRoot'] : styles['root'];
	return (
		<section
			className={rootClassName}
			aria-label={intl.formatMessage(workbenchMessages.terminalPaneLabel, {
				sessionTitle: session.title,
			})}
			onFocusCapture={onFocus}
		>
			{showHeader && (
				<div className={styles['header']}>
					<span
						className={styles['statusIndicator']}
						data-status={session.status}
						aria-hidden='true'
					/>
					<span className={styles['title']}>{session.title}</span>
					<span className={styles['status']}>
						{intl.formatMessage(SESSION_STATUS_MESSAGES[session.status])}
					</span>
				</div>
			)}
			<div ref={terminalPane.surfaceReference} className={styles['surface']} />
			{terminalPane.paneState !== 'ready' && (
				<div
					className={styles['overlay']}
					role={terminalPane.paneState === 'failed' ? 'alert' : 'status'}
				>
					{intl.formatMessage(
						terminalPane.paneState === 'failed'
							? workbenchMessages.terminalStartFailed
							: workbenchMessages.terminalStarting,
					)}
				</div>
			)}
		</section>
	);
}
