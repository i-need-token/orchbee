import { desktopTerminalMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { Button } from '@orchbee/ui/components';
import type { WorkbenchTerminalSurfaceProps } from '@orchbee/workbench';
import type { ReactNode } from 'react';

import styles from './DesktopTerminalSurface.module.css';
import { useDesktopTerminalSurface } from './hooks/useDesktopTerminalSurface';

export function DesktopTerminalSurface({ presentation }: WorkbenchTerminalSurfaceProps): ReactNode {
	const view = useDesktopTerminalSurface(presentation);
	const intl = useOrchbeeIntl();
	return (
		<div
			className={styles['terminalSurface']}
			data-terminal-presentation-id={presentation.presentationId}
			data-terminal-projection-state={view.projectionState}
			ref={view.root}
		>
			{view.stopped ? (
				<div className={styles['stopped']} data-slot='desktop-terminal-stopped'>
					<p>{intl.formatMessage(desktopTerminalMessages.sessionStopped)}</p>
					<Button data-slot='desktop-terminal-start-new-session' onClick={view.startNewSession}>
						{intl.formatMessage(desktopTerminalMessages.startNewSession)}
					</Button>
				</div>
			) : null}
			{view.failureMessage === null ? null : (
				<div className={styles['status']} role='alert'>
					{view.failureMessage}
				</div>
			)}
		</div>
	);
}
